//! Bounded JSON-lines transport for the local agent protocol.
use clanky_core::{
    Engine,
    platform::{self, LocalListener, LocalStream},
};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    io::{self, BufRead, BufReader, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const MAX_LINE: usize = 1024 * 1024;
const MAX_IDS: usize = 4096;
static NEXT_CONNECTION: AtomicU64 = AtomicU64::new(1);
const MAX_REPLY: usize = 128 * 1024 * 1024;
struct ConnectionGuard { engine: Engine, connection: u64 }
impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.engine.agent_connection_closed(self.connection);
        self.engine.clear_window_focus(self.connection);
    }
}
mod client;
pub use client::Client;
#[cfg(test)]
mod status_tests;

/// Serve v1 terminal operations and additive v2 actions until `stop` is set.
/// Platform owns socket security/cleanup.
pub fn serve(engine: Engine, path: &Path, stop: Arc<AtomicBool>) -> io::Result<()> {
    serve_inner(engine, path, stop, None)
}

pub fn serve_owned(
    engine: Engine,
    path: &Path,
    stop: Arc<AtomicBool>,
    owner_token: String,
) -> io::Result<()> {
    engine
        .add_mask_literal("CLANKY_OWNER_TOKEN", &owner_token)
        .map_err(|_| io::Error::other("Mask policy initialization failed"))?;
    serve_inner(engine, path, stop, Some(Arc::new(owner_token)))
}

