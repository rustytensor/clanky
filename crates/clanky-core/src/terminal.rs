use crate::{Cell, TerminalSnapshot};
use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    grid::Dimensions,
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
                let code = params
                    .get(2)
                    .and_then(|p| std::str::from_utf8(p).ok())
                    .and_then(|s| s.parse().ok());
                self.seal(code, true);
            }
            Some(b"A" | b"B") => {
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
    responses: Receiver<Vec<u8>>,
}
impl Terminal {
    pub fn new(rows: usize, cols: usize) -> Self {
        let (tx, responses) = mpsc::sync_channel(64);
        Self {
            term: Term::new(Config::default(), &Size(rows, cols), Listener(tx)),
            processor: ansi::Processor::new(),
            parser: vte::Parser::new(),
            observer: Observer::default(),
            size: Size(rows, cols),
            responses,
        }
    }
    pub fn feed(&mut self, command: &str, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.processor.advance(&mut self.term, bytes);
        self.observer.command_id = command.into();
        self.parser.advance(&mut self.observer, bytes);
        self.responses.try_iter().collect()
    }
    pub fn eof(&mut self) {
        self.observer.seal(None, false);
    }
    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.size = Size(rows, cols);
        self.term.resize(self.size);
    }
    pub fn blocks(&self) -> (Vec<Block>, u64) {
        let mut b: Vec<_> = self.observer.blocks.iter().cloned().collect();
        if let Some(active) = &self.observer.active {
            b.push(active.clone());
        }
        (b, self.observer.evicted)
    }
    pub fn snapshot(&self) -> TerminalSnapshot {
        let mut cells = Vec::with_capacity(self.size.0 * self.size.1);
        for r in 0..self.size.0 {
            for c in 0..self.size.1 {
                let cell = &self.term.grid()[Point::new(Line(r as i32), Column(c))];
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
        let cursor = if self.term.mode().contains(TermMode::SHOW_CURSOR) && point.line.0 >= 0 {
            Some((point.line.0 as usize, point.column.0))
        } else {
            None
        };
        TerminalSnapshot {
            rows: self.size.0,
            cols: self.size.1,
            cells,
            cursor,
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
