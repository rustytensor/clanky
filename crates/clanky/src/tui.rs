//! Text-mode front end for the shared document and review core.

use std::{
    io::{self, Write},
    sync::Arc,
    ops::Range,
    panic::{self, AssertUnwindSafe},
    path::PathBuf,
    time::Duration,
};

use clanky_edit::{Document, HunkState, Location, Selection};
use crossterm::{
    cursor::Show,
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{CrosstermBackend, TestBackend},
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame, Terminal,
};

const HELP: &[&str] = &[
    "Arrows/Home/End/PgUp/PgDn move | Shift+movement selects",
    "Type/Enter/Tab replace selection | Backspace/Delete erase",
    "Ctrl+Z undo | Ctrl+Y redo | Ctrl+S save | Ctrl+Q quit",
    "Ctrl+C quit (asks if unsaved) | Ctrl+F find | Esc cancel",
    "Find: type query, Enter next (wraps), Esc closes",
    "Tab/Shift+Tab select review hunk (when no find prompt)",
    "F5 accept selected/next pending | F6 reject selected/next",
    "F7 accept all pending | F8 reject all pending",
    "F1 help | Accept changes memory only; Ctrl+S writes disk",
];

struct Editor {
    doc: Document,
    saved: String,
    originals: Vec<(u64, String)>,
    selected: Option<u64>,
    status: String,
    find: Option<String>,
    confirm: bool,
    help: bool,
    top: usize,
    left: usize,
}

/// Arguments are the words after `edit`; no display server is required.
pub fn run(args: &[String]) -> Result<(), String> {
    let mut editor = open(args)?;
    // Restore before the panic diagnostic is printed, not just during unwind.
    let previous = Arc::new(panic::take_hook());
    let diagnostic = Arc::clone(&previous);
    panic::set_hook(Box::new(move |info| {
        restore_terminal();
        diagnostic(info);
    }));
    let result = panic::catch_unwind(AssertUnwindSafe(|| interactive(&mut editor)));
    drop(panic::take_hook());
    let previous = Arc::try_unwrap(previous).unwrap_or_else(|_| unreachable!());
    panic::set_hook(previous);
    match result {
        Ok(result) => result,
        Err(payload) => panic::resume_unwind(payload),
    }
}

fn open(args: &[String]) -> Result<Editor, String> {
    let usage = "usage: clanky edit FILE[:LINE[:COL]] [--review FILE]";
    let target = args.first().ok_or(usage)?;
    let review = match &args[1..] {
        [] => None,
        [flag, path] if flag == "--review" => Some(path),
        _ => return Err(usage.into()),
    };
    let location = if target.rsplit_once(':').is_some_and(|(_, tail)| {
        !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit())
    }) {
        Location::parse(target).map_err(|e| e.to_string())?
    } else {
        Location { path: PathBuf::from(target), line: 1, column: 1 }
    };
    let mut doc = Document::open(&location.path).map_err(|e| e.to_string())?;
    let cursor = doc.offset(location.line, location.column).map_err(|e| e.to_string())?;
    doc.select(Selection { anchor: cursor, cursor }).map_err(|e| e.to_string())?;
    let mut originals = Vec::new();
    if let Some(path) = review {
        let bytes = std::fs::read(path).map_err(|e| format!("review {path}: {e}"))?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| format!("review {path}: {e}"))?;
        let hunks = value.get("hunks").and_then(|v| v.as_array())
            .ok_or("review must contain a hunks array")?;
        for hunk in hunks {
            let start = hunk.get("start").and_then(|v| v.as_u64())
                .and_then(|v| usize::try_from(v).ok()).ok_or("invalid review start")?;
            let end = hunk.get("end").and_then(|v| v.as_u64())
                .and_then(|v| usize::try_from(v).ok()).ok_or("invalid review end")?;
            let replacement = hunk.get("replacement").and_then(|v| v.as_str())
                .ok_or("invalid review replacement")?;
            let id = doc.propose(start..end, replacement).map_err(|e| e.to_string())?;
            originals.push((id, doc.text()[start..end].to_owned()));
        }
    }
    let saved = doc.text().to_owned();
    Ok(Editor {
        doc, saved, originals, selected: None,
        status: "F1 help | Ctrl+S save | Ctrl+Q quit".into(),
        find: None, confirm: false, help: false, top: 0, left: 0,
    })
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), DisableBracketedPaste, LeaveAlternateScreen, Show);
}

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) { restore_terminal(); }
}

