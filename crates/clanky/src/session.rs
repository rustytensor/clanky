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

fn connect_start_with(socket: &Path, owner: bool) -> io::Result<(Client, bool)> {
    connect_start_configured(socket, owner, None, false)
}
fn connect_start_configured(socket: &Path, owner: bool, config: Option<&Path>, post_ai: bool) -> io::Result<(Client, bool)> {
    let connect: fn(&Path) -> io::Result<Client> = if owner {
        Client::connect_owner
    } else {
        Client::connect_agent
    };
    match connect(socket) {
        Ok(client) => return Ok((client, false)),
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
    let mut child = platform::spawn_server_configured(&exe, socket, config, post_ai)?;
    let spawned_pid = child.id();
    // A competing launcher may lose the instance flock and exit. Both
    // launchers connect to the winner, and every launcher child is reaped.
    thread::spawn(move || {
        let _ = child.wait();
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match connect(socket) {
            Ok(client) => {
                if !owner {
                    return Ok((client, false));
                }
                let hello = client.exchange(3, "hello", &json!({}))
                    .map_err(|e| io::Error::other(e.message))?;
                return Ok((client, hello["server_pid"].as_u64() == Some(u64::from(spawned_pid))));
            }
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

fn connect_start(socket: &Path) -> io::Result<(Client, bool)> {
    connect_start_with(socket, true)
}

/// Agent-only startup: no owner token is read or authentication attempted.
pub fn connect_agent_or_start(socket: &Path) -> Result<Client, String> {
    let (client, _) = connect_start_with(socket, false)
        .map_err(|_| "cannot connect agent session".to_string())?;
    client.exchange(4, "hello", &json!({})).map_err(|error| {
        if error.message == clanky_core::POST_AI_REASON { error.message }
        else { "cannot connect agent session".to_owned() }
    })?;
    Ok(client)
}

pub fn connect_or_start(socket: &Path) -> Result<Client, String> {
    connect_or_start_with_status(socket).map(|(client, _)| client)
}

pub fn connect_or_start_with_status(socket: &Path) -> Result<(Client, bool), String> {
    connect_start(socket).map_err(|error| format!("cannot connect to session server: {error}"))
}

/// Shared startup path for graphical and offscreen windows.
pub fn prepare_window(socket: &Path, layout: Option<&Path>) -> Result<Client, String> {
    prepare_window_configured(socket, layout, None, crate::config::StartupOverrides::default())
}
pub fn prepare_window_configured(socket: &Path, layout: Option<&Path>, config: Option<&Path>, flags: crate::config::StartupOverrides) -> Result<Client, String> {
    let guard = platform::StartupLock::acquire(socket).map_err(|e| format!("cannot lock window startup: {e}"))?;
    let (client, fresh) = connect_start_configured(socket, true, config, flags.post_ai)
        .map_err(|e| format!("cannot connect to session server: {e}"))?;
    let mut policy = crate::config::Config::load(config)?;
    flags.apply(&mut policy);
    // Enforce before restoring any shells, not only once the window is ready.
    client.exchange(4, "post_ai", &json!({"enabled":policy.post_ai,"force":flags.post_ai})).map_err(|e| e.message)?;
    if let Some(path) = layout { crate::layout_store::restore(&client, path, fresh)?; }
    drop(guard);
    Ok(client)
}

/// A window attached to a session server that runs another executable (an older
/// build kept alive by its shells) gets that server's fixes and bugs, not its own.
/// Returns a status line for the window, or None when both run the same file.
pub fn server_build_notice(client: &Client) -> Option<String> {
    let hello = client.exchange(3, "hello", &json!({})).ok()?;
    let ours = platform::executable_identity()?;
    let build = &hello["build"];
    if build["executable"].as_str() == Some(ours.as_str()) { return None; }
    let version = build["version"].as_str().map_or_else(|| "an older build".to_owned(), |v| format!("another v{v} build"));
    Some(format!("Session server is {version}. Restart it to update; that closes its shells."))
}

pub fn run_server(socket: &Path) -> Result<(), String> {
    run_server_configured(socket, None, crate::config::StartupOverrides::default())
}
pub fn run_server_configured(socket: &Path, config_path: Option<&Path>, flags: crate::config::StartupOverrides) -> Result<(), String> {
    let mut config = crate::config::Config::load(config_path)?;
    flags.apply(&mut config);
    let settings_path = config_path.map(Path::to_owned).or_else(crate::platform::settings_path);
    let mut settings = settings_path.map(crate::settings::Settings::open).transpose()?;
    if let Some(settings) = &settings {
        config.post_ai = settings.config().post_ai;
        flags.apply(&mut config);
    }
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
    engine.0.configure_post_ai(config.post_ai, flags.post_ai);
    let stop = Arc::new(AtomicBool::new(false));
    let monitor_stop = stop.clone();
    let monitor_engine = engine.0.clone();
    let monitor = thread::spawn(move || {
        while !monitor_stop.load(std::sync::atomic::Ordering::Acquire) {
            if let Some(settings) = &mut settings {
                match settings.reload() {
                    Ok(true) => { monitor_engine.set_post_ai(settings.config().post_ai); }
                    Err(error) => eprintln!("clanky: {error}"),
                    _ => (),
                }
            }
            thread::park_timeout(Duration::from_millis(250));
        }
    });
    let result = clanky_api::serve_owned(engine.0.clone(), socket, stop.clone(), token)
        .map_err(|e| format!("session server failed: {e}"));
    stop.store(true, std::sync::atomic::Ordering::Release);
    monitor.thread().unpark();
    let _ = monitor.join();
    result
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
    stdio_configured(socket, None, crate::config::StartupOverrides::default())
}
pub fn stdio_configured(socket: &Path, config_path: Option<&Path>, flags: crate::config::StartupOverrides) -> Result<(), String> {
    let mut config = crate::config::Config::load(config_path)?;
    flags.apply(&mut config);
    let client = match connect_start_configured(socket, true, config_path, flags.post_ai) {
        Ok((client, _)) => {
            if config.post_ai {
                client.exchange(4, "post_ai", &json!({"enabled":true,"force":flags.post_ai})).map_err(|e| e.message)?;
            }
            client
        },
        // Inside a terminal, owner denial is expected. Preserve that role and
        // keep forwarding, including explicit auth requests carrying a token.
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            if config.post_ai { return Err(clanky_core::POST_AI_REASON.into()); }
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
