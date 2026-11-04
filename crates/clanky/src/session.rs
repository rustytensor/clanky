//! Server lifetime and bounded stdio bridging; windows own only clients.
use clanky_api::Client;
use clanky_core::{Engine, platform};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    io::{self, BufRead, BufReader, Write},
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    thread,
    time::{Duration, Instant},
};

fn connect_start(socket: &Path) -> io::Result<Client> {
    match Client::connect_owner(socket) {
        Ok(client) => return Ok(client),
        Err(error) => {
            // NotFound may mean a missing credential, not a missing socket.
            // Never launch over any existing endpoint, even a stale one.
            if error.kind() != io::ErrorKind::NotFound
                || !matches!(std::fs::symlink_metadata(socket), Err(e) if e.kind() == io::ErrorKind::NotFound)
            {
                return Err(error);
            }
        }
    }
    let exe = std::env::current_exe()?;
    let mut child = platform::spawn_server(&exe, socket)?;
    // A competing launcher may lose the instance flock and exit. Both
    // launchers connect to the winner, and every launcher child is reaped.
    thread::spawn(move || {
        let _ = child.wait();
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match Client::connect_owner(socket) {
            Ok(client) => return Ok(client),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) =>
            {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "server startup timed out",
                    ));
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(error),
        }
    }
}

pub fn connect_or_start(socket: &Path) -> Result<Client, String> {
    connect_start(socket).map_err(|error| format!("cannot connect to session server: {error}"))
}

pub fn run_server(socket: &Path) -> Result<(), String> {
    let _lock = platform::ServerLock::acquire(socket)
        .map_err(|e| format!("cannot lock session server: {e}"))?;
    let token = platform::owner_token(socket, true)
        .map_err(|_| "cannot load owner capability".to_string())?;
    platform::install_shutdown_signals()
        .map_err(|e| format!("cannot install shutdown signals: {e}"))?;
    struct Shutdown(Engine);
    impl Drop for Shutdown {
        fn drop(&mut self) {
            self.0.shutdown();
        }
    }
    let engine = Shutdown(Engine::new());
    clanky_api::serve_owned(
        engine.0.clone(),
        socket,
        Arc::new(AtomicBool::new(false)),
        token,
    )
    .map_err(|e| format!("session server failed: {e}"))
}

pub fn connect_pipe(socket: &Path) -> Result<Client, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let args = [
        OsString::from("--stdio"),
        OsString::from("--socket"),
        socket.as_os_str().to_os_string(),
    ];
    let child = platform::spawn_pipe(exe.as_os_str(), &args)
        .map_err(|e| format!("cannot launch stdio bridge: {e}"))?;
    Client::from_child(child).map_err(|e| format!("cannot connect stdio bridge: {e}"))
}

fn invalid(id: Value) -> Value {
    json!({"id":id,"ok":false,"error":{"code":"invalid_request","message":"Invalid bridge request"}})
}

// Incremental bounded framing, including malicious input with no newline.
fn read_line(reader: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    loop {
        let bytes = reader.fill_buf()?;
        if bytes.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(io::Error::other("incomplete stdio request"))
            };
        }
        let n = bytes
            .iter()
            .position(|b| *b == b'\n')
            .map_or(bytes.len(), |i| i + 1);
        if line.len() + n > 1024 * 1024 {
            return Err(io::Error::other("stdio request exceeds 1 MiB"));
        }
        let complete = bytes[n - 1] == b'\n';
        line.extend_from_slice(&bytes[..n]);
        reader.consume(n);
        if complete {
            return Ok(Some(line));
        }
    }
}

pub fn stdio(socket: &Path) -> Result<(), String> {
    let client = match connect_start(socket) {
        Ok(client) => client,
        // Inside a terminal, owner denial is expected. Preserve that role and
        // keep forwarding, including explicit auth requests carrying a token.
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            Client::connect_agent(socket).map_err(|_| "cannot connect agent bridge".to_string())?
        }
        Err(error) => return Err(format!("cannot connect stdio bridge: {error}")),
    };
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    let mut previous_id = None;
    while let Some(line) = read_line(&mut reader).map_err(|e| e.to_string())? {
        let response = match serde_json::from_slice::<Value>(&line) {
            Ok(mut request) => {
                let id = request.get("id").cloned().unwrap_or(Value::Null);
                let sequence = id
                    .as_str()
                    .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                    .and_then(|s| s.parse::<u64>().ok());
                let version = request.get("v").and_then(Value::as_u64);
                let op = request.get("op").and_then(Value::as_str).map(str::to_owned);
                if let (Some(sequence), Some(version), Some(op)) = (sequence, version, op) {
                    if previous_id.is_some_and(|old| sequence <= old) {
                        invalid(id)
                    } else {
                        previous_id = Some(sequence);
                        // Client assigns backend IDs. Never pass pipe IDs or
                        // internal handshake IDs into the backend connection.
                        let args = request.as_object_mut().unwrap();
                        args.remove("id");
                        args.remove("v");
                        args.remove("op");
                        match client.exchange(version, &op, &request) {
                            Ok(result) => json!({"id":id,"ok":true,"result":result}),
                            // Do not echo backend messages: in particular an
                            // invalid auth request must never expose its token.
                            Err(error) => {
                                json!({"id":id,"ok":false,"error":{"code":error.code,"message":"Request failed"}})
                            }
                        }
                    }
                } else {
                    invalid(id)
                }
            }
            Err(_) => invalid(Value::Null),
        };
        serde_json::to_writer(&mut writer, &response)
            .map_err(|_| "cannot write bridge reply".to_string())?;
        writer
            .write_all(b"\n")
            .and_then(|_| writer.flush())
            .map_err(|_| "cannot write bridge reply".to_string())?;
    }
    Ok(())
}