fn interactive(editor: &mut Editor) -> Result<(), String> {
    let _signals = platform::SignalGuard::install()?;
    let _guard = TerminalGuard;
    enable_raw_mode().map_err(|e| e.to_string())?;
    execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)
        .map_err(|e| e.to_string())?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))
        .map_err(|e| e.to_string())?;
    terminal.clear().map_err(|e| e.to_string())?;
    loop {
        match platform::take_signal() {
            0 => (),
            2 => {
                if editor.doc.text() == editor.saved { return Ok(()); }
                editor.confirm = true;
            }
            signal => return Err(format!("editor interrupted by signal {signal}; unsaved edits were not written")),
        }
        let size = terminal.size().map_err(|e| e.to_string())?;
        editor.keep_visible(size.width, size.height);
        terminal.draw(|frame| render(frame, editor)).map_err(|e| e.to_string())?;
        if !event::poll(Duration::from_millis(200)).map_err(|e| e.to_string())? {
            match editor.doc.refresh() {
                Ok(true) => (),
                Ok(false) => editor.status = "disk conflict: reopen before saving/accepting".into(),
                Err(e) => editor.status = e.to_string(),
            }
            continue;
        }
        match event::read().map_err(|e| e.to_string())? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                match editor.key(key) {
                    Ok(true) => return Ok(()),
                    Ok(false) => (),
                    Err(error) => editor.status = error,
                }
            }
            Event::Paste(text) if !editor.confirm => {
                if let Some(query) = &mut editor.find { query.push_str(&text); }
                else if let Err(e) = editor.replace(&text) { editor.status = e; }
            }
            _ => (),
        }
    }
}

impl Editor {
    fn range(&self) -> Range<usize> {
        let s = self.doc.selection();
        s.anchor.min(s.cursor)..s.anchor.max(s.cursor)
    }

    fn replace(&mut self, text: &str) -> Result<(), String> {
        self.doc.edit(self.range(), text).map_err(|e| e.to_string())?;
        self.status = "modified | F1 help | Ctrl+S save | Ctrl+Q quit".into();
        Ok(())
    }

