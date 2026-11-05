//! Agent-only MCP stdio adapter. Startup and socket selection belong to the CLI.
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Write},
    path::Path,
};

const MAX_FRAME: usize = 1024 * 1024;
const PROTOCOL: &str = "2024-11-05";

/// Serve newline-delimited MCP JSON-RPC on stdin/stdout using agent authority only.
/// Tools connect on demand as agents. Idle stdio never needs an owner capability.
pub fn serve(socket: &Path) -> Result<(), String> {
    // Refuse before waiting on stdin, even if the caller's local config differs.
    drop(crate::session::connect_agent_or_start(socket)?);
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    let mut initialized = false;
    loop {
        let Some(frame) = read_frame(&mut reader)? else {
            return Ok(());
        };
        // A running adapter also refuses the next frame after a policy change.
        drop(crate::session::connect_agent_or_start(socket)?);
        let response = match serde_json::from_slice::<Value>(&frame) {
            Ok(request) => dispatch(socket, &request, &mut initialized),
            Err(_) => Some(rpc_error(Value::Null, -32700, "Parse error")),
        };
        if let Some(mut response) = response {
            let mut bytes = serde_json::to_vec(&response)
                .map_err(|_| "MCP response encoding failed".to_owned())?;
            if bytes.len() + 1 > MAX_FRAME {
                response = rpc_error(response["id"].clone(), -32603, "Response exceeds 1 MiB");
                bytes = serde_json::to_vec(&response)
                    .map_err(|_| "MCP response encoding failed".to_owned())?;
            }
            writer
                .write_all(&bytes)
                .and_then(|_| writer.write_all(b"\n"))
                .and_then(|_| writer.flush())
                .map_err(|_| "MCP response write failed".to_owned())?;
        }
    }
}

