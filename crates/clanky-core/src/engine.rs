#[path = "actions.rs"]
mod actions;
#[path = "agent.rs"]
mod agent;
#[path = "events.rs"]
mod events;
#[path = "masking.rs"]
mod masking;
use agent::optional_number;

use crate::{
    TerminalSnapshot, Workspace,
    platform::{self, Control, Process},
    terminal::Terminal,
};
use base64::Engine as _;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    path::Path,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct CoreError {
    pub code: String,
    pub message: String,
}
impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for CoreError {}
fn error(code: &str, message: impl Into<String>) -> CoreError {
    CoreError {
        code: code.into(),
        message: message.into(),
    }
}
fn invalid(message: impl Into<String>) -> CoreError {
    error("invalid_request", message)
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, CoreError> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{key} must be a string")))
}
fn number(v: &Value, key: &str, min: u64, max: u64) -> Result<u64, CoreError> {
    let n = v
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("{key} must be an integer")))?;
    if n < min || n > max {
        return Err(invalid(format!("{key} is out of range")));
    }
    Ok(n)
}
fn dimensions(rows: usize, cols: usize) -> Result<(), CoreError> {
    if rows == 0 || cols == 0 || rows > 1000 || cols > 1000 || rows * cols > 250000 {
        Err(invalid("invalid terminal dimensions"))
    } else {
        Ok(())
    }
}
fn fields(v: &Value, allowed: &[&str]) -> Result<(), CoreError> {
    let obj = v
        .as_object()
        .ok_or_else(|| invalid("request must be an object"))?;
    for k in obj.keys() {
        if !["v", "id", "op"].contains(&k.as_str()) && !allowed.contains(&k.as_str()) {
            return Err(invalid(format!("unknown field {k}")));
        }
    }
    Ok(())
}