    fn key(&mut self, key: KeyEvent) -> Result<bool, String> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        if self.confirm {
            match key.code {
                KeyCode::Char('y' | 'Y') => return Ok(true),
                KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                    self.confirm = false;
                    self.status = "quit cancelled | F1 help".into();
                }
                _ => (),
            }
            return Ok(false);
        }
        if ctrl && matches!(key.code, KeyCode::Char('q' | 'c')) {
            if self.doc.text() == self.saved { return Ok(true); }
            self.confirm = true;
            return Ok(false);
        }
        if ctrl && key.code == KeyCode::Char('s') {
            self.doc.save().map_err(|e| e.to_string())?;
            self.saved = self.doc.text().to_owned();
            self.status = "saved | Ctrl+S save | Ctrl+Q quit".into();
            if let Some(path) = std::env::var_os("CLANKY_TUI_CAPTURE") {
                self.keep_visible(80, 24);
                capture(self, PathBuf::from(path))?;
            }
            return Ok(false);
        }
        if ctrl && key.code == KeyCode::Char('f') {
            self.find = Some(String::new());
            return Ok(false);
        }
        if let Some(query) = &mut self.find {
            match key.code {
                KeyCode::Esc => self.find = None,
                KeyCode::Enter => self.find_next(),
                KeyCode::Backspace => { query.pop(); }
                KeyCode::Char(c) if !ctrl && !key.modifiers.contains(KeyModifiers::ALT) => query.push(c),
                _ => (),
            }
            return Ok(false);
        }
        if ctrl {
            match key.code {
                KeyCode::Char('z') => { self.doc.undo().map_err(|e| e.to_string())?; }
                KeyCode::Char('y') => { self.doc.redo().map_err(|e| e.to_string())?; }
                _ => return Ok(false),
            }
            self.status = "undo/redo | F1 help | Ctrl+S save".into();
            return Ok(false);
        }
        match key.code {
            KeyCode::F(1) => self.help = !self.help,
            KeyCode::F(5) => self.decide(true, false)?,
            KeyCode::F(6) => self.decide(false, false)?,
            KeyCode::F(7) => self.decide(true, true)?,
            KeyCode::F(8) => self.decide(false, true)?,
            KeyCode::Tab | KeyCode::BackTab if self.doc.hunks().iter().any(|h| matches!(h.state, HunkState::Pending | HunkState::Stale)) => {
                self.cycle_hunk(shift || key.code == KeyCode::BackTab)?;
            }
            KeyCode::Tab => self.replace("\t")?,
            KeyCode::Enter => {
                let newline = if self.doc.text().contains("\r\n") { "\r\n" } else { "\n" };
                self.replace(newline)?;
            }
            KeyCode::Backspace | KeyCode::Delete => {
                let mut range = self.range();
                if range.is_empty() {
                    if key.code == KeyCode::Backspace { range.start = previous(self.doc.text(), range.start); }
                    else { range.end = next(self.doc.text(), range.end); }
                }
                self.doc.edit(range, "").map_err(|e| e.to_string())?;
                self.status = "modified | F1 help | Ctrl+S save".into();
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down |
            KeyCode::Home | KeyCode::End | KeyCode::PageUp | KeyCode::PageDown => self.move_cursor(key.code, shift)?,
            KeyCode::Esc => { self.help = false; self.selected = None; }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::ALT) => self.replace(&c.to_string())?,
            _ => (),
        }
        Ok(false)
    }

    fn move_cursor(&mut self, code: KeyCode, shift: bool) -> Result<(), String> {
        let s = self.doc.selection();
        let text = self.doc.text();
        let (line, column) = position(text, s.cursor);
        let starts = line_starts(text);
        let target = match code {
            KeyCode::Left => previous(text, s.cursor),
            KeyCode::Right => next(text, s.cursor),
            KeyCode::Home => starts[line],
            KeyCode::End => line_end(text, starts[line]),
            _ => {
                let target_line = match code {
                    KeyCode::Up => line.saturating_sub(1),
                    KeyCode::PageUp => line.saturating_sub(20),
                    KeyCode::PageDown => (line + 20).min(starts.len() - 1),
                    _ => (line + 1).min(starts.len() - 1),
                };
                let start = starts[target_line];
                let end = line_end(text, start);
                start + text[start..end].char_indices().map(|(i, _)| i)
                    .chain(std::iter::once(end - start)).nth(column).unwrap_or(end - start)
            }
        };
        self.doc.select(Selection { anchor: if shift { s.anchor } else { target }, cursor: target })
            .map_err(|e| e.to_string())
    }

    fn find_next(&mut self) {
        let Some(query) = self.find.as_deref().filter(|s| !s.is_empty()) else { return; };
        let text = self.doc.text();
        let start = self.doc.selection().cursor;
        let found = text[start..].find(query).map(|i| start + i)
            .or_else(|| text[..start].find(query));
        if let Some(start) = found {
            let _ = self.doc.select(Selection { anchor: start, cursor: start + query.len() });
            self.status = "match | Enter next | Esc close find".into();
        } else { self.status = "not found | Esc close find".into(); }
    }

    fn cycle_hunk(&mut self, backwards: bool) -> Result<(), String> {
        let hunks: Vec<_> = self.doc.hunks().iter().filter(|h| matches!(h.state, HunkState::Pending | HunkState::Stale)).collect();
        if hunks.is_empty() { return Ok(()); }
        let index = self.selected.and_then(|id| hunks.iter().position(|h| h.id == id));
        let index = match (index, backwards) {
            (Some(i), true) => (i + hunks.len() - 1) % hunks.len(),
            (Some(i), false) => (i + 1) % hunks.len(),
            (None, true) => hunks.len() - 1,
            (None, false) => 0,
        };
        let h = &hunks[index];
        let (id, state) = (h.id, h.state);
        self.selected = Some(id);
        let cursor = h.range.start.min(self.doc.text().len());
        // Decided/stale ranges need not track later edits, so clamp to a boundary.
        let mut cursor = cursor;
        while !self.doc.text().is_char_boundary(cursor) { cursor -= 1; }
        self.doc.select(Selection { anchor: cursor, cursor }).map_err(|e| e.to_string())?;
        self.status = format!("hunk {id} {state:?} | F5 accept | F6 reject | F1 help");
        Ok(())
    }

    fn decide(&mut self, accept: bool, all: bool) -> Result<(), String> {
        let ids: Vec<_> = self.doc.hunks().iter().filter(|h| h.state == HunkState::Pending)
            .map(|h| h.id).collect();
        let ids = if all { ids } else {
            match self.selected {
                Some(id) if ids.contains(&id) => vec![id],
                Some(_) => return Err("selected hunk is not pending".into()),
                None => ids.first().copied().into_iter().collect(),
            }
        };
        for id in ids {
            // Earlier accepted overlapping hunks may stale later proposals.
            if !self.doc.hunks().iter().any(|h| h.id == id && h.state == HunkState::Pending) { continue; }
            if accept { self.doc.accept(id) } else { self.doc.reject(id) }.map_err(|e| e.to_string())?;
        }
        self.selected = None;
        self.status = "review decision | F1 help | Ctrl+S save".into();
        Ok(())
    }

    fn keep_visible(&mut self, width: u16, height: u16) {
        let rows = display_rows(self);
        let cursor = rows.iter().position(|r| r.cursor.is_some()).unwrap_or(0);
        let available = height.saturating_sub(2).max(1) as usize;
        if cursor < self.top { self.top = cursor; }
        if cursor >= self.top + available { self.top = cursor + 1 - available; }
        let column = rows[cursor].cursor.unwrap_or(0);
        let available = width.max(1) as usize;
        if column < self.left { self.left = column; }
        if column >= self.left + available { self.left = column + 1 - available; }
    }
}

