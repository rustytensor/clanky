//! Start an agent's own command-line tool in a new pane. Clanky makes no AI calls and
//! stores no keys: it opens a terminal running the user's shell and types the
//! configured command into it, so the tool's exit returns to a normal prompt.
use crate::config::AgentTool;
use clanky_api::Client;
use clanky_core::layout::{Axis, Rect};
use serde_json::json;
use std::{ffi::OsStr, path::Path};

/// The program a configured command starts: its first word.
pub fn program(command: &str) -> &str {
    command.split_whitespace().next().unwrap_or("")
}

/// Whether the command's program exists: an executable file at a path, or one on `PATH`.
pub fn installed(command: &str, path: Option<&OsStr>) -> bool {
    let program = program(command);
    let executable = crate::platform::is_executable;
    if program.is_empty() {
        return false;
    }
    if program.contains('/') {
        return executable(Path::new(program));
    }
    path.is_some_and(|path| std::env::split_paths(path).any(|dir| dir.is_absolute() && executable(&dir.join(program))))
}

/// The tool a person named: by name, ignoring case, or by its command.
pub fn find<'a>(agents: &'a [AgentTool], name: &str) -> Option<&'a AgentTool> {
    agents.iter().find(|tool| tool.name.eq_ignore_ascii_case(name))
        .or_else(|| agents.iter().find(|tool| tool.command == name || program(&tool.command) == name))
}

/// The configured tool a command's program belongs to (matched by its file name).
pub fn tool_for_program<'a>(agents: &'a [AgentTool], running: &str) -> Option<&'a AgentTool> {
    let base = |text: &str| Path::new(text).file_name().and_then(OsStr::to_str).map(str::to_owned).unwrap_or_default();
    let running = base(running);
    if running.is_empty() {
        return None;
    }
    agents.iter().find(|tool| base(program(&tool.command)) == running)
}

/// `clanky agent --list`: one line per tool, in config order.
pub fn list(agents: &[AgentTool], path: Option<&OsStr>) -> String {
    agents.iter().map(|tool| format!("{}\t{}\t{}\n", tool.name, tool.command,
        if installed(&tool.command, path) { "found" } else { "not installed" })).collect()
}

/// Open a terminal in `cwd` (or the focused pane's folder), with agent access as
/// configured, and type the tool's command into its shell. Returns the terminal ID.
pub fn start(client: &Client, tool: &AgentTool, focused: Option<&str>, cwd: Option<&str>, allow_agent: bool,
    rows: usize, cols: usize) -> Result<String, String> {
    let id = crate::window_tools::new_terminal_with(client, focused, cwd, allow_agent, rows, cols)?;
    // The window names the pane for the tool (title, amber edge) from this.
    let _ = client.exchange(3, "agent_tool_set", &json!({"tab_id":id,"name":tool.name}));
    // Typed as a person would, once the shell has drawn its prompt (up to 1.5 s), so the
    // terminal does not echo it twice. A slow shell still reads it when it is ready.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1500);
    while std::time::Instant::now() < deadline {
        let drawn = client.exchange(3, "history", &json!({"tab_id":id,"offset":0,"limit":64})).ok()
            .is_some_and(|reply| reply["lines"].as_array().is_some_and(|lines| lines.iter().any(|l| l.as_str().is_some_and(|t| !t.trim().is_empty()))));
        if drawn { break; }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if let Err(error) = client.input(&id, format!("{}\r", tool.command).as_bytes()) {
        let _ = client.request("close_tab", &json!({"tab_id":id}), true);
        return Err(error.message);
    }
    Ok(id)
}

/// Move a terminal the server opened as its own tab into a split to the right of the
/// focused pane, when that pane is wide enough. Otherwise it stays a new tab, which is
/// made the active one. Returns true when it was split.
pub fn place_right(client: &Client, id: &str) -> Result<bool, String> {
    for _ in 0..3 {
        let mut workspace = client.workspace_get().map_err(|e| e.message)?;
        let Some(target) = workspace.groups.get(workspace.active).map(|g| g.focus.clone()).filter(|t| t != id) else {
            return Ok(false);
        };
        let size = client.exchange(3, "terminal_info", &json!({"tab_id":target})).ok()
            .and_then(|info| Some((info["rows"].as_u64()? as usize, info["cols"].as_u64()? as usize)));
        let index = workspace.groups.iter().position(|g| g.layout.contains(&target));
        let fits = size.is_some_and(|(rows, cols)| cols >= 2 * clanky_core::layout::MIN_COLUMNS + 1 && rows >= clanky_core::layout::MIN_ROWS);
        let mut layout = match index { Some(index) if fits => workspace.groups[index].layout.clone(), _ => {
            if let Some(own) = workspace.groups.iter().position(|g| g.layout.contains(id)) { workspace.active = own; }
            return client.workspace_set(&workspace).map(|_| false).map_err(|e| e.message);
        } };
        // The window sizes panes itself; a large tree rectangle only checks the target is a leaf.
        let root = Rect { col: 0, row: 0, cols: 4096, rows: 4096 };
        if layout.split(&target, id.to_owned(), Axis::Columns, root).is_err() { return Ok(false); }
        workspace.groups.retain(|g| !g.layout.contains(id));
        let Some(index) = workspace.groups.iter().position(|g| g.layout.contains(&target)) else { return Ok(false); };
        workspace.groups[index].layout = layout;
        workspace.groups[index].focus = id.to_owned();
        workspace.active = index;
        match client.workspace_set(&workspace) {
            Ok(_) => return Ok(true),
            Err(error) if error.code == "conflict" => continue,
            Err(error) => return Err(error.message),
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tools() -> Vec<AgentTool> {
        vec![AgentTool { name: "Claude Code".into(), command: "claude".into() },
             AgentTool { name: "Aider".into(), command: "aider --no-auto-commits".into() }]
    }
    #[test]
    fn names_commands_and_programs() {
        let tools = tools();
        assert_eq!(find(&tools, "claude code").map(|t| t.command.as_str()), Some("claude"));
        assert_eq!(find(&tools, "aider").map(|t| t.name.as_str()), Some("Aider"));
        assert_eq!(find(&tools, "codex"), None);
        assert_eq!(tool_for_program(&tools, "/usr/local/bin/aider").map(|t| t.name.as_str()), Some("Aider"));
        assert_eq!(tool_for_program(&tools, "bash"), None);
        assert!(installed("sh -c true", Some(OsStr::new("/usr/bin:/bin"))));
        assert!(!installed("no-such-program-t9", Some(OsStr::new("/usr/bin:/bin"))));
    }
}