struct ResultState {
    safe: VecDeque<u8>,
    safe_total: u64,
    masker: Option<clanky_mask::Masker>,
    masking_failed: bool,
    finished_event: bool,
    output: VecDeque<u8>,
    total: u64,
    exited: bool,
    drained: bool,
    code: Option<i32>,
    signal: Option<i32>,
    failure: Option<String>,
}
impl ResultState {
    fn done(&self) -> bool {
        self.exited && self.drained
    }
    fn status(&self) -> Value {
        json!({"done": self.done(), "state": if self.done() {"exited"} else if self.exited {"draining"} else {"running"}, "exit_code": if self.done() {self.code} else {None}, "signal": if self.done() {self.signal} else {None}})
    }
}
pub(crate) struct Command {
    pub id: String,
    state: Mutex<ResultState>,
    changed: Condvar,
    safety: Arc<events::Safety>,
    tab_id: String,
    allowed: Arc<AtomicBool>,
    safe_terminal: Arc<Mutex<Terminal>>,
}
impl Command {
    fn new(id: String, tab_id: String, tab: &Tab) -> Result<Self, CoreError> {
        Ok(Self {
            id,
            state: Mutex::new(ResultState {
                safe: VecDeque::new(),
                safe_total: 0,
                masker: tab.safety.policy.stream().ok(),
                masking_failed: false,
                finished_event: false,
                output: VecDeque::new(),
                total: 0,
                exited: false,
                drained: false,
                code: None,
                signal: None,
                failure: None,
            }),
            changed: Condvar::new(),
            safety: tab.safety.clone(),
            tab_id,
            allowed: tab.allowed.clone(),
            safe_terminal: tab.safe_terminal.clone(),
        })
    }
    fn append_safe(&self, s: &mut ResultState, bytes: &[u8]) {
        let start = s.safe_total;
        s.safe_total += bytes.len() as u64;
        s.safe.extend(bytes);
        let excess = s.safe.len().saturating_sub(8 * 1024 * 1024);
        s.safe.drain(..excess);
        self.safe_terminal.lock().unwrap().feed(&self.id, bytes);
        self.safety
            .output(&self.tab_id, &self.allowed, &self.id, bytes, start);
    }
    fn completion(&self, s: &mut ResultState) {
        if s.done() && !s.finished_event {
            s.finished_event = true;
            self.safety.publish(&self.tab_id, &self.allowed, json!({"kind":"command_finished", "command_id":self.id,"exit_code":s.code,"signal":s.signal}));
        }
    }
    pub fn append(&self, bytes: &[u8]) {
        let mut s = self.state.lock().unwrap();
        match s.masker.as_mut().map(|m| m.push(bytes)) {
            Some(Ok(safe)) => self.append_safe(&mut s, &safe),
            _ => s.masking_failed = true,
        }
        s.total += bytes.len() as u64;
        s.output.extend(bytes);
        let excess = s.output.len().saturating_sub(8 * 1024 * 1024);
        s.output.drain(..excess);
    }
    pub fn exited(&self, code: Option<i32>, signal: Option<i32>) {
        let mut s = self.state.lock().unwrap();
        s.exited = true;
        s.code = code;
        s.signal = signal;
        self.completion(&mut s);
        self.changed.notify_all();
    }
    pub fn failed(&self, message: String) {
        self.state.lock().unwrap().failure = Some(message);
    }
    pub fn drained(&self) {
        let mut s = self.state.lock().unwrap();
        if !s.drained {
            match s.masker.as_mut().map(clanky_mask::Masker::finish) {
                Some(Ok(safe)) => self.append_safe(&mut s, &safe),
                _ => s.masking_failed = true,
            }
            self.safe_terminal.lock().unwrap().eof();
            s.drained = true;
            self.completion(&mut s);
        }
        self.changed.notify_all();
    }
}
struct Commands {
    current: Option<String>,
    all: BTreeMap<String, Arc<Command>>,
}
pub(crate) struct Tab {
    allowed: Arc<AtomicBool>,
    permission: AtomicU64,
    safety: Arc<events::Safety>,
    id: String,
    safe_terminal: Arc<Mutex<Terminal>>,
    last_output: Mutex<Instant>,
    last_input: Mutex<Option<Instant>>,
    was_waiting: AtomicBool,
    closed: AtomicBool,
    lifecycle: Mutex<()>,
    terminal: Mutex<Terminal>,
    commands: Mutex<Commands>,
    process: Mutex<Option<Process>>,
    actions: Mutex<actions::ActionState>,
}
impl Tab {
    fn new(
        rows: usize,
        cols: usize,
        allowed: bool,
        id: String,
        safety: Arc<events::Safety>,
    ) -> Self {
        Self {
            allowed: Arc::new(AtomicBool::new(allowed)),
            permission: AtomicU64::new(0),
            safety,
            id,
            safe_terminal: Arc::new(Mutex::new(Terminal::new(rows, cols))),
            last_output: Mutex::new(Instant::now()),
            last_input: Mutex::new(None),
            was_waiting: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            lifecycle: Mutex::new(()),
            terminal: Mutex::new(Terminal::new(rows, cols)),
            commands: Mutex::new(Commands {
                current: None,
                all: BTreeMap::new(),
            }),
            process: Mutex::new(None),
            actions: Mutex::new(actions::ActionState::default()),
        }
    }
    pub fn feed(&self, command: &str, bytes: &[u8]) {
        *self.last_output.lock().unwrap() = Instant::now();
        let responses = self.terminal.lock().unwrap().feed(command, bytes);
        if !responses.is_empty() {
            if let Some(p) = &*self.process.lock().unwrap() {
                for r in responses {
                    let _ = p.input(r);
                }
            }
        }
    }
    pub fn eof(&self, _: &str) {
        self.terminal.lock().unwrap().eof();
    }
    fn command(&self, id: &str) -> Result<Arc<Command>, CoreError> {
        self.commands
            .lock()
            .unwrap()
            .all
            .get(id)
            .cloned()
            .ok_or_else(|| error("not_found", "unknown command"))
    }
    fn close(&self) {
        let _guard = self.lifecycle.lock().unwrap();
        if !self.closed.swap(true, Ordering::AcqRel) {
            self.safety
                .publish(&self.id, &self.allowed, json!({"kind":"terminal_closed"}));
        }
        let process = self.process.lock().unwrap().take();
        if let Some(p) = process {
            p.stop();
        }
    }
}
struct State {
    tabs: BTreeMap<String, Arc<Tab>>,
    next_tab: u64,
    next_command: u64,
    shutdown: bool,
    workspace: Workspace,
}
struct Inner(
    Mutex<State>,
    Arc<events::Safety>,
    Mutex<Option<std::thread::JoinHandle<()>>>,
);
impl Drop for Inner {
    fn drop(&mut self) {
        self.1.stop.store(true, Ordering::Release);
        self.1.changed.notify_all();
        if let Some(handle) = self.2.get_mut().unwrap().take() {
            if handle.thread().id() != std::thread::current().id() {
                let _ = handle.join();
            }
        }
        let s = self.0.get_mut().unwrap();
        for t in s.tabs.values() {
            t.close();
        }
    }
}
#[derive(Clone)]
pub struct Engine(Arc<Inner>);
impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
impl Engine {
    pub fn new() -> Self {
        let engine = Self(Arc::new(Inner(
            Mutex::new(State {
                tabs: BTreeMap::new(),
                next_tab: 0,
                next_command: 0,
                shutdown: false,
                workspace: Workspace::empty(),
            }),
            Arc::new(events::Safety::new()),
            Mutex::new(None),
        )));
        engine.start_monitor();
        engine
    }
    fn tab(&self, id: &str, trusted: bool) -> Result<Arc<Tab>, CoreError> {
        // Do not admit a fresh lease halfway through an epoch-wide revoke.
        let _authority = (!trusted).then(|| self.0.1.authority.lock().unwrap());
        let tab = self
            .0
            .0
            .lock()
            .unwrap()
            .tabs
            .get(id)
            .cloned()
            .ok_or_else(|| error("not_found", "unknown tab"))?;
        if !trusted && !tab.allowed.load(Ordering::Acquire) {
            return Err(error(
                "permission_denied",
                "Agent access is disabled for this tab",
            ));
        }
        Ok(tab)
    }
    pub fn tab_ids(&self) -> Vec<String> {
        self.0.0.lock().unwrap().tabs.keys().cloned().collect()
    }
    pub fn agent_permission(&self, tab_id: &str) -> Result<bool, CoreError> {
        Ok(self.tab(tab_id, true)?.allowed.load(Ordering::Acquire))
    }
    pub fn set_agent_permission(&self, tab_id: &str, allowed: bool) -> Result<(), CoreError> {
        let tab = self.tab(tab_id, true)?;
        let _authority = self.0.1.authority.lock().unwrap();
        let _guard = tab.lifecycle.lock().unwrap();
        tab.allowed.store(allowed, Ordering::Release);
        tab.permission.fetch_add(1, Ordering::AcqRel);
        tab.safety.changed.notify_all();
        for command in tab.commands.lock().unwrap().all.values() {
            command.changed.notify_all();
        }
        Ok(())
    }
    pub fn snapshot(&self, tab_id: &str) -> Result<TerminalSnapshot, CoreError> {
        Ok(self.tab(tab_id, true)?.terminal.lock().unwrap().snapshot())
    }
    pub fn input(&self, tab_id: &str, bytes: &[u8]) -> Result<(), CoreError> {
        if bytes.len() > 1024 * 1024 {
            return Err(error("resource_limit", "input exceeds 1 MiB"));
        }
        let t = self.tab(tab_id, true)?;
        let p = t.process.lock().unwrap();
        let p = p
            .as_ref()
            .ok_or_else(|| error("not_found", "no active process"))?;
        *t.last_input.lock().unwrap() = Some(Instant::now());
        t.terminal.lock().unwrap().input_submitted();
        p.input(bytes.to_vec())
            .map_err(|e| error("resource_limit", e.to_string()))
    }
    pub fn resize(&self, tab_id: &str, rows: usize, cols: usize) -> Result<(), CoreError> {
        dimensions(rows, cols)?;
        let t = self.tab(tab_id, true)?;
        let _guard = t.lifecycle.lock().unwrap();
        if let Some(p) = &*t.process.lock().unwrap() {
            match p.tx.try_send(Control::Resize(rows, cols)) {
                // An exited PTY has no reader, but its retained screen must
                // still resize when a split collapses or the window changes.
                Ok(()) | Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {}
                Err(std::sync::mpsc::TrySendError::Full(_)) => {
                    return Err(error("resource_limit", "PTY control queue unavailable"));
                }
            }
        }
        t.terminal.lock().unwrap().resize(rows, cols);
        t.safe_terminal.lock().unwrap().resize(rows, cols);
        Ok(())
    }
    pub fn shutdown(&self) {
        self.0.1.stop.store(true, Ordering::Release);
        self.0.1.changed.notify_all();
        let tabs = {
            let mut s = self.0.0.lock().unwrap();
            s.shutdown = true;
            s.workspace = Workspace::empty();
            std::mem::take(&mut s.tabs)
        };
        for t in tabs.values() {
            t.close();
        }
        if let Some(handle) = self.0.2.lock().unwrap().take() {
            let _ = handle.join();
        }
    }
    /// True only after the final explicitly closed tab, not on client loss.
    pub fn idle_exit_ready(&self) -> bool {
        let s = self.0.0.lock().unwrap();
        s.next_tab > 0 && s.tabs.is_empty()
    }

