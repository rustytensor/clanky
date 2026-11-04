//! Real-server probes used by the fixed session-polish gate.
use clanky::notifications::NotificationTracker;
use clanky_api::Client;
use clanky_core::layout::Layout;
use serde_json::{json, Value};
use std::{io::{BufRead, BufReader}, path::{Path, PathBuf}, process::{Child, Command, Stdio}, sync::mpsc, thread, time::{Duration, Instant}};

struct Server { child: Child, client: Option<Client> }
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            if let Ok(workspace) = client.workspace_get() {
                for id in workspace.groups.iter().flat_map(|g| g.layout.leaves()) {
                    let _ = client.request("close_tab", &json!({"tab_id":id}), true);
                }
            }
            drop(client);
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) { return; }
            thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn server(binary: &Path, socket: &Path) -> Result<Server, String> {
    let child = Command::new(binary).arg("--server").arg("--socket").arg(socket)
        .env("SHELL", "/bin/sh").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map_err(|e| e.to_string())?;
    let mut server = Server { child, client: None };
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(client) = Client::connect_owner(socket) { server.client = Some(client); return Ok(server); }
        if server.child.try_wait().map_err(|e| e.to_string())?.is_some() || Instant::now() >= deadline {
            return Err("private server failed to start".into());
        }
        thread::sleep(Duration::from_millis(20));
    }
}
fn call(client: &Client, op: &str, args: Value) -> Result<Value, String> {
    client.request(op, &args, true).map_err(|e| e.to_string())
}
fn notify() -> Result<(), String> {
    let root = PathBuf::from(std::env::var_os("CAPTURE").ok_or("missing private capture path")?).parent().ok_or("invalid capture")?.to_path_buf();
    let binary = PathBuf::from(std::env::var_os("CLANKY_GATE_BINARY").ok_or("missing gate binary")?);
    let server = server(&binary, &root.join("notify.sock"))?;
    let client = server.client.as_ref().ok_or("missing client")?;
    let mut ids = Vec::new();
    for _ in 0..4 {
        ids.push(call(client, "open_tab", json!({"allow_agent":false}))?["tab_id"].as_str().ok_or("missing ID")?.to_owned());
    }
    let mut tracker = NotificationTracker::new();
    let mut disabled = NotificationTracker::new();
    tracker.poll_client(client, &ids[..3], 10, true, Some(&ids[0]))?;
    disabled.poll_client(client, &ids[3..], 0, true, Some(&ids[0]))?;
    call(client, "v3.focus_set", json!({"tab_id":ids[0]}))?;
    let mut commands = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        let delay = if i == 2 { "0.1" } else { "10.2" };
        let code = if i == 0 { 9 } else { 7 };
        let script = format!("printf '\\033]133;C\\007'; sleep {delay}; printf '\\033]133;D;{code}\\007'; exit {code}");
        let result = call(client, "run", json!({"tab_id":id,"argv":["/bin/sh","-c",script],"cwd":"/"}))?;
        commands.push(result["command_id"].as_str().ok_or("missing command ID")?.to_owned());
    }
    let deadline = Instant::now() + Duration::from_secs(18);
    loop {
        let mut done = true;
        for (id, command) in ids.iter().zip(&commands) {
            done &= call(client, "get_exit_code", json!({"tab_id":id,"command_id":command}))?["done"] == true;
        }
        if done { break; }
        if Instant::now() >= deadline { return Err("notification commands timed out".into()); }
        thread::sleep(Duration::from_millis(100));
    }
    // Change focus after both long completions, before observing either one.
    call(client, "v3.focus_set", json!({"tab_id":ids[1]}))?;
    tracker.poll_client(client, &ids[..3], 10, true, Some(&ids[1]))?;
    disabled.poll_client(client, &ids[3..], 0, true, Some(&ids[1]))?;
    thread::sleep(Duration::from_millis(500));
    Ok(())
}
struct OffscreenWindows { socket: PathBuf, children: Vec<Child>, client: Option<Client> }
impl Drop for OffscreenWindows {
    fn drop(&mut self) {
        let client = self.client.take().or_else(|| Client::connect_owner(&self.socket).ok());
        if let Some(client) = client {
            if let Ok(workspace) = client.workspace_get() {
                let mut ids: Vec<_> = workspace.groups.iter().flat_map(|g| g.layout.leaves()).collect();
                // A never-used server exits only after the startup grace; create and close
                // one private empty tab so cleanup does not leave it running.
                if ids.is_empty() {
                    if let Ok(tab) = client.request("open_tab", &json!({}), true) {
                        if let Some(id) = tab["tab_id"].as_str() { ids.push(id.to_owned()); }
                    }
                }
                for id in ids { let _ = client.request("close_tab", &json!({"tab_id":id}), true); }
            }
            drop(client);
        }
        for child in &mut self.children {
            child.stdin.take();
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if matches!(child.try_wait(), Ok(Some(_))) { break; }
                if Instant::now() >= deadline { let _ = child.kill(); let _ = child.wait(); break; }
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
impl OffscreenWindows {
    fn start(&mut self, binary: &Path, layout: &Path) -> Result<(), String> {
        let mut child = Command::new(binary).arg("--polish-offscreen-window").arg(&self.socket).arg(layout)
            .env("CLANKY_GATE_OFFSCREEN", "1").env("SHELL", "/bin/sh")
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
            .spawn().map_err(|e| e.to_string())?;
        let stdout = child.stdout.take().ok_or("missing offscreen output")?;
        self.children.push(child);
        let (tx, rx) = mpsc::sync_channel(1);
        thread::spawn(move || { let mut line = String::new(); let _ = BufReader::new(stdout).read_line(&mut line); let _ = tx.send(line); });
        if rx.recv_timeout(Duration::from_secs(12)).map_err(|_| "offscreen startup timed out")? != "READY\n" {
            return Err("offscreen startup failed".into());
        }
        Ok(())
    }
}
fn restore(layout: &Path, root: &Path, binary: &Path) -> Result<(), String> {
    let mut windows = OffscreenWindows { socket: root.join("restore.sock"), children: Vec::new(), client: None };
    windows.start(binary, layout)?;
    windows.client = Some(Client::connect_owner(&windows.socket).map_err(|e| e.to_string())?);
    let client = windows.client.as_ref().ok_or("missing client")?;
    let workspace = client.workspace_get().map_err(|e| e.to_string())?;
    if workspace.groups.len() != 1 { return Err("expected one restored group".into()); }
    let group = &workspace.groups[0];
    let axis = match &group.layout { Layout::Split { axis, .. } => format!("{axis:?}"), _ => return Err("missing split".into()) };
    let ids = group.layout.leaves();
    if ids.len() != 2 || ids.iter().any(|id| id == "old1" || id == "old2") { return Err("invalid fresh IDs".into()); }
    let focus = ids.iter().position(|id| id == &group.focus).ok_or("missing focus")?;
    let shell = std::fs::canonicalize("/bin/sh").map_err(|e| e.to_string())?;
    let mut terminals = Vec::new();
    let mut fresh_shells = true;
    for id in &ids {
        let info = call(client, "v3.terminal_info", json!({"tab_id":id}))?;
        let exe = std::fs::canonicalize(info["exe"].as_str().ok_or("missing executable")?).map_err(|e| e.to_string())?;
        fresh_shells &= exe == shell;
        terminals.push(json!({"cwd":info["cwd"],"rows":info["rows"],"cols":info["cols"]}));
    }
    let before = serde_json::to_value(&workspace).map_err(|e| e.to_string())?;
    windows.start(binary, layout)?;
    let after = serde_json::to_value(windows.client.as_ref().ok_or("missing client")?.workspace_get().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let result = json!({"active":workspace.active,"axis":axis,"focus":focus,"terminals":terminals,
        "fresh_shells":fresh_shells,"same_server_no_duplicate":before == after,
        "old_command_ran":root.join("old-command-executed").exists()});
    println!("{result}");
    Ok(())
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [mode] if mode == "notify" => notify(),
        [mode, layout, root, binary] if mode == "restore" => restore(Path::new(layout), Path::new(root), Path::new(binary)),
        _ => Err("invalid gate arguments".into()),
    };
    if let Err(error) = result { eprintln!("FAIL: {error}"); std::process::exit(1); }
}
