//! Bounded JSON-lines transport for the local agent protocol.
use clanky_core::{
    Engine,
    platform::{LocalListener, LocalStream},
};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    io::{self, BufRead, BufReader, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const MAX_LINE: usize = 1024 * 1024;
const MAX_IDS: usize = 4096;

/// Serve v1 terminal operations and additive v2 actions until `stop` is set.
/// Platform owns socket security/cleanup.
pub fn serve(engine: Engine, path: &Path, stop: Arc<AtomicBool>) -> io::Result<()> {
    let listener = LocalListener::bind(path)?;
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
        if stop.load(Ordering::Acquire) {
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
                let engine = engine.clone();
                let stop = stop.clone();
                match thread::Builder::new()
                    .name("agent-client".into())
                    .spawn(move || client(stream, engine, stop))
                {
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
    serde_json::to_writer(&mut *stream, value)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn client(stream: LocalStream, engine: Engine, stop: Arc<AtomicBool>) {
    let mut reader = BufReader::new(stream);
    let mut ids = HashSet::new();
    while !stop.load(Ordering::Acquire) && ids.len() < MAX_IDS {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut line = Vec::new();
        loop {
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
            Ok(request) => dispatch(&engine, request, &mut ids),
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

fn dispatch(engine: &Engine, request: Value, ids: &mut HashSet<String>) -> Value {
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
    if !ids.insert(id_str.to_owned()) {
        return error(id, "invalid_request", "Duplicate connection request id");
    }
    if !obj.get("v").is_some_and(|v| v.is_u64()) {
        return error(id, "invalid_request", "v must be an integer");
    }
    let version = obj["v"].as_u64().unwrap();
    if !matches!(version, 1 | 2) {
        return error(
            id,
            "unsupported_version",
            "Supported protocol versions: 1, 2",
        );
    }
    let Some(op) = obj.get("op").and_then(Value::as_str) else {
        return error(id, "invalid_request", "op must be a string");
    };
    if op == "hello" {
        if obj.keys().any(|k| !matches!(k.as_str(), "v" | "id" | "op")) {
            return error(id, "invalid_request", "Unknown hello field");
        }
        if version == 2 {
            return json!({"id":id,"ok":true,"result":{"version":2,"capabilities":["block_actions","tasks","palette"]}});
        }
        return json!({"id":id,"ok":true,"result":{"version":1,"capabilities":["tabs","commands","blocks"]}});
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
    match engine.request(op, &request, false) {
        Ok(result) => json!({"id":id,"ok":true,"result":result}),
        Err(err) => error(id, &err.code, &err.message),
    }
}