    pub fn scroll(&self, tab_id: &str, delta: i32) -> Result<(), CoreError> {
        let tab = self.tab(tab_id, true)?;
        tab.terminal.lock().unwrap().scroll(delta);
        tab.safe_terminal.lock().unwrap().scroll(delta);
        Ok(())
    }

    /// Owner-only operations. The transport must authenticate before calling.
    /// Deliberately separate from the unprivileged v1/v2 dispatcher.
    pub fn owner_request(&self, op: &str, args: &Value) -> Result<Value, CoreError> {
        let allowed: &[&str] = match op {
            "workspace_get" | "shell_defaults" => &[],
            "workspace_set" => &["revision", "groups", "active"],
            "snapshot" => &["tab_id", "known_generation", "known_images"],
            "history" => &["tab_id", "offset", "limit"],
            "scroll" => &["tab_id", "delta"],
            "input" => &["tab_id", "data"],
            "resize" => &["tab_id", "rows", "cols"],
            "permission_get" | "terminal_cwd" => &["tab_id"],
            "permission_set" => &["tab_id", "allow_agent"],
            "window_search" => &["tab_id", "query", "after", "backwards"],
            "clipboard_events" => &["tab_id", "write", "read"],
            "clipboard_reply" => &["tab_id", "request_id", "data"],
            _ => return Err(error("unknown_operation", "unknown operation")),
        };
        fields(args, allowed)?;
        if op == "shell_defaults" {
            let shell = std::env::var("SHELL")
                .ok()
                .filter(|s| Path::new(s).is_absolute() && Path::new(s).is_file())
                .unwrap_or_else(|| "/bin/sh".into());
            let cwd = std::env::current_dir().map_err(|e| error("io_error", e.to_string()))?;
            let home = platform::home_dir().and_then(|path| path.to_str().map(str::to_owned));
            return Ok(json!({"argv":[shell],"cwd":cwd,"home":home}));
        }
        if op == "workspace_get" {
            return Ok(json!(self.0.0.lock().unwrap().workspace));
        }
        if op == "workspace_set" {
            let revision = number(args, "revision", 0, u64::MAX)?;
            let mut s = self.0.0.lock().unwrap();
            if revision != s.workspace.revision {
                return Err(error("conflict", "workspace revision changed"));
            }
            let mut workspace: Workspace = serde_json::from_value(json!({
                "revision": revision, "groups": args.get("groups"), "active": args.get("active")
            }))
            .map_err(|_| invalid("invalid workspace"))?;
            if !workspace.valid(s.tabs.keys()) {
                return Err(invalid("invalid workspace layout, focus or active group"));
            }
            workspace.revision = revision
                .checked_add(1)
                .ok_or_else(|| error("resource_limit", "workspace revision exhausted"))?;
            s.workspace = workspace;
            return Ok(json!(s.workspace));
        }
        let tab_id = string(args, "tab_id")?;
        let tab = self.tab(tab_id, true)?;
        match op {
            "window_search" => {
                let query = string(args, "query")?;
                let after = match args.get("after") {
                    None | Some(Value::Null) => None,
                    Some(value) => {
                        fields(value, &["row", "col"])?;
                        Some((number(value, "row", 0, usize::MAX as u64)? as usize,
                            number(value, "col", 0, usize::MAX as u64)? as usize))
                    }
                };
                let backwards = match args.get("backwards") {
                    None => false,
                    Some(value) => value.as_bool().ok_or_else(|| invalid("backwards must be boolean"))?,
                };
                tab.terminal.lock().unwrap().window_search(query, after, backwards)
                    .map_err(|message| error("invalid_request", message))
            }
            "clipboard_events" => {
                let write = args.get("write").and_then(Value::as_bool).ok_or_else(|| invalid("write must be boolean"))?;
                let read = args.get("read").and_then(Value::as_bool).ok_or_else(|| invalid("read must be boolean"))?;
                let _guard = tab.lifecycle.lock().unwrap();
                let commands = tab.commands.lock().unwrap();
                let current = commands.current.as_deref();
                Ok(tab.terminal.lock().unwrap().clipboard_events(current, write, read))
            }
            "clipboard_reply" => {
                // `id` belongs to the RPC envelope, not the clipboard request.
                let id = number(args, "request_id", 1, u64::MAX)?;
                let data = string(args, "data")?;
                // The lifecycle lock excludes run/close through enqueueing the response.
                let _guard = tab.lifecycle.lock().unwrap();
                let commands = tab.commands.lock().unwrap();
                let current = commands.current.as_deref().filter(|id| {
                    !tab.closed.load(Ordering::Acquire) && commands.all.get(*id)
                        .is_some_and(|c| !c.state.lock().unwrap().exited)
                });
                let response = tab.terminal.lock().unwrap().clipboard_reply(current, id, data)
                    .map_err(|_| invalid("Invalid, expired or oversized clipboard reply"))?;
                let process = tab.process.lock().unwrap();
                process.as_ref().ok_or_else(|| error("not_found", "no active process"))?
                    .input(response).map_err(|_| error("resource_limit", "Clipboard response queue unavailable"))?;
                Ok(json!({}))
            }
            "terminal_cwd" => {
                // Release the terminal guard before touching the PTY or /proc.
                let reported = { tab.terminal.lock().unwrap().cwd() };
                if let Some(cwd) = reported {
                    return Ok(json!({"cwd":cwd,"source":"osc7"}));
                }
                let cwd = tab.process.lock().unwrap().as_ref().and_then(Process::cwd);
                Ok(json!({"source":cwd.as_ref().map(|_| "proc"),"cwd":cwd}))
            }
            "snapshot" => {
                let known_generation = match args.get("known_generation") {
                    None => None,
                    Some(v) => Some(
                        v.as_u64()
                            .ok_or_else(|| invalid("invalid known_generation"))?,
                    ),
                };
                let mut known = std::collections::BTreeSet::new();
                if let Some(value) = args.get("known_images") {
                    let values = value
                        .as_array()
                        .filter(|a| a.len() <= 4096)
                        .ok_or_else(|| invalid("invalid known_images"))?;
                    for value in values {
                        known.insert(value.as_u64().ok_or_else(|| invalid("invalid image key"))?);
                    }
                }
                let terminal = tab.terminal.lock().unwrap();
                let generation = terminal.generation;
                if known_generation == Some(generation) {
                    return Ok(json!({"generation":generation,"unchanged":true}));
                }
                // Placements clone only Arc headers, never their RGBA buffers.
                let snapshot = terminal.snapshot();
                drop(terminal);
                let mut pixels = Vec::new();
                for image in &snapshot.images {
                    if known.insert(image.image_key) {
                        pixels.push(json!({"image_key":image.image_key,
                            "data":base64::engine::general_purpose::STANDARD.encode(image.rgba.as_slice())}));
                    }
                }
                Ok(
                    json!({"generation":generation,"unchanged":false,"snapshot":snapshot,"pixels":pixels}),
                )
            }
            "history" => {
                let offset = if args.get("offset").is_some() {
                    number(args, "offset", 0, usize::MAX as u64)? as usize
                } else {
                    0
                };
                let limit = if args.get("limit").is_some() {
                    number(args, "limit", 1, 64)? as usize
                } else {
                    64
                };
                let (total, lines) = tab.terminal.lock().unwrap().history(offset, limit);
                if offset > total {
                    return Err(invalid("offset exceeds history"));
                }
                Ok(
                    json!({"total":total,"offset":offset,"next_offset":offset+lines.len(),"lines":lines}),
                )
            }
            "scroll" => {
                let delta = args
                    .get("delta")
                    .and_then(Value::as_i64)
                    .and_then(|n| i32::try_from(n).ok())
                    .ok_or_else(|| invalid("invalid scroll delta"))?;
                let offset = tab.terminal.lock().unwrap().scroll(delta);
                tab.safe_terminal.lock().unwrap().scroll(delta);
                Ok(json!({"display_offset":offset}))
            }
            "input" => {
                let data = string(args, "data")?;
                if data.len() > 65536usize.div_ceil(3) * 4 {
                    return Err(error("resource_limit", "input exceeds 64 KiB"));
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .map_err(|_| invalid("invalid input base64"))?;
                if bytes.len() > 65536 {
                    return Err(error("resource_limit", "input exceeds 64 KiB"));
                }
                self.input(tab_id, &bytes)?;
                Ok(json!({}))
            }
            "resize" => {
                self.resize(
                    tab_id,
                    number(args, "rows", 1, 1000)? as usize,
                    number(args, "cols", 1, 1000)? as usize,
                )?;
                Ok(json!({}))
            }
            "permission_get" => Ok(json!({"allow_agent":tab.allowed.load(Ordering::Acquire)})),
            "permission_set" => {
                let allowed = args
                    .get("allow_agent")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| invalid("allow_agent must be boolean"))?;
                self.set_agent_permission(tab_id, allowed)?;
                Ok(json!({"allow_agent":allowed}))
            }
            _ => unreachable!(),
        }
    }

