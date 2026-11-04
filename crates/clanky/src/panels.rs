//! Cell-based controls drawn by the existing terminal renderer.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clanky_api::Client;
use clanky_core::{TerminalSnapshot, platform::Clipboard};
use serde_json::{Value, json};
use winit::keyboard::{Key, NamedKey};

const ACTIONS: [&str; 12] = [
    "Open file",
    "Load changes",
    "Compose",
    "copy",
    "collapse",
    "rerun",
    "send",
    "task_update",
    "select",
    "view",
    "tasks",
    "task_get",
];

#[derive(Clone)]
enum Hit {
    Block(String, usize, bool),
    Action(&'static str),
    Task(String),
    Next,
    Previous,
}
struct Prompt {
    action: &'static str,
    args: Value,
    text: String,
}

pub struct LocalRequest {
    pub action: &'static str,
    pub tab: String,
}

pub struct Panels {
    pub local: Option<LocalRequest>,
    pub visible: bool,
    pub open: bool,
    tab: String,
    view: Value,
    task: Option<String>,
    scroll: usize,
    after: Vec<Value>,
    lines: Vec<(String, Option<Hit>)>,
    choice: usize,
    prompt: Option<Prompt>,
    preedit: String,
    message: String,
    clipboard: Clipboard,
}
impl Default for Panels {
    fn default() -> Self {
        Self {
            local: None,
            visible: true,
            open: false,
            tab: String::new(),
            view: Value::Null,
            task: None,
            scroll: 0,
            after: Vec::new(),
            lines: Vec::new(),
            choice: 0,
            prompt: None,
            preedit: String::new(),
            message: String::new(),
            clipboard: Clipboard::default(),
        }
    }
}
impl Panels {
    pub fn width(&self, cols: usize) -> usize {
        if self.visible && cols >= 64 { 32 } else { 0 }
    }
    fn call(engine: &Client, action: &str, args: &Value) -> Result<Value, String> {
        engine
            .palette_action(action, args, true)
            .map_err(|e| e.message)
    }
    pub fn refresh(&mut self, engine: &Client, tab: Option<&str>) {
        // Pin the displayed target while a modal is open. A socket close or
        // focus change must never retarget an already offered action.
        if self.open {
            return;
        }
        let tab = tab.unwrap_or("");
        if self.tab != tab {
            self.tab = tab.into();
            self.after.clear();
            self.scroll = 0;
            self.task = None;
            self.prompt = None;
        }
        if tab.is_empty() {
            self.view = Value::Null;
            return;
        }
        let mut args = json!({"tab_id":tab,"limit":64});
        if let Some(after) = self.after.last() {
            args["after"] = after.clone();
        }
        match Self::call(engine, "view", &args) {
            Ok(view) => self.view = view,
            Err(e) => {
                self.view = Value::Null;
                self.message = e;
            }
        }
    }
    fn selected(&self) -> Result<Value, String> {
        let block = self.view["selection"]["block_id"]
            .as_str()
            .ok_or("Select a block first")?;
        Ok(json!({"tab_id":self.tab,"block_id":block}))
    }
    fn action(&mut self, engine: &Client, action: &'static str) -> Result<(), String> {
        if matches!(action, "Open file" | "Load changes" | "Compose") {
            self.local = Some(LocalRequest { action, tab: self.tab.clone() });
            self.open = false;
            self.prompt = None;
            self.preedit.clear();
            return Ok(());
        }
        let mut args = match action {
            "tasks" | "view" => json!({"tab_id":self.tab}),
            "task_update" | "task_get" => {
                json!({"tab_id":self.tab,"task_id":self.task.as_deref().ok_or("Select a task first")?})
            }
            _ => self.selected()?,
        };
        match action {
            "rerun" | "send" => {
                let text = if action == "rerun" {
                    json!({"argv":[],"cwd":"/"}).to_string()
                } else {
                    "Review block".into()
                };
                self.prompt = Some(Prompt { action, args, text });
                self.open = true;
                return Ok(());
            }
            "collapse" => {
                let collapsed = self.view["blocks"]
                    .as_array()
                    .and_then(|blocks| blocks.iter().find(|b| b["block_id"] == args["block_id"]))
                    .and_then(|b| b["collapsed"].as_bool())
                    .unwrap_or(false);
                args["collapsed"] = json!(!collapsed);
            }
            "task_update" => args["state"] = json!("done"),
            "select" => {
                let block = self.view["blocks"]
                    .as_array()
                    .and_then(|blocks| blocks.iter().find(|b| b["block_id"] == args["block_id"]))
                    .ok_or("Selected block is not on this page")?;
                if block["truncated"].as_bool().unwrap_or(false) {
                    return Err(
                        "Truncated preview: select an explicit byte range through the API".into(),
                    );
                }
                args["start"] = json!(0);
                args["end"] = json!(
                    block["output"]
                        .as_str()
                        .ok_or("Missing block output")?
                        .len()
                );
            }
            _ => {}
        }
        let result = Self::call(engine, action, &args)?;
        if action == "copy" {
            let bytes = STANDARD
                .decode(result["data"].as_str().ok_or("Missing copy data")?)
                .map_err(|e| e.to_string())?;
            self.clipboard
                .copy(&bytes)
                .map_err(|e| format!("Clipboard: {e}"))?;
        }
        self.message = if action == "copy" {
            "Clipboard helper requested".into()
        } else if action == "task_get" {
            format!("Task payload: {} bytes", result["bytes"])
        } else {
            format!("{action}: OK")
        };
        Ok(())
    }
    fn report(&mut self, result: Result<(), String>) {
        if let Err(e) = result {
            self.message = e;
        }
    }
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.prompt = None;
        self.preedit.clear();
    }
    pub fn text(&mut self, text: &str) {
        if let Some(prompt) = &mut self.prompt {
            if prompt.text.len().saturating_add(text.len()) <= 8192 {
                prompt.text.push_str(text);
            }
        }
    }
    pub fn preedit(&mut self, text: &str) {
        self.preedit = text.chars().take(128).collect();
    }
    pub fn key(&mut self, engine: &Client, key: &Key, text: Option<&str>, composing: bool) {
        if *key == Key::Named(NamedKey::Escape) {
            self.toggle();
            return;
        }
        if self.prompt.is_some() {
            match key {
                Key::Named(NamedKey::Backspace) if !composing => {
                    self.prompt.as_mut().unwrap().text.pop();
                }
                Key::Named(NamedKey::Enter) if !composing => {
                    let prompt = self.prompt.take().unwrap();
                    let result = (|| {
                        let mut args = prompt.args.clone();
                        if prompt.action == "rerun" {
                            let fields: Value =
                                serde_json::from_str(&prompt.text).map_err(|e| e.to_string())?;
                            let fields = fields
                                .as_object()
                                .ok_or("Enter a JSON object with argv and cwd")?;
                            if fields.len() != 2
                                || !fields.contains_key("argv")
                                || !fields.contains_key("cwd")
                            {
                                return Err("Expected only argv and cwd".into());
                            }
                            args["argv"] = fields["argv"].clone();
                            args["cwd"] = fields["cwd"].clone();
                        } else {
                            args["label"] = json!(prompt.text);
                        }
                        Self::call(engine, prompt.action, &args)?;
                        self.message = format!("{}: OK", prompt.action);
                        Ok(())
                    })();
                    if result.is_err() {
                        self.prompt = Some(prompt);
                    }
                    self.report(result);
                }
                _ if !composing => {
                    if let Some(text) = text {
                        if !text.chars().any(char::is_control) {
                            self.text(text);
                        }
                    }
                }
                _ => {}
            }
        } else {
            match key {
                Key::Named(NamedKey::ArrowUp) => {
                    self.choice = (self.choice + ACTIONS.len() - 1) % ACTIONS.len()
                }
                Key::Named(NamedKey::ArrowDown) => self.choice = (self.choice + 1) % ACTIONS.len(),
                Key::Named(NamedKey::Enter) => {
                    let result = self.action(engine, ACTIONS[self.choice]);
                    self.report(result);
                }
                _ => {}
            }
        }
    }
    pub fn scroll(&mut self, delta: i32) {
        self.scroll = self
            .scroll
            .saturating_add_signed(delta as isize)
            .min(self.lines.len().saturating_sub(1));
    }
    pub fn click(&mut self, engine: &Client, row: usize) {
        if self.open {
            return;
        }
        let hit = self
            .lines
            .get(self.scroll + row.saturating_sub(1))
            .and_then(|(_, hit)| hit.clone());
        let result = match hit {
            Some(Hit::Block(id, len, truncated)) => {
                if truncated || len > 64 * 1024 {
                    Err("Large/truncated block: select a byte range through the API".into())
                } else {
                    Self::call(
                        engine,
                        "select",
                        &json!({"tab_id":self.tab,"block_id":id,"start":0,"end":len}),
                    )
                    .map(|_| ())
                }
            }
            Some(Hit::Action(action)) => self.action(engine, action),
            Some(Hit::Task(id)) => {
                self.task = Some(id);
                Ok(())
            }
            Some(Hit::Next) => {
                self.after.push(self.view["next_after"].clone());
                self.scroll = 0;
                Ok(())
            }
            Some(Hit::Previous) => {
                self.after.pop();
                self.scroll = 0;
                Ok(())
            }
            None => Ok(()),
        };
        self.report(result);
    }
    pub fn draw(&mut self, frame: &mut TerminalSnapshot, width: usize) {
        self.lines.clear();
        self.lines.push(("[Open file]".into(), Some(Hit::Action("Open file"))));
        self.lines.push(("[Load changes]".into(), Some(Hit::Action("Load changes"))));
        self.lines.push(("[Compose]".into(), Some(Hit::Action("Compose"))));
        self.lines.push((format!("Blocks: {}", self.tab), None));
        if !self.after.is_empty() {
            self.lines
                .push(("< Previous page".into(), Some(Hit::Previous)));
        }
        if let Some(blocks) = self.view["blocks"].as_array() {
            for block in blocks {
                let Some(id) = block["block_id"].as_str() else {
                    continue;
                };
                let selected = self.view["selection"]["block_id"] == block["block_id"];
                let output = block["output"].as_str().unwrap_or("");
                self.lines.push((
                    format!("{} Block {id}", if selected { ">" } else { " " }),
                    Some(Hit::Block(
                        id.into(),
                        output.len(),
                        block["truncated"].as_bool().unwrap_or(false),
                    )),
                ));
                if !block["collapsed"].as_bool().unwrap_or(false) {
                    self.lines.push((
                        output
                            .chars()
                            .take(80)
                            .map(|c| if c.is_control() { ' ' } else { c })
                            .collect(),
                        Some(Hit::Block(
                            id.into(),
                            output.len(),
                            block["truncated"].as_bool().unwrap_or(false),
                        )),
                    ));
                }
                if selected {
                    for action in ["copy", "collapse", "rerun", "send"] {
                        self.lines
                            .push((format!("  [{action}]"), Some(Hit::Action(action))));
                    }
                }
            }
        }
        if self.view["has_more"] == true {
            self.lines.push(("Next page >".into(), Some(Hit::Next)));
        }
        self.lines.push(("Tasks (claimed / process)".into(), None));
        if let Some(tasks) = self.view["tasks"].as_array() {
            for task in tasks {
                let Some(id) = task["task_id"].as_str() else {
                    continue;
                };
                let chosen = self.task.as_deref() == Some(id);
                self.lines.push((
                    format!(
                        "{} {}: {}",
                        if chosen { ">" } else { " " },
                        task["label"]
                            .as_str()
                            .unwrap_or(id)
                            .chars()
                            .take(16)
                            .collect::<String>(),
                        task["state"].as_str().unwrap_or("?")
                    ),
                    Some(Hit::Task(id.into())),
                ));
                self.lines.push((
                    format!(
                        "  process {} exit {}",
                        task["process"]["state"], task["process"]["exit_code"]
                    ),
                    Some(Hit::Task(id.into())),
                ));
                if chosen {
                    self.lines
                        .push(("  [Mark done]".into(), Some(Hit::Action("task_update"))));
                }
            }
        }
        if width > 0 {
            let col = frame.cols - width;
            for (row, (text, _)) in self
                .lines
                .iter()
                .skip(self.scroll)
                .take(frame.rows.saturating_sub(2))
                .enumerate()
            {
                paint(frame, row + 1, col, width, text, false);
            }
            paint(frame, frame.rows - 1, col, width, &self.message, true);
        }
        if self.open {
            // Images are not allowed to paint over the modal input surface.
            frame.images.clear();
            frame.cursor = None;
            let target = format!(
                "Target {} / block {} / task {}",
                self.tab,
                self.view["selection"]["block_id"]
                    .as_str()
                    .unwrap_or("none"),
                self.task.as_deref().unwrap_or("none")
            );
            paint(frame, 0, 0, frame.cols, &target, true);
            paint(
                frame,
                1,
                0,
                frame.cols,
                "Palette: Up/Down, Enter; Escape cancels",
                true,
            );
            if let Some(prompt) = &self.prompt {
                paint(
                    frame,
                    2,
                    0,
                    frame.cols,
                    if prompt.action == "rerun" {
                        "Rerun NEW terminal: edit JSON argv + absolute cwd; Enter confirms"
                    } else {
                        "Send label (queues only); Enter confirms"
                    },
                    true,
                );
                // Show the end of a long editable prompt, not an unbounded snapshot.
                let tail: String = prompt
                    .text
                    .chars()
                    .rev()
                    .take(frame.cols.saturating_sub(1))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                paint(frame, 3, 0, frame.cols, &format!("{tail}_"), true);
                paint(frame, 4, 0, frame.cols, &self.preedit, true);
            } else {
                for (i, action) in ACTIONS.iter().enumerate() {
                    paint(
                        frame,
                        i + 2,
                        0,
                        frame.cols,
                        &format!("{} {action}", if i == self.choice { ">" } else { " " }),
                        i == self.choice,
                    );
                }
            }
            paint(frame, frame.rows - 1, 0, frame.cols, &self.message, true);
        }
    }
}
fn paint(
    frame: &mut TerminalSnapshot,
    row: usize,
    col: usize,
    width: usize,
    text: &str,
    selected: bool,
) {
    if row >= frame.rows {
        return;
    }
    let mut chars = text.chars();
    for x in col..(col + width).min(frame.cols) {
        let cell = &mut frame.cells[row * frame.cols + x];
        cell.ch = chars
            .next()
            .map(|c| if c.is_control() { ' ' } else { c })
            .unwrap_or(' ');
        cell.bg = if selected {
            [50, 80, 120]
        } else {
            [35, 40, 48]
        };
        cell.fg = [235, 235, 235];
    }
}
