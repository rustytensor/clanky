use crate::{Cell, TerminalSnapshot};
use clanky_vt::{
    Term,
    event::{Event, EventListener},
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point},
    term::{Config, TermMode, cell::Flags},
    vte::{
        self,
        ansi::{self, Color},
    },
};
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::mpsc::{self, Receiver, SyncSender},
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
#[derive(Clone)]
struct Listener(SyncSender<Vec<u8>>);
impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(s) = event {
            let _ = self.0.try_send(s.into_bytes());
        }
    }
}
#[derive(Clone, Serialize)]
pub(crate) struct Block {
    pub block_id: String,
    pub command_id: String,
    pub command: Option<String>,
    pub output: String,
    pub complete: bool,
    pub exit_code: Option<i32>,
    pub truncated: bool,
}
#[derive(Default)]
struct Observer {
    blocks: VecDeque<Block>,
    active: Option<Block>,
    next: u64,
    evicted: u64,
    command_id: String,
    cr: bool,
    prompt_waiting: bool,
    kitty: crate::kitty::Kitty,
    cursor: (usize, usize),
    viewport: (usize, usize),
    responses: Option<SyncSender<Vec<u8>>>,
}
impl Observer {
    fn seal(&mut self, code: Option<i32>, complete: bool) {
        if let Some(mut b) = self.active.take() {
            if b.output.ends_with('\n') {
                b.output.pop();
            }
            b.complete = complete;
            b.exit_code = code;
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
        if params.first() != Some(&b"133".as_slice()) {
            return;
        }
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
                    output: String::new(),
                    complete: false,
                    exit_code: None,
                    truncated: false,
                });
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
}
impl Terminal {
    pub fn new(rows: usize, cols: usize) -> Self {
        let (tx, responses) = mpsc::sync_channel(64);
        Self {
            term: Term::new(Config::default(), &Size(rows, cols), Listener(tx.clone())),
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
        }
    }
    pub fn feed(&mut self, command: &str, bytes: &[u8]) -> Vec<Vec<u8>> {
        if !bytes.is_empty() {
            self.generation += 1;
        }
        self.observer.command_id = command.into();
        // Keep the upstream ANSI processor and observer in lockstep. The APC
        // callback sees the cursor at its event, even with later text in this chunk.
        for byte in bytes {
            let byte = std::slice::from_ref(byte);
            self.processor.advance(&mut self.term, byte);
            let point = self.term.grid().cursor.point;
            self.observer.cursor = (point.line.0.max(0) as usize, point.column.0);
            self.parser.advance(&mut self.observer, byte);
        }
        self.responses.try_iter().collect()
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
        self.size = Size(rows, cols);
        self.term.resize(self.size);
        self.observer.viewport = (rows, cols);
    }
    pub fn blocks(&self) -> (Vec<Block>, u64) {
        let mut b: Vec<_> = self.observer.blocks.iter().cloned().collect();
        if let Some(active) = &self.observer.active {
            b.push(active.clone());
        }
        (b, self.observer.evicted)
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
