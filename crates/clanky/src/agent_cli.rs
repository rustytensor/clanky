//! `clanky agent`: start an agent's own tool in a new pane of the running Clanky.
use crate::{agent_launch, config::Config};
use clanky_api::Client;
use std::{ffi::OsString, path::{Path, PathBuf}, time::{Duration, Instant}};

const USAGE: &str = "clanky agent [--socket PATH] [--config PATH] [--tab] NAME [DIR]
clanky agent [--config PATH] --list";

enum Failure {
    /// Exit code 1.
    Error(String),
    /// Exit code 2: bad arguments or Post-AI mode.
    Usage(String),
    PostAi,
}
use Failure::{Error, PostAi, Usage};

/// Runs `clanky agent ARGS` and returns the process exit code.
pub fn main(args: &[String]) -> i32 {
    match run(args) {
        Ok(()) => 0,
        Err(Error(message)) => { eprintln!("clanky agent: {message}"); 1 }
        Err(Usage(message)) => { eprintln!("clanky agent: {message}\n{USAGE}"); 2 }
        Err(PostAi) => { eprintln!("{}", clanky_core::POST_AI_REASON); 2 }
    }
}

fn run(args: &[String]) -> Result<(), Failure> {
    let (mut socket, mut config_path, mut tab, mut list) = (None, None, false, false);
    let mut words = Vec::new();
    let mut items = args.iter();
    while let Some(arg) = items.next() {
        match arg.as_str() {
            "--socket" => socket = Some(PathBuf::from(items.next().ok_or_else(|| Usage("--socket needs a path".into()))?)),
            "--config" => config_path = Some(PathBuf::from(items.next().ok_or_else(|| Usage("--config needs a path".into()))?)),
            "--tab" => tab = true,
            "--list" => list = true,
            "--help" | "-h" => { println!("{USAGE}"); return Ok(()); }
            _ if arg.starts_with("--") => return Err(Usage(format!("unknown option {arg}"))),
            _ => words.push(arg.clone()),
        }
    }
    let config = Config::load(config_path.as_deref()).map_err(Error)?;
    let path = std::env::var_os("PATH");
    if list {
        if !words.is_empty() || tab { return Err(Usage("--list takes no other arguments".into())); }
        print!("{}", agent_launch::list(&config.agents, path.as_deref()));
        return Ok(());
    }
    if config.post_ai { return Err(PostAi); }
    let (name, dir) = match words.as_slice() {
        [name] => (name, None),
        [name, dir] => (name, Some(dir)),
        _ => return Err(Usage("name one agent, and optionally a folder".into())),
    };
    let tool = agent_launch::find(&config.agents, name)
        .ok_or_else(|| Usage(format!("no agent named {name:?} in the config (clanky agent --list shows them)")))?;
    let dir = match dir { Some(dir) => PathBuf::from(dir), None => std::env::current_dir().map_err(|e| Error(e.to_string()))? };
    let dir = std::fs::canonicalize(&dir).ok().filter(|d| d.is_dir())
        .ok_or_else(|| Usage(format!("{} is not a folder", dir.display())))?;
    let dir = dir.to_str().ok_or_else(|| Usage("the folder name is not UTF-8".into()))?.to_owned();
    if !agent_launch::installed(&tool.command, path.as_deref()) {
        return Err(Error(format!("{} is not installed: {} is not on PATH", tool.name, agent_launch::program(&tool.command))));
    }
    let explicit = socket.is_some();
    let socket = match socket { Some(path) => path, None => clanky_core::platform::default_socket().map_err(|e| Error(e.to_string()))? };
    let client = connect(&socket, explicit, config_path.as_deref())?;
    let allow = config.agent_pane_access;
    let id = agent_launch::start(&client, tool, None, Some(&dir), allow, config.rows, config.columns).map_err(Error)?;
    if !tab { agent_launch::place_right(&client, &id).map_err(Error)?; }
    else {
        let mut workspace = client.workspace_get().map_err(|e| Error(e.message))?;
        if let Some(index) = workspace.groups.iter().position(|g| g.layout.contains(&id)) {
            workspace.active = index;
            let _ = client.workspace_set(&workspace);
        }
    }
    Ok(())
}

/// Connect as the owner. Post-AI refuses an unauthenticated hello, which is how a
/// Post-AI server is told apart; with no server on the default socket, start a window.
fn connect(socket: &Path, explicit: bool, config: Option<&Path>) -> Result<Client, Failure> {
    let probe = |socket: &Path| -> Option<Result<(), Failure>> {
        let client = Client::connect_agent(socket).ok()?;
        Some(match client.exchange(3, "hello", &serde_json::json!({})) {
            Err(error) if error.message == clanky_core::POST_AI_REASON => Err(PostAi),
            _ => Ok(()),
        })
    };
    match probe(socket) {
        Some(result) => result?,
        None if explicit => return Err(Error(format!("no Clanky is running on {}", socket.display()))),
        None => {
            let mut args: Vec<OsString> = Vec::new();
            if let Some(config) = config { args.extend(["--config".into(), config.as_os_str().to_owned()]); }
            crate::platform::spawn_detached(&args).map_err(|e| Error(format!("cannot start Clanky: {e}")))?;
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                if let Some(result) = probe(socket) { result?; break; }
                if Instant::now() > deadline { return Err(Error("Clanky did not start within 15 s".into())); }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    Client::connect_owner(socket).map_err(|e| Error(format!(
        "{e} (run clanky agent from outside Clanky's panes, or press Ctrl+Shift+G in the window)")))
}