fn previous(text: &str, at: usize) -> usize {
    if at >= 2 && &text.as_bytes()[at - 2..at] == b"\r\n" { return at - 2; }
    text[..at].char_indices().next_back().map_or(0, |(i, _)| i)
}
fn next(text: &str, at: usize) -> usize {
    if text[at..].starts_with("\r\n") { return at + 2; }
    at + text[at..].chars().next().map_or(0, char::len_utf8)
}
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect()
}
fn line_end(text: &str, start: usize) -> usize {
    let end = text[start..].find('\n').map_or(text.len(), |i| start + i);
    if end < text.len() && text[start..end].ends_with('\r') { end - 1 } else { end }
}
fn position(text: &str, cursor: usize) -> (usize, usize) {
    let prefix = &text[..cursor];
    let line = prefix.bytes().filter(|b| *b == b'\n').count();
    let start = prefix.rfind('\n').map_or(0, |i| i + 1);
    (line, text[start..cursor].chars().count())
}

struct DisplayRow {
    line: Line<'static>,
    cursor: Option<usize>,
}

fn visible_char(c: char) -> String {
    match c {
        '\t' => "    ".into(),
        c if c.is_control() => "�".into(),
        c => c.to_string(),
    }
}

fn display_rows(editor: &Editor) -> Vec<DisplayRow> {
    let text = editor.doc.text();
    let selection = editor.range();
    let caret = editor.doc.selection().cursor;
    let mut rows = Vec::new();
    let starts = line_starts(text);
    for (index, &start) in starts.iter().enumerate() {
        let end = line_end(text, start);
        let mut spans = Vec::new();
        for (offset, c) in text[start..end].char_indices() {
            let style = if selection.contains(&(start + offset)) {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else { Style::default() };
            spans.push(Span::styled(visible_char(c), style));
        }
        let physical_end = starts.get(index + 1).copied().unwrap_or(text.len() + 1);
        let cursor = if start <= caret && caret < physical_end {
            let prefix: String = text[start..caret.min(end)].chars().map(visible_char).collect();
            Some(Line::from(prefix).width())
        } else { None };
        rows.push(DisplayRow { line: Line::from(spans), cursor });
        for hunk in editor.doc.hunks() {
            if !matches!(hunk.state, HunkState::Pending | HunkState::Stale) { continue; }
            let anchor = hunk.range.start.min(text.len());
            if anchor < start || anchor >= physical_end { continue; }
            let marker = if editor.selected == Some(hunk.id) { ">" } else { " " };
            let heading = format!("{marker} hunk {} {:?}", hunk.id, hunk.state);
            rows.push(DisplayRow { line: Line::styled(heading, Style::default().add_modifier(Modifier::BOLD)), cursor: None });
            let old = editor.originals.iter().find(|(id, _)| *id == hunk.id).map(|(_, s)| s.as_str()).unwrap_or("");
            for (prefix, value, color) in [("- ", old, Color::Red), ("+ ", hunk.replacement.as_str(), Color::Green)] {
                for line in value.split('\n') {
                    let clean: String = line.chars().map(visible_char).collect();
                    rows.push(DisplayRow { line: Line::styled(format!("{prefix}{clean}"), Style::default().fg(color)), cursor: None });
                }
            }
        }
    }
    rows
}

/// The live terminal and test backend deliberately share this renderer.
fn render(frame: &mut Frame<'_>, editor: &Editor) {
    let area = frame.area();
    if area.width == 0 || area.height == 0 { return; }
    frame.render_widget(Paragraph::new("Clanky editor"), Rect::new(0, 0, area.width, 1));
    let body_height = area.height.saturating_sub(2);
    if editor.help {
        let lines: Vec<_> = HELP.iter().map(|s| Line::from(*s)).collect();
        frame.render_widget(Paragraph::new(lines), Rect::new(0, 1, area.width, body_height));
    } else {
        let rows = display_rows(editor);
        for (y, row) in rows.iter().skip(editor.top).take(body_height as usize).enumerate() {
            let rect = Rect::new(0, y as u16 + 1, area.width, 1);
            frame.render_widget(Paragraph::new(row.line.clone()).scroll((0, editor.left.min(u16::MAX as usize) as u16)), rect);
            if let Some(column) = row.cursor {
                if column >= editor.left && column - editor.left < area.width as usize {
                    frame.set_cursor_position(((column - editor.left) as u16, y as u16 + 1));
                }
            }
        }
    }
    let status = if editor.confirm {
        "Discard unsaved edits and quit? y/N".to_owned()
    } else if let Some(query) = &editor.find {
        format!("Find: {query} | Enter next | Esc close | {}", editor.status)
    } else { editor.status.clone() };
    frame.render_widget(Paragraph::new(status), Rect::new(0, area.height - 1, area.width, 1));
}

fn capture(editor: &Editor, path: PathBuf) -> Result<(), String> {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).map_err(|e| e.to_string())?;
    terminal.draw(|frame| render(frame, editor)).map_err(|e| e.to_string())?;
    let buffer = terminal.backend().buffer();
    let mut output = String::new();
    for y in 0..24 {
        for x in 0..80 { output.push_str(buffer[(x, y)].symbol()); }
        output.push('\n');
    }
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true)
        .open(path).map_err(|e| format!("screen capture: {e}"))?;
    file.write_all(output.as_bytes()).map_err(|e| format!("screen capture: {e}"))
}