fn serve_inner(
    engine: Engine,
    path: &Path,
    stop: Arc<AtomicBool>,
    token: Option<Arc<String>>,
) -> io::Result<()> {
    let listener = LocalListener::bind(path)?;
    let started = Instant::now();
    let mut empty_since = None;
    let mut clients: Vec<(LocalStream, thread::JoinHandle<()>)> = Vec::new();
    let result = loop {
        let mut i = 0;
        while i < clients.len() {
            if clients[i].1.is_finished() {
                let (_, handle) = clients.swap_remove(i);
                let _ = handle.join();
            } else {
                i += 1;
            }
        }
        if token.is_some() {
            if engine.idle_exit_ready() {
                let since = empty_since.get_or_insert_with(Instant::now);
                if since.elapsed() >= Duration::from_millis(200) {
                    break Ok(());
                }
            } else {
                empty_since = None;
                if engine.tab_ids().is_empty() && started.elapsed() >= Duration::from_secs(30) {
                    break Ok(());
                }
            }
        }
        if stop.load(Ordering::Acquire) || (token.is_some() && platform::shutdown_requested()) {
            break Ok(());
        }
        match listener.accept() {
            Ok(stream) => {
                if clients.len() >= 32 {
                    let _ = stream.shutdown();
                    continue;
                }
                let setup = (|| {
                    stream.set_read_timeout(Some(Duration::from_millis(250)))?;
                    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
                    stream.try_clone()
                })();
                let control = match setup {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                // Failure to prove ancestry is a permanent agent classification.
                let eligible = platform::owner_eligible(&stream).unwrap_or(false);
                let peer = match platform::agent_peer(&stream) {
                    Ok(peer) => peer,
                    Err(_) => {
                        let _ = stream.shutdown();
                        continue;
                    }
                };
                let connection_id = NEXT_CONNECTION.fetch_add(1, Ordering::Relaxed);
                let token = token.clone();
                let engine = engine.clone();
                let stop = stop.clone();
                match thread::Builder::new()
                    .name("agent-client".into())
                    .spawn(move || {
                        client(stream, engine, stop, token, eligible, peer, connection_id)
                    }) {
                    Ok(handle) => clients.push((control, handle)),
                    Err(err) => break Err(err),
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => break Err(err),
        }
    };
    if token.is_some() {
        stop.store(true, Ordering::Release);
        engine.shutdown();
    }
    for (stream, _) in &clients {
        let _ = stream.shutdown();
    }
    // Core wait operations themselves have a maximum 30-second deadline.
    for (_, handle) in clients {
        let _ = handle.join();
    }
    result
}

fn error(id: Value, code: &str, message: &str) -> Value {
    json!({"id":id,"ok":false,"error":{"code":code,"message":message}})
}

fn send(stream: &mut LocalStream, value: &Value) -> io::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_REPLY {
        return Err(io::Error::other("reply exceeds limit"));
    }
    stream.write_all(&bytes)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn client(
    stream: LocalStream,
    engine: Engine,
    stop: Arc<AtomicBool>,
    token: Option<Arc<String>>,
    eligible: bool,
    peer: platform::AgentPeer,
    connection_id: u64,
) {
    let connection = connection_id;
    let _connection_guard = ConnectionGuard { engine: engine.clone(), connection };
    engine.agent_connection_role(connection, &peer, false);
    let mut reader = BufReader::new(stream);
    let mut ids = HashSet::new();
    let mut owner = false;
    let mut eligible = eligible;
    let mut last_id = None;
    while !stop.load(Ordering::Acquire) && (owner || ids.len() < MAX_IDS) {
        let mut deadline = Instant::now() + Duration::from_secs(30);
        let mut line = Vec::new();
        loop {
            if owner && line.is_empty() {
                deadline = Instant::now() + Duration::from_secs(30);
            }
            if stop.load(Ordering::Acquire) || Instant::now() >= deadline {
                return;
            }
            match reader.fill_buf() {
                Ok([]) => return,
                Ok(bytes) => {
                    let n = bytes
                        .iter()
                        .position(|b| *b == b'\n')
                        .map_or(bytes.len(), |i| i + 1);
                    if line.len() + n > MAX_LINE {
                        let _ = send(
                            reader.get_mut(),
                            &error(Value::Null, "invalid_request", "Line exceeds 1 MiB"),
                        );
                        return;
                    }
                    let complete = bytes[n - 1] == b'\n';
                    line.extend_from_slice(&bytes[..n]);
                    reader.consume(n);
                    if complete {
                        break;
                    }
                }
                Err(err)
                    if matches!(
                        err.kind(),
                        io::ErrorKind::WouldBlock
                            | io::ErrorKind::TimedOut
                            | io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                Err(_) => return,
            }
        }
        let response = match serde_json::from_slice::<Value>(&line) {
            Ok(request) => {
                let was_owner = owner;
                engine.agent_connection_busy(connection, true);
                let mut response = dispatch(
                    &engine,
                    &request,
                    connection,
                    &mut ids,
                    &mut owner,
                    &mut eligible,
                    &mut last_id,
                    token.as_deref().map(String::as_str),
                    reader.get_ref(),
                );
                if was_owner != owner {
                    engine.agent_connection_role(connection, &peer, owner);
                }
                engine.agent_connection_busy(connection, false);
                if !was_owner && !owner {
                    // Only successful runs supply a real, generated command ID.
                    let version = request["v"].as_u64().unwrap_or(0);
                    let op = request["op"].as_str().unwrap_or("");
                    if response["ok"]==true && !(version==3 && matches!(op,"hello"|"auth")) {engine.agent_connection_announced(connection);}
                    if response["ok"] == true && ((version == 1 && op == "run") || (version == 2 && op == "rerun")) {
                        let tab = if op == "rerun" { &response["result"]["tab_id"] } else { &request["tab_id"] };
                        if let (Some(tab), Some(command)) = (tab.as_str(), response["result"]["command_id"].as_str()) {
                            engine.agent_run_bound(connection, peer.pid, tab, command);
                        }
                    }
                    if let Some(message) = response["error"]["message"].as_str() {
                        let masked = engine.mask_message(message);
                        response["error"]["message"] = json!(masked);
                    }
                    let version = request["v"].as_u64().unwrap_or(0);
                    let op = request["op"].as_str().unwrap_or("invalid_request");
                    let code = if response["ok"] == true {
                        "ok"
                    } else {
                        response["error"]["code"].as_str().unwrap_or("error")
                    };
                    if op == "open_tab" && response["ok"] == true {
                        let mut args = request.clone();
                        args["tab_id"] = response["result"]["tab_id"].clone();
                        engine.record_agent(&peer, connection_id, version, op, &args, code);
                    } else {
                        engine.record_agent(&peer, connection_id, version, op, &request, code);
                    }
                }
                response
            }
            Err(_) => error(
                Value::Null,
                "invalid_request",
                "Expected a UTF-8 JSON object",
            ),
        };
        if send(reader.get_mut(), &response).is_err() {
            return;
        }
    }
}

fn dispatch(
    engine: &Engine,
    request: &Value,
    connection: u64,
    ids: &mut HashSet<String>,
    owner: &mut bool,
    eligible: &mut bool,
    last_id: &mut Option<u64>,
    token: Option<&str>,
    stream: &LocalStream,
) -> Value {
    let id = request
        .get("id")
        .filter(|v| v.is_string())
        .cloned()
        .unwrap_or(Value::Null);
    let Some(obj) = request.as_object() else {
        return error(id, "invalid_request", "Expected an object");
    };
    let Some(id_str) = id.as_str().filter(|s| !s.is_empty() && s.len() <= 256) else {
        return error(
            Value::Null,
            "invalid_request",
            "id must be a nonempty string of at most 256 bytes",
        );
    };
    if *owner {
        let numeric = id_str
            .parse::<u64>()
            .ok()
            .filter(|n| n.to_string() == id_str);
        if numeric.is_none() || last_id.is_some_and(|last| numeric.unwrap() <= last) {
            return error(
                id,
                "invalid_request",
                "Owner ids must increase as decimal u64",
            );
        }
        *last_id = numeric;
    } else if !ids.insert(id_str.to_owned()) {
        return error(id, "invalid_request", "Duplicate connection request id");
    }
    if !obj.get("v").is_some_and(|v| v.is_u64()) {
        return error(id, "invalid_request", "v must be an integer");
    }
    let version = obj["v"].as_u64().unwrap();
    if !matches!(version, 1 | 2 | 3 | 4) {
        return error(
            id,
            "unsupported_version",
            "Supported protocol versions: 1, 2, 3, 4",
        );
    }
    let Some(op) = obj.get("op").and_then(Value::as_str) else {
        return error(id, "invalid_request", "op must be a string");
    };
    if op == "hello" {
        if obj.keys().any(|k| !matches!(k.as_str(), "v" | "id" | "op" | "client_name" | "model")) {
            return error(id, "invalid_request", "Unknown hello field");
        }
        for field in ["client_name", "model"] {
            if let Some(value)=obj.get(field) {
                if !value.as_str().is_some_and(|s| !s.trim().is_empty() && s.len()<=128 && !s.chars().any(char::is_control)) {
                    return error(id,"invalid_request","Hello labels must be nonempty strings of at most 128 bytes without controls");
                }
            }
        }
        if !*owner {
            engine.agent_connection_identity(connection,obj.get("client_name").and_then(Value::as_str),obj.get("model").and_then(Value::as_str));
        }
        if version == 4 {
            return json!({"id":id,"ok":true,"result":{"version":4,"capabilities":["agent_control","events","masked_reads","audit","revoke"]}});
        }
        if version == 3 {
            return json!({"id":id,"ok":true,"result":{"version":3,"server_pid":std::process::id(),"capabilities":["sessions","owner"]}});
        }
        if version == 2 {
            return json!({"id":id,"ok":true,"result":{"version":2,"capabilities":["block_actions","tasks","palette"]}});
        }
        return json!({"id":id,"ok":true,"result":{"version":1,"capabilities":["tabs","commands","blocks"]}});
    }
    if op.starts_with("v2.") || op.starts_with("v3.") || op.starts_with("v4.") {
        return error(id, "unknown_operation", "Use an explicit protocol version");
    }
    if version == 4 {
        return match engine.agent_request(op, request, *owner) {
            Ok(result) => json!({"id":id,"ok":true,"result":result}),
            Err(err) => error(id, &err.code, &err.message),
        };
    }
    if version == 3 {
        if op == "auth" {
            *eligible &= platform::owner_eligible(stream).unwrap_or(false);
            if !*eligible {
                *owner = false;
            }
            if obj
                .keys()
                .any(|k| !matches!(k.as_str(), "v" | "id" | "op" | "token"))
            {
                return error(id, "invalid_request", "Unknown auth field");
            }
            let supplied = obj.get("token").and_then(Value::as_str).unwrap_or("");
            // Compare the full capability without reporting any credential detail.
            let valid = token.is_some_and(|expected| {
                let mut diff = expected.len() ^ supplied.len();
                for (a, b) in expected.bytes().zip(supplied.bytes()) {
                    diff |= usize::from(a ^ b);
                }
                diff == 0
            });
            if !*eligible || !valid {
                return error(id, "permission_denied", "Owner authentication denied");
            }
            let Some(n) = id_str
                .parse::<u64>()
                .ok()
                .filter(|n| n.to_string() == id_str)
            else {
                return error(id, "invalid_request", "Owner ids must be decimal u64");
            };
            *owner = true;
            *last_id = Some(n);
            ids.clear();
            return json!({"id":id,"ok":true,"result":{"authenticated":true}});
        }
        if !*owner {
            return error(id, "permission_denied", "Owner authentication required");
        }
        if op == "focus_set" {
            if obj.keys().any(|k| !matches!(k.as_str(), "v" | "id" | "op" | "tab_id")) {
                return error(id, "invalid_request", "Unknown focus field");
            }
            let tab = match obj.get("tab_id") {
                Some(Value::String(tab)) if !tab.is_empty() => Some(tab.as_str()),
                Some(Value::Null) => None,
                _ => return error(id, "invalid_request", "tab_id must be a string or null"),
            };
            return match engine.set_window_focus(connection, tab) {
                Ok(()) => json!({"id":id,"ok":true,"result":{}}),
                Err(err) => error(id, &err.code, &err.message),
            };
        }
        return match engine.owner_request(op, &request) {
            Ok(result) => json!({"id":id,"ok":true,"result":result}),
            Err(err) => error(id, &err.code, &err.message),
        };
    }
    // Namespaced core actions are exposed only by the explicit v2 envelope.
    if version == 1 && op.starts_with("v2.") {
        return error(
            id,
            "unknown_operation",
            "Action operations require protocol v2",
        );
    }
    let action;
    let op = if version == 2 {
        action = format!("v2.{op}");
        action.as_str()
    } else {
        op
    };
    match engine.request(op, &request, *owner) {
        Ok(result) => json!({"id":id,"ok":true,"result":result}),
        Err(err) => error(id, &err.code, &err.message),
    }
}
