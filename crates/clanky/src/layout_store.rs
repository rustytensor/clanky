//! Private, command-free layout snapshots and fresh-shell restoration.
use clanky_api::Client;
use clanky_core::{layout::Layout, platform, Workspace, WorkspaceGroup};
use serde_json::{json, Value};
use std::{collections::{BTreeMap, BTreeSet}, fs, io::{Read, Write}, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

const LIMIT: u64 = 1024 * 1024;

pub fn default_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").filter(|v| !v.is_empty()).map(|v| PathBuf::from(v).join(".config")))?;
    Some(base.join("clanky/layout.json"))
}

fn call(client: &Client, op: &str, args: Value) -> Result<Value, String> {
    client.request(op, &args, true).map_err(|e| e.message)
}

fn inspect_layout(layout: &Layout, ids: &mut BTreeSet<String>, depth: usize) -> Result<(), String> {
    if depth > 16 { return Err("saved layout too deep".into()); }
    match layout {
        Layout::Leaf(id) => {
            if id.is_empty() || !ids.insert(id.clone()) || ids.len() > 64 { return Err("invalid saved terminal IDs".into()); }
        }
        Layout::Split { first, second, .. } => {
            inspect_layout(first, ids, depth + 1)?;
            inspect_layout(second, ids, depth + 1)?;
        }
    }
    Ok(())
}

fn validate(value: &Value) -> Result<(Vec<WorkspaceGroup>, usize, BTreeMap<String, (String, usize, usize)>), String> {
    let object = value.as_object().ok_or("saved layout must be an object")?;
    if object.len() != 4 || value["version"] != 1 { return Err("invalid saved layout version or fields".into()); }
    let groups: Vec<WorkspaceGroup> = serde_json::from_value(value["groups"].clone()).map_err(|_| "invalid saved groups")?;
    let active = value["active"].as_u64().and_then(|n| usize::try_from(n).ok()).ok_or("invalid saved active group")?;
    if groups.len() > 64 || (groups.is_empty() && active != 0) || (!groups.is_empty() && active >= groups.len()) { return Err("invalid saved active group".into()); }
    let mut ids = BTreeSet::new();
    for group in &groups {
        inspect_layout(&group.layout, &mut ids, 1)?;
        if !group.layout.contains(&group.focus) { return Err("invalid saved focus".into()); }
    }
    let values = value["terminals"].as_object().ok_or("invalid saved terminals")?;
    if values.keys().cloned().collect::<BTreeSet<_>>() != ids { return Err("saved terminal IDs differ from layout".into()); }
    let mut terminals = BTreeMap::new();
    for (id, v) in values {
        let fields = v.as_object().ok_or("invalid saved terminal")?;
        if fields.len() != 3 { return Err("invalid saved terminal fields".into()); }
        let cwd = v["cwd"].as_str().ok_or("invalid saved directory")?;
        if !Path::new(cwd).is_absolute() || !Path::new(cwd).is_dir() || cwd.contains('\0') { return Err("saved directory is unavailable".into()); }
        let rows = v["rows"].as_u64().and_then(|n| usize::try_from(n).ok()).ok_or("invalid saved rows")?;
        let cols = v["cols"].as_u64().and_then(|n| usize::try_from(n).ok()).ok_or("invalid saved columns")?;
        if rows == 0 || cols == 0 || rows > 1000 || cols > 1000 || rows * cols > 250_000 { return Err("invalid saved dimensions".into()); }
        terminals.insert(id.clone(), (cwd.to_owned(), rows, cols));
    }
    Ok((groups, active, terminals))
}

pub(crate) fn remap(layout: &Layout, ids: &BTreeMap<String, String>) -> Result<Layout, String> {
    Ok(match layout {
        Layout::Leaf(id) => Layout::Leaf(ids.get(id).ok_or("missing restored ID")?.clone()),
        Layout::Split { axis, ratio, first, second } => Layout::Split { axis: *axis, ratio: *ratio, first: Box::new(remap(first, ids)?), second: Box::new(remap(second, ids)?) },
    })
}

