//! Headless real-process gate: expected policy errors and exit codes from M12.
use clanky_api::Client;
use clanky_core::{POST_AI_REASON, platform::LocalStream};
use serde_json::json;
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Server {
    child: Child,
    owner: Client,
    socket: PathBuf,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn start(exe: &Path, root: &Path, name: &str, config: &Path, forced: bool) -> Server {
    let socket = root.join(name).join("api.sock");
    let mut command = Command::new(exe);
    command
        .args(["--server", "--socket"])
        .arg(&socket)
        .arg("--config")
        .arg(config)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if forced {
        command.arg("--post-ai");
    }
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let owner = loop {
        if let Ok(client) = Client::connect_owner(&socket) {
            break client;
        }
        if child.try_wait().unwrap().is_some() || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("server did not accept owner authentication");
        }
        thread::sleep(Duration::from_millis(20));
    };
    Server {
        child,
        owner,
        socket,
    }
}
fn agent_denied(socket: &Path) {
    let agent = Client::connect_agent(socket).unwrap();
    let error = agent.exchange(4, "hello", &json!({})).unwrap_err();
    assert_eq!(error.code, "permission_denied");
    assert_eq!(error.message, POST_AI_REASON);
    // A legacy client cannot skip hello to gain access.
    let agent = Client::connect_agent(socket).unwrap();
    assert_eq!(
        agent
            .exchange(1, "list_tabs", &json!({}))
            .unwrap_err()
            .message,
        POST_AI_REASON
    );
}
fn mcp_denied(exe: &Path, socket: &Path, config: &Path, force: bool) {
    let mut command = Command::new(exe);
    command
        .arg("mcp")
        .arg("--socket")
        .arg(socket)
        .arg("--config")
        .arg(config)
        .stdin(Stdio::null());
    if force {
        command.arg("--post-ai");
    }
    let result = command.output().unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert_eq!(
        String::from_utf8(result.stderr).unwrap(),
        format!("{POST_AI_REASON}\n")
    );
}
fn main() {
    let target =
        PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").expect("CARGO_TARGET_DIR required"));
    let root = target.join(format!("postai-gate-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _scratch = Scratch(root.clone());
    let exe = std::env::current_exe().unwrap().with_file_name("clanky");
    let config = root.join("config.json");
    let disabled = root.join("disabled.json");
    fs::write(&config, "{\"post_ai\":false}").unwrap();
    fs::write(&disabled, "{\"post_ai\":false}").unwrap();
    {
        let server = start(&exe, &root, "live", &config, false);
        let tab = server
            .owner
            .exchange(1, "open_tab", &json!({"allow_agent":true}))
            .unwrap();
        let tab_id = tab["tab_id"].as_str().unwrap().to_owned();
        let agent = Client::connect_agent(&server.socket).unwrap();
        agent
            .exchange(4, "hello", &json!({"client_name":"waiting"}))
            .unwrap();
        let cursor = agent
            .exchange(4, "events", &json!({"after":0,"timeout_ms":0}))
            .unwrap()["next_cursor"]
            .clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let waiter = thread::spawn(move || {
            let result = agent.exchange(
                4,
                "events",
                &json!({"after":cursor,"timeout_ms":30000,"tab_id":tab_id}),
            );
            tx.send(result.is_err()).unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let state = server.owner.exchange(3, "look_state", &json!({})).unwrap();
            if state["agents"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["client_name"] == "waiting" && a["running"] == true)
            {
                break;
            }
            assert!(Instant::now() < deadline, "agent wait never became pending");
            thread::sleep(Duration::from_millis(10));
        }
        let mut idle = LocalStream::connect(&server.socket).unwrap();
        idle.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        idle.write_all(b"{\"v\":4,\"id\":\"idle\",\"op\":\"hello\"}\n")
            .unwrap();
        let mut idle = BufReader::new(idle);
        let mut line = String::new();
        idle.read_line(&mut line).unwrap();
        assert!(line.contains("\"ok\":true"));
        assert_eq!(
            server
                .owner
                .exchange(4, "post_ai", &json!({"enabled":true}))
                .unwrap()["post_ai"],
            true
        );
        assert!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            "pending wait was not cancelled"
        );
        waiter.join().unwrap();
        line.clear();
        assert_eq!(
            idle.read_line(&mut line).unwrap(),
            0,
            "idle agent socket was not disconnected"
        );
        if std::env::var_os("CLANKY_POSTAI_FAULT").is_some() {
            server
                .owner
                .exchange(4, "post_ai", &json!({"enabled":false}))
                .unwrap();
        }
        agent_denied(&server.socket);
        let owner = Client::connect_owner(&server.socket).unwrap();
        assert_eq!(
            owner.exchange(1, "list_tabs", &json!({})).unwrap()["tabs"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let fresh = owner.exchange(1, "open_tab", &json!({})).unwrap();
        owner
            .exchange(1, "close_tab", &json!({"tab_id":fresh["tab_id"]}))
            .unwrap();
        mcp_denied(&exe, &server.socket, &disabled, false);
        owner
            .exchange(4, "post_ai", &json!({"enabled":false}))
            .unwrap();
        let agent = Client::connect_agent(&server.socket).unwrap();
        agent.exchange(4, "hello", &json!({})).unwrap();
        assert!(agent.exchange(4, "resume_agents", &json!({})).is_err());
        // Disk change, with no window present, must switch the live server on.
        fs::write(&config, "{\"post_ai\":true}").unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let probe = Client::connect_agent(&server.socket).unwrap();
            if let Err(error) = probe.exchange(4, "hello", &json!({})) {
                assert_eq!(error.message, POST_AI_REASON);
                break;
            }
            assert!(Instant::now() < deadline, "server did not reload post_ai");
            thread::sleep(Duration::from_millis(20));
        }
        agent_denied(&server.socket);
    }
    {
        let server = start(&exe, &root, "configured", &config, false);
        agent_denied(&server.socket);
        mcp_denied(&exe, &server.socket, &config, false);
    }
    {
        let server = start(&exe, &root, "forced", &disabled, true);
        assert_eq!(
            server
                .owner
                .exchange(4, "post_ai", &json!({"enabled":false}))
                .unwrap()["post_ai"],
            true
        );
        agent_denied(&server.socket);
        mcp_denied(&exe, &server.socket, &disabled, true);
    }
    // Real theme bytes through the same timer-owned runtime used by App.
    let folder = root.join("themes");
    fs::create_dir(&folder).unwrap();
    let source = include_str!("../../../../assets/themes/stone-dark.json");
    fs::write(folder.join("custom.json"), source).unwrap();
    let store = clanky::theme_store::ThemeStore::open(&folder).unwrap();
    let mut runtime = clanky::theme_runtime::ThemeRuntime::from_store(store).unwrap();
    let config =
        clanky::config::Config::from_json(r#"{"theme":"dark","theme_dark":"user:custom"}"#)
            .unwrap();
    assert!(runtime.poll(&config, false));
    assert!(!runtime.poll(&config, false));
    fs::write(
        folder.join("custom.json"),
        source.replace("#1a1816", "#010203"),
    )
    .unwrap();
    assert!(runtime.poll(&config, false));
    assert_eq!(runtime.active().unwrap().colors.background, [1, 2, 3, 255]);
    assert!(!runtime.poll(&config, false));
    println!(
        "PASS: Post-AI hello/legacy refusal, owner access, disconnect/cancel, config reload, CLI forcing, MCP exit 2 and theme timer data"
    );
}