    pub fn request(&self, op: &str, args: &Value, trusted: bool) -> Result<Value, CoreError> {
        if let Some(action) = op.strip_prefix("v4.") {
            return self.agent_request(action, args, trusted);
        }
        if let Some(action) = op.strip_prefix("v2.") {
            return self.palette_action(action, args, trusted);
        }
        let allowed: &[&str] = match op {
            "open_tab" => &["rows", "cols", "allow_agent"],
            "list_tabs" => &[],
            "run" => &["tab_id", "argv", "cwd"],
            "wait" => &["tab_id", "command_id", "timeout_ms"],
            "get_exit_code" => &["tab_id", "command_id"],
            "get_output" => &["tab_id", "command_id", "offset", "limit", "encoding"],
            "list_blocks" => &["tab_id", "command_id", "after", "limit"],
            "close_tab" => &["tab_id"],
            _ => return Err(error("unknown_operation", "unknown operation")),
        };
        fields(args, allowed)?;
        if op == "open_tab" {
            let rows = if args.get("rows").is_some() {
                number(args, "rows", 1, 1000)? as usize
            } else {
                24
            };
            let cols = if args.get("cols").is_some() {
                number(args, "cols", 1, 1000)? as usize
            } else {
                80
            };
            dimensions(rows, cols)?;
            let allow = match args.get("allow_agent") {
                None => false,
                Some(Value::Bool(b)) => *b,
                _ => return Err(invalid("allow_agent must be boolean")),
            };
            let mut s = self.0.0.lock().unwrap();
            if !trusted && self.0.1.latched.load(Ordering::Acquire) {
                return Err(error("permission_denied", "agent creation is revoked"));
            }
            if s.shutdown || s.tabs.len() >= 64 {
                return Err(error("resource_limit", "tab limit or engine shut down"));
            }
            s.next_tab += 1;
            let id = format!("t{}", s.next_tab);
            s.tabs.insert(
                id.clone(),
                Arc::new(Tab::new(rows, cols, allow, id.clone(), self.0.1.clone())),
            );
            s.workspace.add(id.clone());
            return Ok(json!({"tab_id": id, "allow_agent": allow}));
        }
        if op == "list_tabs" {
            let s = self.0.0.lock().unwrap();
            let tabs: Vec<_> = s.tabs.iter().filter(|(_,t)| trusted || t.allowed.load(Ordering::Acquire)).map(|(id,t)| json!({"tab_id": id, "allow_agent": t.allowed.load(Ordering::Acquire), "command_id": t.commands.lock().unwrap().current})).collect();
            return Ok(json!({"tabs": tabs}));
        }
        let tab_id = string(args, "tab_id")?;
        let tab = self.tab(tab_id, trusted)?;
        let lease = self.lease(&tab);
        match op {
            "run" => {
                let values = args
                    .get("argv")
                    .and_then(Value::as_array)
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| invalid("argv must be a nonempty string array"))?;
                let argv: Vec<String> = values
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .filter(|s| !s.contains('\0'))
                            .map(str::to_owned)
                            .ok_or_else(|| invalid("argv must contain NUL-free strings"))
                    })
                    .collect::<Result<_, _>>()?;
                if argv[0].is_empty()
                    || argv.iter().map(|s| s.len() + 1).sum::<usize>() > 1024 * 1024
                {
                    return Err(invalid("invalid argv size"));
                }
                let cwd = string(args, "cwd")?;
                let cwd = Path::new(cwd);
                if !cwd.is_absolute()
                    || !cwd.is_dir()
                    || cwd.as_os_str().as_encoded_bytes().contains(&0)
                {
                    return Err(invalid("cwd must be an existing absolute directory"));
                }
                let _authority = self.0.1.authority.lock().unwrap();
                let _guard = tab.lifecycle.lock().unwrap();
                self.check_lease(&tab, lease, trusted)?;
                if tab.closed.load(Ordering::Acquire) {
                    return Err(error("not_found", "tab closed"));
                }
                {
                    let commands = tab.commands.lock().unwrap();
                    if let Some(c) = commands
                        .current
                        .as_ref()
                        .and_then(|id| commands.all.get(id))
                    {
                        if !c.state.lock().unwrap().done() {
                            return Err(error("busy", "tab already running"));
                        }
                    }
                    if commands.all.len() >= 1024 {
                        return Err(error("resource_limit", "command history limit reached"));
                    }
                }
                let previous = tab.process.lock().unwrap().take();
                if let Some(p) = previous {
                    p.stop();
                }
                let id = {
                    let mut s = self.0.0.lock().unwrap();
                    s.next_command += 1;
                    format!("c{}", s.next_command)
                };
                let command = Arc::new(Command::new(id.clone(), tab_id.into(), &tab)?);
                let size = tab.terminal.lock().unwrap().size;
                let process =
                    platform::spawn(&argv, cwd, size.0, size.1, command.clone(), tab.clone())
                        .map_err(|e| error("spawn_failed", e.to_string()))?;
                *tab.process.lock().unwrap() = Some(process);
                let mut commands = tab.commands.lock().unwrap();
                commands.current = Some(id.clone());
                commands.all.insert(id.clone(), command);
                Ok(json!({"command_id": id, "state": "running"}))
            }
            "wait" | "get_exit_code" => {
                let command = tab.command(string(args, "command_id")?)?;
                let mut s = command.state.lock().unwrap();
                if op == "wait" {
                    let deadline = Instant::now()
                        + Duration::from_millis(number(args, "timeout_ms", 0, 30000)?);
                    while !s.done() {
                        self.check_lease(&tab, lease, trusted)?;
                        let now = Instant::now();
                        if now >= deadline {
                            break;
                        }
                        let (next, _) = command
                            .changed
                            .wait_timeout(s, (deadline - now).min(Duration::from_millis(100)))
                            .unwrap();
                        s = next;
                    }
                }
                if let Some(failure) = &s.failure {
                    return Err(error("internal", failure.clone()));
                }
                self.check_lease(&tab, lease, trusted)?;
                Ok(s.status())
            }
            "get_output" => {
                let offset = number(args, "offset", 0, u64::MAX)?;
                let limit = number(args, "limit", 1, 65536)? as usize;
                let encoding = string(args, "encoding")?;
                if !["utf8", "base64"].contains(&encoding) {
                    return Err(invalid("invalid output encoding"));
                }
                let command = tab.command(string(args, "command_id")?)?;
                let s = command.state.lock().unwrap();
                if let Some(failure) = &s.failure {
                    return Err(error("internal", failure.clone()));
                }
                if !trusted && s.masking_failed {
                    return Err(error("masking_unavailable", "masking policy unavailable"));
                }
                let (output, output_start) = if trusted {
                    (&s.output, s.total - s.output.len() as u64)
                } else {
                    (&s.safe, s.safe_total - s.safe.len() as u64)
                };
                // Preserve v1 raw coordinates and retention metadata. An unsafe
                // pending suffix yields no bytes, not a different offset space.
                let total = s.total;
                let start = (s.total - s.output.len() as u64).max(output_start);
                if offset < start {
                    return Err(error(
                        "output_evicted",
                        format!("earliest available offset: {start}"),
                    ));
                }
                if offset > total {
                    return Err(invalid("offset exceeds total bytes"));
                }
                let bytes: Vec<u8> = output
                    .iter()
                    .skip((offset - output_start) as usize)
                    .take(limit)
                    .copied()
                    .collect();
                let next = offset + bytes.len() as u64;
                let data = if encoding == "base64" {
                    base64::engine::general_purpose::STANDARD.encode(&bytes)
                } else {
                    String::from_utf8_lossy(&bytes).into_owned()
                };
                self.check_lease(&tab, lease, trusted)?;
                Ok(
                    json!({"encoding": encoding, "data": data, "next_offset": next, "retained_start": start, "total_bytes": total, "truncated": start != 0, "eof": s.done() && next == total}),
                )
            }
            "list_blocks" => {
                let limit = number(args, "limit", 1, 64)? as usize;
                let command = match args.get("command_id") {
                    None | Some(Value::Null) => None,
                    Some(Value::String(id)) => {
                        tab.command(id)?;
                        Some(id.as_str())
                    }
                    _ => return Err(invalid("command_id must be string or null")),
                };
                let after = match args.get("after") {
                    None | Some(Value::Null) => None,
                    Some(Value::String(id)) => Some(id.as_str()),
                    _ => return Err(invalid("after must be string or null")),
                };
                let (mut blocks, evicted) = if trusted {
                    tab.terminal.lock().unwrap().blocks()
                } else {
                    tab.safe_terminal.lock().unwrap().blocks()
                };
                if !trusted {
                    for block in &mut blocks {
                        let complete = !block.truncated
                            && (block.complete
                                || tab
                                    .command(&block.command_id)
                                    .ok()
                                    .is_some_and(|c| c.state.lock().unwrap().done()));
                        self.0.1.policy.block(block, complete)?;
                    }
                }
                let start = if let Some(id) = after {
                    match blocks.iter().position(|b| b.block_id == id) {
                        Some(i) => i + 1,
                        None => return Err(error("not_found", "block cursor no longer retained")),
                    }
                } else {
                    0
                };
                let matching: Vec<_> = blocks
                    .into_iter()
                    .skip(start)
                    .filter(|b| command.is_none_or(|id| b.command_id == id))
                    .collect();
                let mut page = Vec::new();
                let mut budget = 256 * 1024;
                for mut b in matching.iter().take(limit).cloned() {
                    if budget == 0 {
                        break;
                    }
                    if b.output.len() > budget {
                        let mut end = budget;
                        while !b.output.is_char_boundary(end) {
                            end -= 1;
                        }
                        b.output.truncate(end);
                        b.truncated = true;
                    }
                    budget -= b.output.len();
                    page.push(b);
                }
                let next = page
                    .last()
                    .map(|b| b.block_id.clone())
                    .or_else(|| after.map(str::to_owned));
                self.check_lease(&tab, lease, trusted)?;
                Ok(
                    json!({"has_more": matching.len() > page.len(), "blocks": page, "next_after": next, "evicted": evicted}),
                )
            }
            "close_tab" => {
                let _authority = self.0.1.authority.lock().unwrap();
                self.check_lease(&tab, lease, trusted)?;
                tab.close();
                let mut s = self.0.0.lock().unwrap();
                if s.tabs.remove(tab_id).is_some() {
                    s.workspace.remove(tab_id);
                }
                Ok(json!({"closed": true}))
            }
            _ => unreachable!(),
        }
    }
}