/// Restore only after a caller has proved this is a freshly started local server.
pub fn restore(client: &Client, path: &Path, fresh: bool) -> Result<bool, String> {
    if !fresh { return Ok(false); }
    let mut file = match platform::read_private_file(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(format!("cannot read saved layout: {e}")),
    };
    if file.metadata().map_err(|e| e.to_string())?.len() > LIMIT { return Err("saved layout too large".into()); }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file).take(LIMIT + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > LIMIT { return Err("saved layout too large".into()); }
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| format!("invalid saved layout: {e}"))?;
    let (groups, active, terminals) = validate(&value)?;
    if groups.is_empty() { return Ok(false); }
    if !client.workspace_get().map_err(|e| e.message)?.groups.is_empty() { return Ok(false); }
    let defaults = call(client, "v3.shell_defaults", json!({}))?;
    let mut created = Vec::new();
    let result = (|| {
        let mut ids = BTreeMap::new();
        for group in &groups {
            for old in group.layout.leaves() {
                let (cwd, rows, cols) = terminals.get(&old).ok_or("missing saved terminal")?;
                let tab = call(client, "open_tab", json!({"rows":rows,"cols":cols,"allow_agent":false}))?;
                let id = tab["tab_id"].as_str().ok_or("missing new tab ID")?.to_owned();
                created.push(id.clone());
                call(client, "run", json!({"tab_id":id,"argv":defaults["argv"],"cwd":cwd}))?;
                ids.insert(old, id);
            }
        }
        let workspace = client.workspace_get().map_err(|e| e.message)?;
        let groups = groups.iter().map(|g| Ok(WorkspaceGroup { layout: remap(&g.layout, &ids)?, focus: ids.get(&g.focus).ok_or("missing focus")?.clone() })).collect::<Result<Vec<_>, String>>()?;
        client.workspace_set(&Workspace { revision: workspace.revision, groups, active }).map_err(|e| e.message)?;
        Ok(true)
    })();
    if result.is_err() {
        for id in created { let _ = call(client, "close_tab", json!({"tab_id":id})); }
    }
    result
}

fn shell_finished(client: &Client, id: &str) -> bool {
    call(client, "v3.finished_shells", json!({})).ok()
        .is_some_and(|v| v["tab_ids"].as_array().is_some_and(|ids| ids.iter().any(|t| t.as_str() == Some(id))))
}

/// Persist only a complete, current snapshot. Never include command or output.
pub fn save(client: &Client, path: &Path) -> Result<(), String> {
    let workspace = client.workspace_get().map_err(|e| e.message)?;
    let mut terminals = serde_json::Map::new();
    for group in &workspace.groups {
        for id in group.layout.leaves() {
            // A pane whose shell just exited has no folder left to read; the window closes it
            // and saves the layout again then.
            let info = match call(client, "v3.terminal_info", json!({"tab_id":id})) {
                Ok(info) => info,
                Err(_) if shell_finished(client, &id) => return Ok(()),
                Err(e) => return Err(e),
            };
            terminals.insert(id, json!({"cwd":info["cwd"],"rows":info["rows"],"cols":info["cols"]}));
        }
    }
    if client.workspace_get().map_err(|e| e.message)?.revision != workspace.revision { return Err("workspace changed during save".into()); }
    let value = json!({"version":1,"active":workspace.active,"groups":workspace.groups,"terminals":terminals});
    validate(&value)?;
    let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > LIMIT { return Err("saved layout too large".into()); }
    let dir = path.parent().ok_or("invalid layout path")?;
    let base = dir.parent().ok_or("invalid layout directory")?;
    fs::create_dir_all(base).map_err(|e| e.to_string())?;
    platform::create_private_directory(dir).map_err(|e| e.to_string())?;
    match fs::symlink_metadata(path) {
        Ok(m) if !m.file_type().is_file() => return Err("unsafe saved layout path".into()),
        Ok(_) => {
            let file = platform::read_private_file(path).map_err(|e| e.to_string())?;
            let mut old = Vec::new();
            file.take(LIMIT + 1).read_to_end(&mut old).map_err(|e| e.to_string())?;
            if old == bytes { return Ok(()); }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.to_string()),
    }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let temp = dir.join(format!(".layout-{}-{stamp}.tmp", std::process::id()));
    let result = (|| {
        let mut file = platform::create_private_file(&temp).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        if client.workspace_get().map_err(|e| e.message)?.revision != workspace.revision { return Err("workspace changed during save".into()); }
        fs::rename(&temp, path).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&temp); }
    result
}
