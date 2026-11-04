//! Narrow real-shell expectations for the owner-only current-directory query.
use clanky_api::Client;
use clanky_core::platform;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Child,
    thread,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn require(ok: bool, label: &str) -> Result<()> {
    if !ok {
        return Err(label.into());
    }
    Ok(())
}

struct Fixture {
    server: Child,
    owner: Option<Client>,
    tabs: Vec<String>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            for tab in &self.tabs {
                let _ = owner.exchange(1, "close_tab", &json!({"tab_id":tab}));
            }
        }
        // SIGTERM lets the real server clean up its PTYs even after a broken
        // client connection. Reap only our saved child PID.
        let _ = platform::stop_server(&mut self.server);
    }
}

fn cwd(owner: &Client, tab: &str) -> Result<Value> {
    Ok(owner.exchange(3, "terminal_cwd", &json!({"tab_id":tab}))?)
}

fn ready(owner: &Client, tab: &str, marker: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let screen = owner.snapshot(tab)?;
        let text: String = screen.cells.iter().map(|c| c.ch).collect();
        if text.contains(marker) {
            return Ok(());
        }
        require(Instant::now() < deadline, "shell readiness deadline")?;
        thread::sleep(Duration::from_millis(20));
    }
}

fn report(owner: &Client, tab: &str, url: &str, marker: &str) -> Result<()> {
    // No shell expansion of URL bytes: single quotes are escaped explicitly.
    let quoted = url.replace('\'', "'\\''");
    owner.input(tab, format!("printf '\\033]7;%s\\007\\n%s\\n' '{quoted}' '{marker}'\n").as_bytes())?;
    ready(owner, tab, marker)
}

fn gate(root: &Path) -> Result<()> {
    let socket = root.join("server.sock");
    let work = root.join("work");
    let reported = root.join("reported β");
    std::fs::create_dir(&work)?;
    std::fs::create_dir(&reported)?;
    let server = platform::spawn_server(&std::env::current_exe()?, &socket)?;
    let mut fixture = Fixture { server, owner: None, tabs: Vec::new() };
    let deadline = Instant::now() + Duration::from_secs(10);
    let owner = loop {
        match Client::connect_owner(&socket) {
            Ok(owner) => break owner,
            Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused) => {
                require(Instant::now() < deadline, "server readiness deadline")?;
                require(fixture.server.try_wait()?.is_none(), "server exited before readiness")?;
                thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(e.into()),
        }
    };
    fixture.owner = Some(owner.clone());
    let tab = owner.exchange(1, "open_tab", &json!({"allow_agent":true}))?["tab_id"]
        .as_str().ok_or("missing tab")?.to_owned();
    fixture.tabs.push(tab.clone());
    require(cwd(&owner, &tab)? == json!({"cwd":null,"source":null}), "idle cwd must be null")?;
    let agent = Client::connect_agent(&socket)?;
    let denied = agent.exchange(3, "terminal_cwd", &json!({"tab_id":tab}))
        .err().ok_or("agent cwd unexpectedly allowed")?;
    require(denied.code == "permission_denied", "agent cwd must be denied")?;
    for v in [1, 2] {
        let denied = owner.exchange(v, "terminal_cwd", &json!({"tab_id":tab}))
            .err().ok_or("old-version cwd unexpectedly allowed")?;
        require(denied.code == "unknown_operation", "cwd must require v3")?;
    }
    // The child starts at root and actually changes directory. read is a shell
    // builtin, so the shell remains alive in work for every subsequent query.
    let command = owner.exchange(1, "run", &json!({"tab_id":tab,"cwd":root,"argv":[
        "/bin/sh", "-c",
        "stty -echo; cd work || exit 2; printf 'CWD_READY\\n'; while IFS= read -r line; do eval \"$line\"; done"
    ]}))?;
    ready(&owner, &tab, "CWD_READY")?;
    let mut observed = cwd(&owner, &tab)?;
    if std::env::var("CLANKY_GATE_FAULT").as_deref() == Ok("cwd") {
        observed["cwd"] = json!(root);
    }
    require(observed == json!({"cwd":work,"source":"proc"}), "real cd must return work from proc")?;

    let encoded: String = reported.to_str().ok_or("fixture path must be UTF-8")?
        .as_bytes().iter().map(|b| {
            if b.is_ascii_alphanumeric() || b"/-_.~".contains(b) {
                (*b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        }).collect();
    report(&owner, &tab, &format!("file://localhost{encoded}"), "LOCAL_READY")?;
    let expected = json!({"cwd":reported,"source":"osc7"});
    require(cwd(&owner, &tab)? == expected, "local OSC7 decoded override")?;
    report(&owner, &tab, "file://foreign.invalid/not-local", "FOREIGN_READY")?;
    require(cwd(&owner, &tab)? == expected, "foreign OSC7 must be ignored")?;
    report(&owner, &tab, "file:///bad%QQ", "INVALID_READY")?;
    require(cwd(&owner, &tab)? == expected, "invalid OSC7 must be ignored")?;
    require(owner.exchange(1, "get_exit_code", &json!({"tab_id":tab,"command_id":command["command_id"]}))?["done"] == false,
        "shell must stay alive throughout queries")?;
    Ok(())
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--server") {
        require(args.len() == 3 && args[1] == "--socket", "server arguments")?;
        return clanky::session::run_server(Path::new(&args[2])).map_err(Into::into);
    }
    require(args.len() == 1, "expected private fixture directory")?;
    let root = PathBuf::from(&args[0]).canonicalize()?;
    gate(&root)
}

fn main() {
    if let Err(error) = run() {
        eprintln!("FAIL: terminal cwd: {error}");
        std::process::exit(1);
    }
    println!("PASS: terminal cwd");
}