// Check the bound before extending: read_until/read_line can allocate without limit.
fn read_frame(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, String> {
    let mut frame = Vec::new();
    loop {
        let available = reader.fill_buf().map_err(|_| "MCP input read failed")?;
        if available.is_empty() {
            return if frame.is_empty() {
                Ok(None)
            } else {
                Err("Incomplete MCP frame".into())
            };
        }
        let n = available
            .iter()
            .position(|b| *b == b'\n')
            .map_or(available.len(), |i| i + 1);
        if frame.len() + n > MAX_FRAME {
            return Err("MCP frame exceeds 1 MiB".into());
        }
        let complete = available[n - 1] == b'\n';
        frame.extend_from_slice(&available[..n]);
        reader.consume(n);
        if complete {
            return Ok(Some(frame));
        }
    }
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

fn dispatch(socket: &Path, request: &Value, initialized: &mut bool) -> Option<Value> {
    let Some(object) = request.as_object() else {
        return Some(rpc_error(Value::Null, -32600, "Invalid Request"));
    };
    let id = object.get("id");
    let valid_id = id
        .is_none_or(|id| id.is_i64() || id.is_u64() || id.as_str().is_some_and(|s| s.len() <= 256));
    if request["jsonrpc"] != "2.0"
        || !valid_id
        || !request["method"].is_string()
        || object
            .keys()
            .any(|k| !matches!(k.as_str(), "jsonrpc" | "id" | "method" | "params"))
        || object.get("params").is_some_and(|p| !p.is_object())
    {
        return Some(rpc_error(Value::Null, -32600, "Invalid Request"));
    }
    // Notifications, including notifications/initialized, never receive a reply.
    let id = id?.clone();
    let params = object.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = match request["method"].as_str().unwrap() {
        "initialize" => {
            if *initialized || !params["protocolVersion"].is_string() {
                return Some(rpc_error(id, -32602, "Invalid initialize parameters"));
            }
            *initialized = true;
            json!({"protocolVersion":PROTOCOL,"capabilities":{"tools":{"listChanged":false}},
                "serverInfo":{"name":"clanky","version":env!("CARGO_PKG_VERSION")}})
        }
        "ping" => json!({}),
        _ if !*initialized => return Some(rpc_error(id, -32600, "Initialize first")),
        "tools/list" => {
            if params.as_object().unwrap().keys().any(|k| k != "_meta") {
                return Some(rpc_error(id, -32602, "Invalid list parameters"));
            }
            json!({"tools":tools()})
        }
        "tools/call" => {
            if params
                .as_object()
                .unwrap()
                .keys()
                .any(|k| !matches!(k.as_str(), "name" | "arguments" | "_meta"))
            {
                return Some(rpc_error(id, -32602, "Invalid call parameters"));
            }
            let Some(name) = params["name"].as_str() else {
                return Some(rpc_error(id, -32602, "Tool name required"));
            };
            let Some(tool) = tools().into_iter().find(|tool| tool["name"] == name) else {
                return Some(rpc_error(id, -32602, "Unknown tool"));
            };
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            if !valid_arguments(&args, &tool["inputSchema"])
                || (name == "paste" && args["text"].as_str().is_some_and(|s| s.len() > 65536))
            {
                tool_error("invalid_request")
            } else {
                let version = if matches!(
                    name,
                    "input_state" | "wait_input" | "screen" | "send_key" | "paste" | "events"
                ) {
                    4
                } else {
                    1
                };
                // A fresh agent connection avoids idle expiry without replaying
                // an operation that might already have executed.
                match crate::session::connect_agent_or_start(socket)
                    .map_err(|_| "transport_error".to_string())
                    .and_then(|client| client.exchange(version, name, &args).map_err(|e| e.code))
                {
                    Ok(result) => {
                        json!({"content":[{"type":"text","text":result.to_string()}],"isError":false})
                    }
                    // Keep stable socket codes, but never echo request-bearing diagnostics.
                    Err(code) => tool_error(&code),
                }
            }
        }
        _ => return Some(rpc_error(id, -32601, "Method not found")),
    };
    Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
}

fn tool_error(code: &str) -> Value {
    let text = json!({"error":{"code":code,"message":"Agent operation failed"}}).to_string();
    json!({"content":[{"type":"text","text":text}],"isError":true})
}

fn integer(min: u64, max: u64) -> Value {
    json!({"type":"integer","minimum":min,"maximum":max})
}
fn string() -> Value {
    json!({"type":"string"})
}
fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({"name":name,"description":description,"inputSchema":{
        "type":"object","properties":properties,"required":required,"additionalProperties":false}})
}

fn tools() -> Vec<Value> {
    let timeout = integer(0, 30000);
    vec![
        tool(
            "open_tab",
            "Create a terminal (agent access defaults to false).",
            json!({
            "rows":integer(1,1000),"cols":integer(1,1000),"allow_agent":{"type":"boolean"}}),
            &[],
        ),
        tool(
            "run",
            "Run explicit argv in an allowed terminal, without shell interpolation.",
            json!({
            "tab_id":string(),"argv":{"type":"array","items":string(),"minItems":1},"cwd":string()}),
            &["tab_id", "argv", "cwd"],
        ),
        tool(
            "wait",
            "Wait for drained command completion, at most 30 seconds.",
            json!({
            "tab_id":string(),"command_id":string(),"timeout_ms":timeout}),
            &["tab_id", "command_id", "timeout_ms"],
        ),
        tool(
            "get_output",
            "Read masked PTY bytes using original byte offsets.",
            json!({
            "tab_id":string(),"command_id":string(),"offset":integer(0,u64::MAX),"limit":integer(1,65536),
            "encoding":{"type":"string","enum":["utf8","base64"]}}),
            &["tab_id", "command_id", "offset", "limit", "encoding"],
        ),
        tool(
            "list_blocks",
            "Page masked OSC 133 command blocks.",
            json!({
            "tab_id":string(),"command_id":{"type":["string","null"]},
            "after":{"type":["string","null"]},"limit":integer(1,64)}),
            &["tab_id", "limit"],
        ),
        tool(
            "list_tabs",
            "List only agent-authorized terminals.",
            json!({}),
            &[],
        ),
        tool(
            "close_tab",
            "Close an allowed terminal and terminate its processes.",
            json!({"tab_id":string()}),
            &["tab_id"],
        ),
        tool(
            "input_state",
            "Query observed input-wait state.",
            json!({"tab_id":string()}),
            &["tab_id"],
        ),
        tool(
            "wait_input",
            "Wait for input readiness, at most 30 seconds.",
            json!({
            "tab_id":string(),"timeout_ms":timeout}),
            &["tab_id", "timeout_ms"],
        ),
        tool(
            "screen",
            "Read masked screen text and paged history.",
            json!({
            "tab_id":string(),"history_offset":integer(0,u64::MAX),"history_limit":integer(1,64)}),
            &["tab_id"],
        ),
        tool(
            "send_key",
            "Send a named key using the terminal's current modes.",
            json!({
            "tab_id":string(),"key":{"type":"string","enum":["Enter","Ctrl-C","Up","Down","Left","Right","Escape","Tab","Backspace","Home","End","Delete","PageUp","PageDown"]}}),
            &["tab_id", "key"],
        ),
        tool(
            "paste",
            "Paste UTF-8 text (at most 64 KiB), bracketed when enabled.",
            json!({
            "tab_id":string(),"text":{"type":"string","maxLength":65536}}),
            &["tab_id", "text"],
        ),
        tool(
            "events",
            "Replay or long-poll permission-filtered masked terminal events.",
            json!({
            "after":integer(0,u64::MAX),"limit":integer(1,128),"timeout_ms":timeout,"tab_id":string()}),
            &["after"],
        ),
    ]
}

// Validate the deliberately small schema vocabulary used above; core also validates
// semantic constraints (dimensions, filesystem paths, NULs and permissions).
fn valid_arguments(value: &Value, schema: &Value) -> bool {
    let type_matches = |kind: &str| match kind {
        "object" => value.is_object(),
        "string" => value.is_string(),
        "integer" => value.is_u64(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "null" => value.is_null(),
        _ => false,
    };
    let valid_type = match &schema["type"] {
        Value::String(kind) => type_matches(kind),
        Value::Array(kinds) => kinds
            .iter()
            .any(|kind| kind.as_str().is_some_and(type_matches)),
        _ => false,
    };
    if !valid_type {
        return false;
    }
    if let Some(choices) = schema["enum"].as_array() {
        if !choices.contains(value) {
            return false;
        }
    }
    if let Some(n) = value.as_u64() {
        if schema["minimum"].as_u64().is_some_and(|min| n < min)
            || schema["maximum"].as_u64().is_some_and(|max| n > max)
        {
            return false;
        }
    }
    if let Some(text) = value.as_str() {
        if schema["maxLength"]
            .as_u64()
            .is_some_and(|max| text.chars().count() as u64 > max)
        {
            return false;
        }
    }
    if let Some(items) = value.as_array() {
        if schema["minItems"]
            .as_u64()
            .is_some_and(|min| (items.len() as u64) < min)
            || items
                .iter()
                .any(|item| !valid_arguments(item, &schema["items"]))
        {
            return false;
        }
    }
    if let Some(object) = value.as_object() {
        if schema["required"].as_array().is_some_and(|keys| {
            keys.iter()
                .any(|key| !object.contains_key(key.as_str().unwrap()))
        }) {
            return false;
        }
        let Some(properties) = schema["properties"].as_object() else {
            return false;
        };
        if object
            .iter()
            .any(|(key, v)| properties.get(key).is_none_or(|s| !valid_arguments(v, s)))
        {
            return false;
        }
    }
    true
}
