mod search;

use crate::{Cell, CellColorSource, ColorSource, TerminalSnapshot};
use clanky_vt::{
    Term,
    event::{Event, EventListener},
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point},
    term::{ClipboardType, Config, Osc52, TermMode, cell::Flags},
    vte::{
        self,
        ansi::{self, Color},
    },
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, mpsc::{self, Receiver, SyncSender}},
    time::Instant,
};
#[derive(Clone, Copy)]
pub(crate) struct Size(pub usize, pub usize);
impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.0
    }
    fn screen_lines(&self) -> usize {
        self.0
    }
    fn columns(&self) -> usize {
        self.1
    }
}
type ClipboardFormatter = Arc<dyn Fn(&str) -> String + Sync + Send + 'static>;
enum ClipboardRequest {
    Write(String),
    Read(ClipboardFormatter),
}
#[derive(Default)]
struct ClipboardState {
    command: String,
    next: u64,
    queue: VecDeque<(u64, String, ClipboardRequest)>,
    pending: BTreeMap<u64, (String, ClipboardFormatter)>,
}
#[derive(Clone)]
struct Listener(SyncSender<Vec<u8>>, Arc<Mutex<ClipboardState>>);
impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let request = match event {
            Event::PtyWrite(s) => {
                let _ = self.0.try_send(s.into_bytes());
                return;
            }
            Event::ClipboardStore(ClipboardType::Clipboard, s) if s.len() <= 1024 * 1024 => ClipboardRequest::Write(s),
            Event::ClipboardLoad(ClipboardType::Clipboard, formatter) => ClipboardRequest::Read(formatter),
            _ => return,
        };
        let mut state = self.1.lock().unwrap();
        if state.queue.len() >= 8 || state.command.is_empty() { return; }
        let Some(id) = state.next.checked_add(1) else { return; };
        state.next = id;
        let command = state.command.clone();
        state.queue.push_back((id, command, request));
    }
}
#[derive(Clone, Serialize)]
pub(crate) struct Block {
    pub block_id: String,
    pub command_id: String,
    pub command: Option<String>,
    /// Absolute retained-grid lines; null means absent, evicted or invalidated.
    pub prompt_line: Option<u64>,
    pub command_line: Option<u64>,
    pub end_line: Option<u64>,
    pub output: String,
    pub complete: bool,
    pub exit_code: Option<i32>,
    /// Monotonic C..D elapsed time; null for active or interrupted blocks.
    pub duration_ms: Option<u64>,
    #[serde(skip)]
    pub focused_tab_at_completion: Option<String>,
    pub truncated: bool,
    /// Wall clock at the C marker (Unix ms), for saved history.
    #[serde(skip)]
    pub started_unix_ms: i64,
    /// OSC 7 directory reported before the C marker, if any.
    #[serde(skip)]
    pub reported_cwd: Option<String>,
    /// (cwd, actor) for saved history, filled by the tab when it first sees the block.
    #[serde(skip)]
    pub origin: Option<(Option<String>, String)>,
}
#[derive(Serialize)]
pub(crate) struct BlockEvent {
    pub block_id: String,
    pub complete: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub focused_tab_at_completion: Option<String>,
}
#[derive(Default)]
struct Observer {
    cwd: Option<String>,
    blocks: VecDeque<Block>,
    active: Option<Block>,
    started: Option<Instant>,
    focus: Option<Arc<Mutex<Option<(u64, String)>>>>,
    next: u64,
    evicted: u64,
    command_id: String,
    cr: bool,
    prompt_waiting: bool,
    marker: Option<u8>,
    prompt_start: Option<u64>,
    command_start: Option<(u64, usize)>,
    command_starts: VecDeque<String>,
    kitty: crate::kitty::Kitty,
    cursor: (usize, usize),
    viewport: (usize, usize),
    responses: Option<SyncSender<Vec<u8>>>,
    /// Sealed blocks kept for saved history until the tab takes them.
    history: bool,
    sealed: Vec<Block>,
}
impl Observer {
    fn seal(&mut self, code: Option<i32>, complete: bool) {
        let started = self.started.take();
        if let Some(mut b) = self.active.take() {
            if b.output.ends_with('\n') {
                b.output.pop();
            }
            b.complete = complete;
            b.exit_code = code;
            if complete {
                b.focused_tab_at_completion = self.focus.as_ref().and_then(|focus| focus.lock().ok().and_then(|v| v.as_ref().map(|(_, tab)| tab.clone())));
            }
            b.duration_ms = if complete {
                started.map(|start| start.elapsed().as_millis().min(u64::MAX as u128) as u64)
            } else {
                None
            };
            if self.history { self.sealed.push(b.clone()); }
            self.blocks.push_back(b);
            while self.blocks.len() > 1024 {
                self.blocks.pop_front();
                self.evicted += 1;
            }
        }
        self.cr = false;
    }
    fn push(&mut self, ch: char) {
        if let Some(b) = &mut self.active {
            if b.output.len() + ch.len_utf8() <= 1024 * 1024 {
                b.output.push(ch);
            } else {
                b.truncated = true;
            }
        }
    }
}
impl vte::Perform for Observer {
    fn apc_dispatch(&mut self, bytes: &[u8]) {
        if let Some(reply) = self.kitty.dispatch(bytes, self.cursor, self.viewport) {
            if let Some(tx) = &self.responses {
                let _ = tx.try_send(reply);
            }
        }
    }
    fn print(&mut self, ch: char) {
        self.cr = false;
        self.push(ch);
    }
    fn execute(&mut self, byte: u8) {
        match byte {
            b'\r' => {
                self.push('\n');
                self.cr = true;
            }
            b'\n' => {
                if !self.cr {
                    self.push('\n');
                }
                self.cr = false;
            }
            b'\t' => {
                self.cr = false;
                self.push('\t');
            }
            _ => (),
        }
    }
    fn osc_dispatch(&mut self, params: &[&[u8]], _: bool) {
        if params.first() == Some(&b"7".as_slice()) {
            // vte splits OSC parameters at semicolons, which can also occur
            // literally in a file URL path. Reassemble only this OSC payload.
            let payload = params[1..].join(&b';');
            if let Some(cwd) = crate::platform::local_osc7_cwd(&payload) {
                self.cwd = Some(cwd);
            }
            return;
        }
        if params.first() != Some(&b"133".as_slice()) {
            return;
        }
        self.marker = match params.get(1).copied() {
            Some(b"A") => Some(b'A'), Some(b"B") => Some(b'B'),
            Some(b"C") => Some(b'C'),
            Some(b"D") if self.active.is_some() => Some(b'D'), _ => None,
        };
        match params.get(1).copied() {
            Some(b"C") => {
                self.prompt_waiting = false;
                self.seal(None, false);
                while self.blocks.len() >= 1024 {
                    self.blocks.pop_front();
                    self.evicted += 1;
                }
                self.next += 1;
                self.active = Some(Block {
                    block_id: format!("b{}", self.next),
                    command_id: self.command_id.clone(),
                    command: None,
                    prompt_line: None,
                    command_line: None,
                    end_line: None,
                    output: String::new(),
                    complete: false,
                    exit_code: None,
                    duration_ms: None,
                    focused_tab_at_completion: None,
                    truncated: false,
                    started_unix_ms: crate::history::now_ms(),
                    reported_cwd: self.cwd.clone(),
                    origin: None,
                });
                self.started = Some(Instant::now());
            }
            Some(b"D") => {
                self.prompt_waiting = false;
                let code = params
                    .get(2)
                    .and_then(|p| std::str::from_utf8(p).ok())
                    .and_then(|s| s.parse().ok());
                self.seal(code, true);
            }
            Some(b"A" | b"B") => {
                self.prompt_waiting = true;
                self.seal(None, false);
            }
            _ => (),
        }
    }
}
pub(crate) struct Terminal {
    term: Term<Listener>,
    processor: ansi::Processor,
    parser: vte::Parser,
    observer: Observer,
    pub size: Size,
    pub generation: u64,
    responses: Receiver<Vec<u8>>,
    clipboard: Arc<Mutex<ClipboardState>>,
}
impl Terminal {
    pub fn new(rows: usize, cols: usize) -> Self {
        let (tx, responses) = mpsc::sync_channel(64);
        let clipboard = Arc::new(Mutex::new(ClipboardState::default()));
        Self {
            term: Term::new(Config { osc52: Osc52::CopyPaste, ..Config::default() }, &Size(rows, cols), Listener(tx.clone(), clipboard.clone())),
            processor: ansi::Processor::new(),
            parser: vte::Parser::new(),
            observer: Observer {
                responses: Some(tx),
                viewport: (rows, cols),
                ..Observer::default()
            },
            size: Size(rows, cols),
            generation: 0,
            responses,
            clipboard,
        }
    }
    pub fn clipboard_events(&mut self, current: Option<&str>, write: bool, read: bool,
        mask: &dyn Fn(&str) -> Result<String, String>) -> Result<serde_json::Value, String> {
        use base64::Engine as _;
        let mut state = self.clipboard.lock().unwrap();
        state.pending.retain(|_, (origin, _)| Some(origin.as_str()) == current);
        while let Some((id, origin, request)) = state.queue.pop_front() {
            if Some(origin.as_str()) != current { continue; }
            let event = match request {
                ClipboardRequest::Write(data) if write => serde_json::json!({"id":id,"kind":"write","data":base64::engine::general_purpose::STANDARD.encode(mask(&data)?.as_bytes())}),
                ClipboardRequest::Read(formatter) if read => {
                    // A missing desktop helper must not permanently fill the queue.
                    if state.pending.len() >= 8 {
                        if let Some(oldest) = state.pending.keys().next().copied() {
                            state.pending.remove(&oldest);
                        }
                    }
                    state.pending.insert(id, (origin, formatter));
                    serde_json::json!({"id":id,"kind":"read"})
                }
                _ => continue,
            };
            return Ok(serde_json::json!({"events":[event]}));
        }
        Ok(serde_json::json!({"events":[]}))
    }
    pub fn clipboard_reply(&mut self, current: Option<&str>, id: u64, data: &str,
        mask: &dyn Fn(&str) -> Result<String, String>) -> Result<Vec<u8>, String> {
        use base64::Engine as _;
        // Consume before validation: even malformed or expired replies are one-use.
        let (origin, formatter) = self.clipboard.lock().unwrap().pending.remove(&id)
            .ok_or_else(|| "Unknown or expired clipboard request".to_owned())?;
        if Some(origin.as_str()) != current { return Err("Clipboard command changed".into()); }
        if data.len() > 65536usize.div_ceil(3) * 4 { return Err("Clipboard reply exceeds 64 KiB".into()); }
        let bytes = base64::engine::general_purpose::STANDARD.decode(data)
            .map_err(|_| "Invalid clipboard reply base64".to_owned())?;
        if bytes.len() > 65536 { return Err("Clipboard reply exceeds 64 KiB".into()); }
        let text = std::str::from_utf8(&bytes).map_err(|_| "Clipboard reply is not UTF-8".to_owned())?;
        Ok(formatter(&mask(text)?).into_bytes())
    }
    pub fn feed(&mut self, command: &str, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.feed_masked(command, bytes, &|_, _, _| false)
    }
    pub(crate) fn feed_masked(
        &mut self, command: &str, bytes: &[u8],
        mask: &dyn Fn(&mut [String], &[bool], Option<(usize, usize)>) -> bool,
    ) -> Vec<Vec<u8>> {
        if !bytes.is_empty() {
            self.generation += 1;
        }
        self.observer.command_id = command.into();
        {
            let mut state = self.clipboard.lock().unwrap();
            if state.command != command {
                // Do not complete a previous process's partial escape sequence.
                self.observer.command_start = None;
                self.observer.prompt_start = None;
                self.processor = ansi::Processor::new();
                self.parser = vte::Parser::new();
                state.queue.clear();
                state.pending.clear();
                state.command = command.into();
            }
        }
        // Keep the upstream ANSI processor and observer in lockstep. The APC
        // callback sees the cursor at its event, even with later text in this chunk.
        for byte in bytes {
            let byte = std::slice::from_ref(byte);
            let revision = self.term.grid().anchor_revision();
            let alternate = self.term.mode().contains(TermMode::ALT_SCREEN);
            self.processor.advance(&mut self.term, byte);
            if revision != self.term.grid().anchor_revision()
                || alternate != self.term.mode().contains(TermMode::ALT_SCREEN) {
                self.invalidate_anchors();
            }
            let point = self.term.grid().cursor.point;
            self.observer.cursor = (point.line.0.max(0) as usize, point.column.0);
            self.parser.advance(&mut self.observer, byte);
            if let Some(marker) = self.observer.marker.take() {
                self.record_marker(marker, mask);
            }
        }
        self.responses.try_iter().collect()
    }
    fn invalidate_anchors(&mut self) {
        self.observer.prompt_start = None;
        self.observer.command_start = None;
        for block in self.observer.blocks.iter_mut().chain(self.observer.active.iter_mut()) {
            block.prompt_line = None;
            block.command_line = None;
            block.end_line = None;
        }
    }
    fn record_marker(&mut self, marker: u8,
        mask: &dyn Fn(&mut [String], &[bool], Option<(usize, usize)>) -> bool,
    ) {
        if self.term.mode().contains(TermMode::ALT_SCREEN) { return; }
        let Some((row, byte)) = self.text_cursor() else { return; };
        let base = self.term.grid().evicted_lines();
        let line = base + row as u64;
        match marker {
            b'A' => { self.observer.prompt_start = Some(line); self.observer.command_start = None; }
            b'B' => {
                self.observer.prompt_start.get_or_insert(line);
                self.observer.command_start = Some((line, byte));
            }
            b'C' => {
                let command = self.observer.command_start.and_then(|(start, column)| {
                    let start = usize::try_from(start.checked_sub(base)?).ok()?;
                    if start > row { return None; }
                    let (_, _, mut lines, wraps, truncated) = self.text_context();
                    // The same logical context and undecided-prefix masking as `screen`.
                    if !mask(&mut lines, &wraps, Some((row, byte))) { return None; }
                    if truncated {
                        for (i, text) in lines.iter_mut().enumerate() {
                            *text = "*".repeat(text.len());
                            if !wraps[i] { break; }
                        }
                    }
                    let mut command = String::new();
                    for i in start..=row {
                        let first = if i == start { column } else { 0 };
                        let last = if i == row { byte } else { lines.get(i)?.len() };
                        let part = lines.get(i)?.get(first..last)?;
                        if command.len().saturating_add(part.len()) > 65536 { return None; }
                        command.push_str(if wraps[i] { part } else { part.trim_end_matches(' ') });
                        if i < row && !wraps[i] { command.push('\n'); }
                    }
                    Some(command.trim_end_matches([' ', '\n']).to_owned())
                });
                if let Some(text)=command.as_ref().filter(|text|!text.trim().is_empty()) {
                    self.observer.command_starts.push_back(text.chars().take(128).collect());
                    while self.observer.command_starts.len()>20{self.observer.command_starts.pop_front();}
                }
                if let Some(block) = &mut self.observer.active {
                    block.command = command;
                    block.prompt_line = self.observer.prompt_start;
                    // No A/B means no card, even if a shell emits C alone.
                    block.command_line = self.observer.prompt_start.map(|_| line);
                }
                self.observer.command_start = None;
                self.observer.prompt_start = None;
            }
            b'D' => {
                if let Some(block) = self.observer.blocks.back_mut().filter(|b| b.complete) {
                    block.end_line = block.command_line.map(|_| line);
                }
            }
            _ => (),
        }
    }
    pub fn eof(&mut self) {
        self.observer.prompt_waiting = false;
        self.observer.seal(None, false);
    }
    pub(crate) fn prompt_waiting(&self) -> bool {
        self.observer.prompt_waiting
    }
    /// Clear stale prompt evidence when input is submitted, before the shell
    /// has had an opportunity to emit its next OSC marker.
    pub(crate) fn input_submitted(&mut self) {
        self.observer.prompt_waiting = false;
    }
    pub(crate) fn key_bytes(&self, key: &str) -> Result<Vec<u8>, String> {
        let application = self.term.mode().contains(TermMode::APP_CURSOR);
        let bytes: &[u8] = match key {
            "Enter" => b"\r",
            "Ctrl-C" => b"\x03",
            "Escape" => b"\x1b",
            "Tab" => b"\t",
            "Backspace" => b"\x7f",
            "Up" if application => b"\x1bOA",
            "Down" if application => b"\x1bOB",
            "Right" if application => b"\x1bOC",
            "Left" if application => b"\x1bOD",
            "Home" if application => b"\x1bOH",
            "End" if application => b"\x1bOF",
            "Up" => b"\x1b[A",
            "Down" => b"\x1b[B",
            "Right" => b"\x1b[C",
            "Left" => b"\x1b[D",
            "Home" => b"\x1b[H",
            "End" => b"\x1b[F",
            "Delete" => b"\x1b[3~",
            "PageUp" => b"\x1b[5~",
            "PageDown" => b"\x1b[6~",
            _ => return Err("unsupported key".into()),
        };
        Ok(bytes.to_vec())
    }
    pub(crate) fn bracketed_paste(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }
    pub(crate) fn paste_bytes(&self, text: &str) -> Vec<u8> {
        if self.bracketed_paste() {
            let mut bytes = Vec::with_capacity(text.len() + 12);
            bytes.extend_from_slice(b"\x1b[200~");
            bytes.extend_from_slice(text.as_bytes());
            bytes.extend_from_slice(b"\x1b[201~");
            bytes
        } else {
            text.as_bytes().to_vec()
        }
    }
    /// Current display viewport, matching snapshot coordinates. Policy and
    /// masking belong to the caller, before it slices or paginates text.
    pub(crate) fn screen_text(&self) -> Vec<String> {
        let offset = self.term.grid().display_offset();
        (0..self.size.0)
            .map(|row| {
                let mut text = String::new();
                for col in 0..self.size.1 {
                    let cell = &self.term.grid()
                        [Point::new(Line(row as i32 - offset as i32), Column(col))];
                    if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                        continue;
                    }
                    if cell.flags.contains(Flags::HIDDEN) {
                        text.push(' ');
                    } else {
                        text.push(cell.c);
                        if let Some(extra) = cell.zerowidth() {
                            text.extend(extra.iter().copied());
                        }
                    }
                }
                text.trim_end_matches(' ').to_owned()
            })
            .collect()
    }
    /// True means this row continues into the next without a newline.
    /// Callers masking logical text must join these rows before matching.
    /// Full raw-grid logical context; keep padding until masking has decided
    /// whether it belongs to a literal. Never feed replacement bytes to a grid.
    pub(crate) fn text_context(&self) -> (usize, usize, Vec<String>, Vec<bool>, bool) {
        let total = self.term.grid().history_size();
        let mut lines = Vec::new();
        let mut wraps = Vec::new();
        for row in -(total as i32)..self.size.0 as i32 {
            let mut text = String::new();
            for col in 0..self.size.1 {
                let cell = &self.term.grid()[Point::new(Line(row), Column(col))];
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                if cell.flags.contains(Flags::HIDDEN) {
                    text.push(' ');
                } else {
                    text.push(cell.c);
                    if let Some(extra) = cell.zerowidth() {
                        text.extend(extra.iter().copied());
                    }
                }
            }
            lines.push(text);
            wraps.push(
                self.term.grid()[Point::new(Line(row), Column(self.size.1 - 1))]
                    .flags
                    .contains(Flags::WRAPLINE),
            );
        }
        (
            total,
            self.term.grid().display_offset(),
            lines,
            wraps,
            total >= Config::default().scrolling_history,
        )
    }
    /// Byte position of the live cursor in text_context (not the scrolled view).
    pub(crate) fn text_cursor(&self) -> Option<(usize, usize)> {
        let cursor = &self.term.grid().cursor;
        let row = usize::try_from(cursor.point.line.0).ok()?;
        if row >= self.size.0 {
            return None;
        }
        let end = (cursor.point.column.0 + usize::from(cursor.input_needs_wrap)).min(self.size.1);
        let mut bytes = 0;
        for col in 0..end {
            let cell = &self.term.grid()[Point::new(Line(row as i32), Column(col))];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            if cell.flags.contains(Flags::HIDDEN) {
                bytes += 1;
            } else {
                bytes += cell.c.len_utf8();
                if let Some(extra) = cell.zerowidth() {
                    bytes += extra.iter().map(|c| c.len_utf8()).sum::<usize>();
                }
            }
        }
        Some((self.term.grid().history_size() + row, bytes))
    }
    pub(crate) fn screen_wraps(&self) -> Vec<bool> {
        let offset = self.term.grid().display_offset();
        (0..self.size.0)
            .map(|row| {
                self.term.grid()
                    [Point::new(Line(row as i32 - offset as i32), Column(self.size.1 - 1))]
                .flags
                .contains(Flags::WRAPLINE)
            })
            .collect()
    }
    pub(crate) fn history_wraps(&self) -> Vec<bool> {
        let total = self.term.grid().history_size();
        (0..total)
            .map(|row| {
                self.term.grid()
                    [Point::new(Line(row as i32 - total as i32), Column(self.size.1 - 1))]
                .flags
                .contains(Flags::WRAPLINE)
            })
            .collect()
    }
    pub fn resize(&mut self, rows: usize, cols: usize) {
        if self.size.0 == rows && self.size.1 == cols {
            return;
        }
        self.generation += 1;
        // Reflow re-wraps rows but keeps logical lines and the cursor: carry each anchor
        // as "n logical lines above the cursor's line" and find that line again after.
        // Anchors that are not at the start of a logical line cannot be carried exactly
        // and are dropped, as every anchor was before.
        let alternate = self.term.mode().contains(TermMode::ALT_SCREEN);
        let before = (!alternate).then(|| self.logical_layout());
        let mut carried = Vec::new();
        if let Some(layout) = &before {
            let distance = |line: Option<u64>| line.and_then(|line| layout.distance_above_cursor(line));
            for block in self.observer.blocks.iter().chain(self.observer.active.iter()) {
                carried.push([distance(block.prompt_line), distance(block.command_line), distance(block.end_line)]);
            }
            carried.push([distance(self.observer.prompt_start), distance(self.observer.command_start.map(|(line, _)| line)), None]);
        }
        let command_column = self.observer.command_start.map(|(_, column)| column);
        self.invalidate_anchors();
        self.size = Size(rows, cols);
        self.term.resize(self.size);
        self.observer.viewport = (rows, cols);
        if before.is_some() && !self.term.mode().contains(TermMode::ALT_SCREEN) {
            let after = self.logical_layout();
            let mut carried = carried.into_iter();
            for block in self.observer.blocks.iter_mut().chain(self.observer.active.iter_mut()) {
                let Some([prompt, command, end]) = carried.next() else { break; };
                block.prompt_line = prompt.and_then(|d| after.line_above_cursor(d));
                block.command_line = command.and_then(|d| after.line_above_cursor(d));
                block.end_line = end.and_then(|d| after.line_above_cursor(d));
            }
            if let Some([prompt, command, _]) = carried.next() {
                self.observer.prompt_start = prompt.and_then(|d| after.line_above_cursor(d));
                self.observer.command_start = command.and_then(|d| after.line_above_cursor(d)).zip(command_column);
            }
        }
    }
    /// Where logical lines start in the retained grid, and the cursor's logical line.
    fn logical_layout(&self) -> LogicalLayout {
        let grid = self.term.grid();
        let history = grid.history_size();
        let cols = self.size.1;
        let mut starts = Vec::new();
        let mut wrapped_before = false;
        for row in -(history as i32)..self.size.0 as i32 {
            if !wrapped_before { starts.push((row + history as i32) as u64); }
            wrapped_before = cols > 0 && grid[Point::new(Line(row), Column(cols - 1))].flags.contains(Flags::WRAPLINE);
        }
        let cursor_row = history as u64 + grid.cursor.point.line.0.max(0) as u64;
        let cursor = starts.partition_point(|start| *start <= cursor_row).saturating_sub(1);
        LogicalLayout { evicted: grid.evicted_lines(), starts, cursor }
    }
    pub fn cwd(&self) -> Option<String> {
        self.observer.cwd.clone()
    }
    pub fn set_focus_tracker(&mut self, focus: Arc<Mutex<Option<(u64, String)>>>) {
        self.observer.focus = Some(focus);
    }
    /// Saved history: turn recording of sealed blocks on or off.
    pub(crate) fn record_history(&mut self, on: bool) {
        self.observer.history = on;
        if !on { self.observer.sealed.clear(); }
    }
    /// Give each block seen for the first time its origin, then hand over the
    /// blocks sealed since the last call.
    pub(crate) fn take_sealed(&mut self, mut origin: impl FnMut(&Block) -> (Option<String>, String)) -> Vec<Block> {
        for block in self.observer.sealed.iter_mut().chain(self.observer.active.iter_mut()) {
            if block.origin.is_none() { block.origin = Some(origin(block)); }
        }
        std::mem::take(&mut self.observer.sealed)
    }
    pub(crate) fn needs_origin(&self) -> bool {
        self.observer.sealed.iter().chain(self.observer.active.iter()).any(|b| b.origin.is_none())
    }
    /// Seal a running block as interrupted (server shutdown).
    pub(crate) fn interrupt(&mut self) {
        self.observer.seal(None, false);
    }
    pub(crate) fn take_command_starts(&mut self)->Vec<String>{self.observer.command_starts.drain(..).collect()}
    pub fn block_events(&self) -> Vec<BlockEvent> {
        self.observer.blocks.iter().chain(self.observer.active.iter()).map(|b| BlockEvent {
            block_id: b.block_id.clone(), complete: b.complete, exit_code: b.exit_code, duration_ms: b.duration_ms,
            focused_tab_at_completion: b.focused_tab_at_completion.clone(),
        }).collect()
    }
    pub fn blocks(&self) -> (Vec<Block>, u64) {
        let mut b: Vec<_> = self.observer.blocks.iter().cloned().collect();
        if let Some(active) = &self.observer.active {
            b.push(active.clone());
        }
        let oldest = self.term.grid().evicted_lines();
        for block in &mut b {
            for anchor in [&mut block.prompt_line, &mut block.command_line, &mut block.end_line] {
                if anchor.is_some_and(|line| line < oldest) { *anchor = None; }
            }
        }
        (b, self.observer.evicted)
    }
    /// Presentation metadata captured under the same lock as the snapshot.
    pub(crate) fn card_metadata(&self) -> serde_json::Value {
        let top = self.viewport_line();
        let oldest = self.term.grid().evicted_lines();
        let blocks: Vec<_> = self.observer.blocks.iter().chain(self.observer.active.iter())
            .filter(|b| b.prompt_line.is_some_and(|p| p >= oldest)
                && b.command_line.is_some_and(|c| c >= top && c < top + self.size.0 as u64))
            .take(64).map(|b| serde_json::json!({
                "block_id":b.block_id,"command":b.command,"prompt_line":b.prompt_line,
                "command_line":b.command_line,"end_line":b.end_line,
                "complete":b.complete,"exit_code":b.exit_code,"duration_ms":b.duration_ms
            })).collect();
        serde_json::json!({"viewport_line":top,"blocks":blocks})
    }
    /// Absolute line at the top of the displayed viewport.
    pub(crate) fn viewport_line(&self) -> u64 {
        self.term.grid().evicted_lines()
            + (self.term.grid().history_size() - self.term.grid().display_offset()) as u64
    }
    pub fn scroll(&mut self, delta: i32) -> usize {
        let before = self.term.grid().display_offset();
        // Clamp first: the upstream Delta implementation adds using i32.
        let target = (before as i64 + delta as i64).clamp(0, self.term.grid().history_size() as i64)
            as usize;
        self.term
            .scroll_display(Scroll::Delta(target as i32 - before as i32));
        if before != target {
            self.generation += 1;
        }
        target
    }
    pub fn history(&self, offset: usize, limit: usize) -> (usize, Vec<String>) {
        let total = self.term.grid().history_size();
        let mut lines = Vec::new();
        for index in offset..offset.saturating_add(limit).min(total) {
            let row = Line(index as i32 - total as i32);
            let mut text = String::new();
            for col in 0..self.size.1 {
                let cell = &self.term.grid()[Point::new(row, Column(col))];
                if !cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    text.push(cell.c);
                    if let Some(extra) = cell.zerowidth() {
                        text.extend(extra.iter().copied());
                    }
                }
            }
            lines.push(text.trim_end_matches(' ').to_owned());
        }
        (total, lines)
    }
    /// Replace secret-bearing cells using full masked logical context. Keep the
    /// raw grid, widths, colours and cursor intact; never parse replacement stars.
    pub(crate) fn masked_snapshot(&self, lines: &[String]) -> TerminalSnapshot {
        let mut snapshot = self.snapshot();
        let grid = self.term.grid();
        let history = grid.history_size();
        let offset = grid.display_offset();
        for row in 0..self.size.0 {
            let safe = lines[history - offset + row].as_bytes();
            let mut byte = 0;
            for col in 0..self.size.1 {
                let cell = &grid[Point::new(Line(row as i32 - offset as i32), Column(col))];
                if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    continue;
                }
                let len = cell_text_len(cell);
                let original: String = if cell.flags.contains(Flags::HIDDEN) {
                    " ".into()
                } else {
                    std::iter::once(cell.c).chain(cell.zerowidth().into_iter().flatten().copied()).collect()
                };
                if safe.get(byte..byte + len) != Some(original.as_bytes()) {
                    snapshot.cells[row * self.size.1 + col].ch = '*';
                    if cell.flags.contains(Flags::WIDE_CHAR) && col + 1 < self.size.1 {
                        snapshot.cells[row * self.size.1 + col + 1].ch = '*';
                    }
                }
                byte += len;
            }
        }
        snapshot
    }
    pub fn snapshot(&self) -> TerminalSnapshot {
        let mut cells = Vec::with_capacity(self.size.0 * self.size.1);
        let offset = self.term.grid().display_offset();
        for r in 0..self.size.0 {
            for c in 0..self.size.1 {
                let cell = &self.term.grid()[Point::new(Line(r as i32 - offset as i32), Column(c))];
                let mut fg = color(cell.fg, true);
                let mut bg = color(cell.bg, false);
                if cell.flags.contains(Flags::INVERSE) {
                    std::mem::swap(&mut fg, &mut bg);
                }
                cells.push(Cell {
                    ch: if cell
                        .flags
                        .intersects(Flags::HIDDEN | Flags::WIDE_CHAR_SPACER)
                    {
                        ' '
                    } else {
                        cell.c
                    },
                    fg,
                    bg,
                    color_source: Some(CellColorSource {
                        fg: color_source(cell.fg), bg: color_source(cell.bg),
                        inverse: cell.flags.contains(Flags::INVERSE),
                    }),
                });
            }
        }
        let point = self.term.grid().cursor.point;
        let cursor =
            if offset == 0 && self.term.mode().contains(TermMode::SHOW_CURSOR) && point.line.0 >= 0
            {
                Some((point.line.0 as usize, point.column.0))
            } else {
                None
            };
        TerminalSnapshot {
            rows: self.size.0,
            cols: self.size.1,
            cells,
            cursor,
            images: if offset == 0 {
                self.observer.kitty.snapshot(self.size.0, self.size.1)
            } else {
                Vec::new()
            },
        }
    }
}
fn cell_text_len(cell: &clanky_vt::term::cell::Cell) -> usize {
    if cell.flags.contains(Flags::HIDDEN) { 1 } else {
        cell.c.len_utf8() + cell.zerowidth().into_iter().flatten().map(|c| c.len_utf8()).sum::<usize>()
    }
}
fn color_source(color: Color) -> ColorSource {
    match color {
        Color::Spec(_) => ColorSource::Truecolor,
        Color::Indexed(index) => ColorSource::Indexed(index),
        Color::Named(name) if (name as usize) < 256 => ColorSource::Indexed(name as u8),
        Color::Named(_) => ColorSource::Default,
    }
}
fn color(c: Color, foreground: bool) -> [u8; 3] {
    match c {
        Color::Spec(rgb) => [rgb.r, rgb.g, rgb.b],
        Color::Named(n) => indexed(n as usize, foreground),
        Color::Indexed(i) => indexed(i as usize, foreground),
    }
}
fn indexed(i: usize, foreground: bool) -> [u8; 3] {
    const PALETTE: [[u8; 3]; 16] = [
        [0, 0, 0],
        [255, 0, 0],
        [0, 205, 0],
        [205, 205, 0],
        [0, 0, 238],
        [205, 0, 205],
        [0, 205, 205],
        [229, 229, 229],
        [127, 127, 127],
        [255, 0, 0],
        [0, 255, 0],
        [255, 255, 0],
        [92, 92, 255],
        [255, 0, 255],
        [0, 255, 255],
        [255, 255, 255],
    ];
    match i {
        0..=15 => PALETTE[i],
        16..=231 => {
            let v = i - 16;
            let f = |x: usize| if x == 0 { 0 } else { 55 + x as u8 * 40 };
            [f(v / 36), f(v / 6 % 6), f(v % 6)]
        }
        232..=255 => [8 + (i as u8 - 232) * 10; 3],
        256 | 258 => [229, 229, 229],
        257 => [0, 0, 0],
        _ => {
            if foreground {
                [229, 229, 229]
            } else {
                [0, 0, 0]
            }
        }
    }
}
pub fn fixture() -> TerminalSnapshot {
    let mut t = Terminal::new(24, 80);
    t.feed("", "\x1b[31m█\x1b[?25l".as_bytes());
    t.snapshot()
}

/// Logical-line view of the retained grid used to carry anchors across a resize.
struct LogicalLayout { evicted: u64, starts: Vec<u64>, cursor: usize }
impl LogicalLayout {
    /// Logical lines between an anchor's line and the cursor's, if the anchor starts a
    /// logical line that is still retained and not below the cursor.
    fn distance_above_cursor(&self, line: u64) -> Option<usize> {
        let row = line.checked_sub(self.evicted)?;
        let index = self.starts.binary_search(&row).ok()?;
        self.cursor.checked_sub(index)
    }
    fn line_above_cursor(&self, distance: usize) -> Option<u64> {
        let index = self.cursor.checked_sub(distance)?;
        self.starts.get(index).map(|row| self.evicted + row)
    }
}
