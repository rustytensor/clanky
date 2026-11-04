//! Shared, bounded command-block actions for the window and v2 API.
use super::*;
use crate::terminal::Block;
use serde::Serialize;
use std::collections::BTreeSet;

const PAYLOAD_LIMIT: usize = 64 * 1024;
const ACTIONS: &[&str] = &[
    "select",
    "copy",
    "collapse",
    "rerun",
    "send",
    "task_update",
    "view",
    "tasks",
    "task_get",
];

#[derive(Clone, Serialize)]
struct Selection {
    block_id: String,
    start: usize,
    end: usize,
}
struct Task {
    task_id: String,
    label: String,
    state: String,
    tab_id: String,
    block_id: String,
    command_id: String,
    payload: Vec<u8>,
}
#[derive(Default)]
pub(super) struct ActionState {
    selection: Option<Selection>,
    collapsed: BTreeSet<String>,
    tasks: Vec<Task>,
    next_task: u64,
}
impl ActionState {
    fn prune(&mut self, blocks: &[Block]) {
        self.collapsed
            .retain(|id| blocks.iter().any(|b| &b.block_id == id));
        if self
            .selection
            .as_ref()
            .is_some_and(|s| !blocks.iter().any(|b| b.block_id == s.block_id))
        {
            self.selection = None;
        }
    }
    fn selected<'a>(&self, block: &'a Block) -> Result<&'a [u8], CoreError> {
        let s = self
            .selection
            .as_ref()
            .filter(|s| s.block_id == block.block_id)
            .ok_or_else(|| invalid("select a range in this block first"))?;
        let text = block
            .output
            .get(s.start..s.end)
            .ok_or_else(|| invalid("selection is no longer within retained output"))?;
        Ok(text.as_bytes())
    }
}
fn payload(bytes: &[u8]) -> Value {
    json!({"encoding":"base64", "data":base64::engine::general_purpose::STANDARD.encode(bytes), "bytes":bytes.len()})
}
fn task_value(task: &Task, tab: &Tab, detailed: bool) -> Value {
    let process = tab
        .command(&task.command_id)
        .ok()
        .map(|c| c.state.lock().unwrap().status());
    let mut result = json!({"task_id":task.task_id,"label":task.label,"state":task.state,
        "tab_id":task.tab_id,"block_id":task.block_id,"command_id":task.command_id,"process":process});
    if detailed {
        result
            .as_object_mut()
            .unwrap()
            .extend(payload(&task.payload).as_object().unwrap().clone());
    }
    result
}
fn validate_run(args: &Value) -> Result<(), CoreError> {
    let argv = args
        .get("argv")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or_else(|| invalid("argv must be a nonempty string array"))?;
    let mut size = 0usize;
    for (i, value) in argv.iter().enumerate() {
        let s = value
            .as_str()
            .filter(|s| !s.contains('\0'))
            .ok_or_else(|| invalid("argv must contain NUL-free strings"))?;
        if i == 0 && s.is_empty() {
            return Err(invalid("invalid argv size"));
        }
        size = size.saturating_add(s.len()).saturating_add(1);
    }
    if size > 1024 * 1024 {
        return Err(invalid("invalid argv size"));
    }
    let cwd = string(args, "cwd")?;
    if cwd.contains('\0') || !Path::new(cwd).is_absolute() || !Path::new(cwd).is_dir() {
        return Err(invalid("cwd must be an existing absolute directory"));
    }
    Ok(())
}
impl Engine {
    /// Dispatch the same authorized block action used by the GUI and v2 transport.
    pub fn palette_action(
        &self,
        action: &str,
        args: &Value,
        trusted: bool,
    ) -> Result<Value, CoreError> {
        let allowed: &[&str] = match action {
            "hello" | "actions" => &[],
            "view" => &["tab_id", "after", "limit"],
            "select" => &["tab_id", "block_id", "start", "end"],
            "copy" => &["tab_id", "block_id"],
            "collapse" => &["tab_id", "block_id", "collapsed"],
            "rerun" => &["tab_id", "block_id", "argv", "cwd"],
            "send" => &["tab_id", "block_id", "label"],
            "task_update" => &["tab_id", "task_id", "state"],
            "tasks" => &["tab_id"],
            "task_get" => &["tab_id", "task_id"],
            _ => return Err(error("unknown_operation", "unknown action")),
        };
        fields(args, allowed)?;
        if action == "hello" {
            return Ok(json!({"version":2,"capabilities":["block_actions","tasks","palette"]}));
        }
        if action == "actions" {
            return Ok(json!({"actions":ACTIONS}));
        }
        let tab_id = string(args, "tab_id")?;
        let tab = self.tab(tab_id, trusted)?;
        // Snapshot and release the terminal before locking metadata or calling run.
        let (blocks, _) = tab.terminal.lock().unwrap().blocks();
        if action == "rerun" {
            validate_run(args)?;
            let block_id = string(args, "block_id")?;
            if !blocks.iter().any(|b| b.block_id == block_id) {
                return Err(error("not_found", "unknown retained block"));
            }
            let permission = tab.allowed.load(Ordering::Acquire);
            let opened = self.request("open_tab", &json!({"allow_agent":permission}), trusted)?;
            let new_id = opened["tab_id"].as_str().unwrap();
            let result = self.request(
                "run",
                &json!({"tab_id":new_id,"argv":args["argv"],"cwd":args["cwd"]}),
                trusted,
            );
            return match result {
                Ok(run) => Ok(
                    json!({"tab_id":new_id,"command_id":run["command_id"],"source_tab_id":tab_id,"block_id":block_id}),
                ),
                Err(e) => {
                    // Remove only the newly created terminal, even if permission changed.
                    let removed = {
                        let mut state = self.0.0.lock().unwrap();
                        let removed = state.tabs.remove(new_id);
                        if removed.is_some() {
                            state.workspace.remove(new_id);
                        }
                        removed
                    };
                    if let Some(t) = removed {
                        t.close();
                    }
                    Err(e)
                }
            };
        }
        let mut state = tab.actions.lock().unwrap();
        state.prune(&blocks);
        match action {
            "view" => {
                let limit = if args.get("limit").is_some() {
                    number(args, "limit", 1, 64)? as usize
                } else {
                    64
                };
                let after = match args.get("after") {
                    None | Some(Value::Null) => None,
                    Some(Value::String(id)) => Some(id.as_str()),
                    _ => return Err(invalid("after must be string or null")),
                };
                let start = match after {
                    Some(id) => {
                        blocks
                            .iter()
                            .position(|b| b.block_id == id)
                            .ok_or_else(|| error("not_found", "block cursor no longer retained"))?
                            + 1
                    }
                    None => 0,
                };
                let mut page = Vec::new();
                let mut budget = 256 * 1024;
                for mut block in blocks.iter().skip(start).take(limit).cloned() {
                    if budget == 0 {
                        break;
                    }
                    if block.output.len() > budget {
                        let mut end = budget;
                        while !block.output.is_char_boundary(end) {
                            end -= 1;
                        }
                        block.output.truncate(end);
                        block.truncated = true;
                    }
                    budget -= block.output.len();
                    let collapsed = state.collapsed.contains(&block.block_id);
                    let mut value = serde_json::to_value(block).unwrap();
                    value["collapsed"] = json!(collapsed);
                    page.push(value);
                }
                let next = page
                    .last()
                    .map(|b| b["block_id"].clone())
                    .unwrap_or_else(|| json!(after));
                let tasks: Vec<_> = state
                    .tasks
                    .iter()
                    .map(|t| task_value(t, &tab, false))
                    .collect();
                Ok(
                    json!({"has_more":blocks.len().saturating_sub(start)>page.len(),"next_after":next,
                    "blocks":page,"selection":state.selection,"tasks":tasks}),
                )
            }
            "tasks" => Ok(
                json!({"tasks":state.tasks.iter().map(|t| task_value(t,&tab,false)).collect::<Vec<_>>()}),
            ),
            "task_get" | "task_update" => {
                let id = string(args, "task_id")?;
                let next = if action == "task_update" {
                    let s = string(args, "state")?;
                    if !["queued", "running", "done", "failed", "cancelled"].contains(&s) {
                        return Err(invalid("invalid task state"));
                    }
                    Some(s)
                } else {
                    None
                };
                let task = state
                    .tasks
                    .iter_mut()
                    .find(|t| t.task_id == id)
                    .ok_or_else(|| error("not_found", "unknown task"))?;
                if let Some(next) = next {
                    task.state = next.into();
                }
                Ok(task_value(task, &tab, action == "task_get"))
            }
            _ => {
                let block_id = string(args, "block_id")?;
                let block = blocks
                    .iter()
                    .find(|b| b.block_id == block_id)
                    .ok_or_else(|| error("not_found", "unknown retained block"))?;
                match action {
                    "select" => {
                        let start = number(args, "start", 0, block.output.len() as u64)? as usize;
                        let end = number(args, "end", 0, block.output.len() as u64)? as usize;
                        if start > end
                            || end - start > PAYLOAD_LIMIT
                            || !block.output.is_char_boundary(start)
                            || !block.output.is_char_boundary(end)
                        {
                            return Err(invalid(
                                "selection must be ordered UTF-8 boundaries of at most 64 KiB",
                            ));
                        }
                        state.selection = Some(Selection {
                            block_id: block_id.into(),
                            start,
                            end,
                        });
                        Ok(json!({"selection":state.selection}))
                    }
                    "copy" => Ok(payload(state.selected(block)?)),
                    "collapse" => {
                        let collapsed = args
                            .get("collapsed")
                            .and_then(Value::as_bool)
                            .ok_or_else(|| invalid("collapsed must be boolean"))?;
                        if collapsed {
                            state.collapsed.insert(block_id.into());
                        } else {
                            state.collapsed.remove(block_id);
                        }
                        Ok(json!({"block_id":block_id,"collapsed":collapsed}))
                    }
                    "send" => {
                        let label = string(args, "label")?;
                        if label.is_empty() || label.len() > 256 {
                            return Err(invalid("label must be 1 to 256 UTF-8 bytes"));
                        }
                        if state.tasks.len() >= 128 {
                            return Err(error("resource_limit", "task limit reached"));
                        }
                        let bytes = state.selected(block)?.to_vec();
                        state.next_task += 1;
                        let task = Task {
                            task_id: format!("{}-task{}", tab_id, state.next_task),
                            label: label.into(),
                            state: "queued".into(),
                            tab_id: tab_id.into(),
                            block_id: block_id.into(),
                            command_id: block.command_id.clone(),
                            payload: bytes,
                        };
                        let value = task_value(&task, &tab, true);
                        state.tasks.push(task);
                        Ok(value)
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}