// Signal handlers only set a flag. Terminal IO and cleanup happen in the event
// loop, never in an async-signal-unsafe handler. SIGKILL cannot be recovered.
#[cfg(target_os = "linux")]
mod platform {
    use std::sync::atomic::{AtomicI32, Ordering};

    static SIGNAL: AtomicI32 = AtomicI32::new(0);
    const SIGNALS: [i32; 3] = [1, 2, 15]; // HUP, INT, TERM on Linux.
    unsafe extern "C" {
        fn signal(number: i32, handler: usize) -> usize;
    }
    extern "C" fn received(number: i32) {
        SIGNAL.store(number, Ordering::Relaxed);
    }
    pub(super) fn take_signal() -> i32 { SIGNAL.swap(0, Ordering::Relaxed) }

    pub(super) struct SignalGuard(Vec<(i32, usize)>);
    impl SignalGuard {
        pub(super) fn install() -> Result<Self, String> {
            SIGNAL.store(0, Ordering::Relaxed);
            let mut guard = Self(Vec::new());
            for number in SIGNALS {
                // Linux signal() returns the previous disposition or SIG_ERR.
                let previous = unsafe { signal(number, received as *const () as usize) };
                if previous == usize::MAX {
                    return Err(format!("install signal handler: {}", std::io::Error::last_os_error()));
                }
                guard.0.push((number, previous));
            }
            Ok(guard)
        }
    }
    impl Drop for SignalGuard {
        fn drop(&mut self) {
            for &(number, previous) in self.0.iter().rev() {
                unsafe { signal(number, previous); }
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    pub(super) struct SignalGuard;
    impl SignalGuard {
        pub(super) fn install() -> Result<Self, String> { Ok(Self) }
    }
    pub(super) fn take_signal() -> i32 { 0 }
}

