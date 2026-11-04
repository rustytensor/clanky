//! Window-local, memory-only compose drafts and one-shot bracketed-paste sending.
mod spelling;
pub use spelling::{Misspelling, Spellchecker};
use crate::editor::EditorPane;
use clanky_core::{layout::Rect, Cell, TerminalSnapshot};
use clanky_render::Underline;
use serde_json::json;
use std::{collections::HashMap, path::{Path, PathBuf}};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const MAX_BYTES: usize = 256 * 1024;
const NORMAL: [u8; 3] = [24, 24, 28];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposeEvent { None, Send }

struct Draft {
    editor: EditorPane,
    flagged: Vec<Misspelling>,
}
struct Suggestions {
    word: Misspelling,
    words: Vec<String>,
    selected: usize,
}

pub struct Compose {
    drafts: HashMap<String, Draft>,
    target: Option<String>,
    checker: Option<Spellchecker>,
    spell_error: Option<String>,
    error: Option<String>,
    personal_enabled: bool,
    send_enter: bool,
    popup: Option<Suggestions>,
    preedit: String,
}

impl Compose {
    pub fn new(dir: &Path, language: &str, personal: Option<PathBuf>, send_enter: bool) -> Self {
        let personal_enabled = personal.is_some();
        let (checker, spell_error) = match Spellchecker::load(dir, language, personal) {
            Ok(checker) => (Some(checker), None),
            Err(error) => (None, Some(error)),
        };
        Self { drafts: HashMap::new(), target: None, checker, spell_error, error: None,
            personal_enabled, send_enter, popup: None, preedit: String::new() }
    }

    pub fn open(&mut self, terminal: &str) -> Result<(), String> {
        if terminal.is_empty() { return Err("no compose target".into()); }
        self.drafts.entry(terminal.to_owned()).or_insert_with(|| Draft {
            editor: EditorPane::from_text(""), flagged: Vec::new(),
        });
        self.target = Some(terminal.to_owned());
        self.popup = None;
        self.preedit.clear();
        self.error = None;
        Ok(())
    }
    pub fn close(&mut self) { self.target = None; self.popup = None; self.preedit.clear(); }
    pub fn active_target(&self) -> Option<&str> { self.target.as_deref() }
    pub fn has_drafts(&self) -> bool { self.drafts.values().any(|d| !d.editor.text().is_empty()) }
    pub fn draft(&self, terminal: &str) -> Option<&EditorPane> { self.drafts.get(terminal).map(|d| &d.editor) }
    pub fn spell_error(&self) -> Option<&str> { self.spell_error.as_deref() }
    fn active(&self) -> Result<&Draft, String> {
        self.target.as_ref().and_then(|t| self.drafts.get(t)).ok_or_else(|| "compose is closed".into())
    }
    fn active_mut(&mut self) -> Result<&mut Draft, String> {
        self.target.as_ref().and_then(|t| self.drafts.get_mut(t)).ok_or_else(|| "compose is closed".into())
    }
    fn remember<T>(&mut self, result: Result<T, String>) -> Result<T, String> {
        self.error = result.as_ref().err().cloned();
        result
    }
    fn check_insert(&self, text: &str) -> Result<(), String> {
        let editor = &self.active()?.editor;
        if editor.find_active() { return Ok(()); }
        let s = editor.selection();
        let removed = s.anchor.abs_diff(s.cursor);
        if editor.text().len().saturating_sub(removed).saturating_add(text.len()) > MAX_BYTES {
            return Err("compose draft exceeds 256 KiB".into());
        }
        Ok(())
    }
    fn recheck(&mut self) {
        if let Some(draft) = self.target.as_ref().and_then(|t| self.drafts.get_mut(t)) {
            draft.flagged = self.checker.as_ref().map_or_else(Vec::new, |c| c.flagged(draft.editor.text()));
        }
        self.popup = None;
    }
    pub fn insert_text(&mut self, text: &str) -> Result<(), String> {
        let result = (|| {
            self.check_insert(text)?;
            let before = self.active()?.editor.text().to_owned();
            self.active_mut()?.editor.insert_text(text)?;
            if self.active()?.editor.text() != before { self.recheck(); }
            Ok(())
        })();
        self.remember(result)
    }

    pub fn key(&mut self, key: &Key, text: Option<&str>, modifiers: ModifiersState, composing: bool) -> Result<ComposeEvent, String> {
        let result = self.handle_key(key, text, modifiers, composing);
        self.remember(result)
    }
    fn handle_key(&mut self, key: &Key, text: Option<&str>, modifiers: ModifiersState, composing: bool) -> Result<ComposeEvent, String> {
        self.active()?;
        if composing { return Ok(ComposeEvent::None); }
        if *key == Key::Named(NamedKey::Escape) {
            if self.popup.take().is_some() { return Ok(ComposeEvent::None); }
            if !self.preedit.is_empty() { self.preedit.clear(); return Ok(ComposeEvent::None); }
            if self.active()?.editor.find_active() {
                self.active_mut()?.editor.key(key, text, modifiers, false)?;
            } else { self.close(); }
            return Ok(ComposeEvent::None);
        }
        if *key == Key::Named(NamedKey::Enter) && modifiers.control_key() && !modifiers.shift_key() {
            return Ok(ComposeEvent::Send);
        }
        if let Some(popup) = &mut self.popup {
            match key {
                Key::Named(NamedKey::ArrowUp) => {
                    popup.selected = if popup.selected == 0 { popup.words.len() - 1 } else { popup.selected - 1 };
                    return Ok(ComposeEvent::None);
                }
                Key::Named(NamedKey::ArrowDown) => {
                    popup.selected = (popup.selected + 1) % popup.words.len();
                    return Ok(ComposeEvent::None);
                }
                Key::Named(NamedKey::Enter) => {
                    let popup = self.popup.take().unwrap();
                    let replacement = &popup.words[popup.selected];
                    let editor = &self.active()?.editor;
                    if editor.text().len() - popup.word.range.len() + replacement.len() > MAX_BYTES {
                        self.popup = Some(popup);
                        return Err("compose draft exceeds 256 KiB".into());
                    }
                    let editor = &mut self.active_mut()?.editor;
                    editor.command("select", &json!({"anchor": popup.word.range.start, "cursor": popup.word.range.end}))?;
                    editor.command("insert", &json!({"text": replacement}))?;
                    self.recheck();
                    return Ok(ComposeEvent::None);
                }
                _ => { self.popup = None; }
            }
        }
        if *key == Key::Named(NamedKey::F7) {
            if self.suggestions()?.is_empty() { return Err("no suggestions".into()); }
            return Ok(ComposeEvent::None);
        }
        if modifiers.control_key() {
            if let Key::Character(ch) = key {
                if ch.eq_ignore_ascii_case("s") { return Ok(ComposeEvent::None); }
                if ch.eq_ignore_ascii_case("a") && modifiers.alt_key() {
                    self.add_current_word()?;
                    return Ok(ComposeEvent::None);
                }
            }
        }
        // All edits enter through this controller, so every undo/redo state is
        // already bounded. Preflight insertions before the editor sees them.
        if !modifiers.control_key() && !modifiers.alt_key() && !modifiers.super_key() {
            match key {
                Key::Named(NamedKey::Enter) => self.check_insert("\n")?,
                Key::Named(NamedKey::Tab) => self.check_insert("\t")?,
                Key::Named(NamedKey::Space) => self.check_insert(" ")?,
                Key::Character(_) => if let Some(text) = text { self.check_insert(text)?; },
                _ => {},
            }
        }
        let before = self.active()?.editor.text().to_owned();
        self.active_mut()?.editor.key(key, text, modifiers, false)?;
        if self.active()?.editor.text() != before { self.recheck(); }
        Ok(ComposeEvent::None)
    }

    pub fn send_with(&mut self, send: impl FnOnce(&str, &[u8]) -> Result<(), String>) -> Result<(), String> {
        let result = (|| {
            let target = self.target.as_deref().ok_or("compose is closed")?;
            let text = self.active()?.editor.text();
            if text.is_empty() { return Err("compose draft is empty".into()); }
            if text.len() > MAX_BYTES { return Err("compose draft exceeds 256 KiB".into()); }
            if text.chars().any(|c| c.is_control() && !matches!(c, '\t' | '\r' | '\n')) {
                return Err("compose draft contains forbidden control characters".into());
            }
            let mut payload = Vec::with_capacity(text.len() + 13);
            payload.extend_from_slice(b"\x1b[200~");
            payload.extend_from_slice(text.as_bytes());
            payload.extend_from_slice(b"\x1b[201~");
            if self.send_enter { payload.push(b'\r'); }
            send(target, &payload)
        })();
        if result.is_ok() { self.close(); }
        self.remember(result)
    }
    pub fn misspellings(&self) -> &[Misspelling] {
        self.active().map_or(&[], |d| d.flagged.as_slice())
    }
    fn current_word(&self) -> Result<Misspelling, String> {
        let draft = self.active()?;
        let caret = draft.editor.selection().cursor;
        draft.flagged.iter().find(|w| w.range.contains(&caret))
            .or_else(|| draft.flagged.iter().find(|w| w.range.start >= caret))
            .or_else(|| draft.flagged.first()).cloned().ok_or_else(|| "no misspelling".into())
    }
    pub fn suggestions(&mut self) -> Result<Vec<String>, String> {
        let result = (|| {
            let checker = self.checker.as_ref().ok_or_else(|| self.spell_error.clone().unwrap_or_else(|| "spellchecker unavailable".into()))?;
            let word = self.current_word()?;
            let words = checker.suggest(&word.word);
            self.popup = if words.is_empty() { None } else { Some(Suggestions { word, words: words.clone(), selected: 0 }) };
            if words.is_empty() { self.error = Some("no suggestions".into()); }
            Ok(words)
        })();
        if result.is_err() { self.error = result.as_ref().err().cloned(); }
        result
    }
    pub fn add_current_word(&mut self) -> Result<(), String> {
        let result = (|| {
            let word = self.current_word()?;
            let checker = self.checker.as_mut().ok_or("spellchecker unavailable")?;
            checker.add_word(&word.word)?;
            for draft in self.drafts.values_mut() {
                draft.flagged = checker.flagged(draft.editor.text());
            }
            self.popup = None;
            Ok(())
        })();
        self.remember(result)
    }

    fn editor_rect(&self, rect: &Rect) -> Rect {
        let popup_rows = self.popup.as_ref().map_or(0, |p| p.words.len().min(5));
        Rect { col: rect.col, row: rect.row, cols: rect.cols, rows: rect.rows.saturating_sub(2 + popup_rows) }
    }
    pub fn click(&mut self, rect: &Rect, col: usize, row: usize, extend: bool) -> Result<(), String> {
        let rect = self.editor_rect(rect);
        let result = self.active_mut().and_then(|d| d.editor.click(&rect, col, row, extend));
        self.remember(result)
    }
    pub fn drag(&mut self, rect: &Rect, col: usize, row: usize) -> Result<(), String> {
        let rect = self.editor_rect(rect);
        let result = self.active_mut().and_then(|d| d.editor.drag(&rect, col, row));
        self.remember(result)
    }
    pub fn scroll(&mut self, lines: i64, columns: i64) -> Result<(), String> {
        let result = self.active_mut().and_then(|d| d.editor.command("scroll", &json!({"lines": lines, "columns": columns})).map(|_| ()));
        self.remember(result)
    }
    pub fn preedit(&mut self, text: &str) {
        self.preedit = text.chars().scan(0usize, |bytes, ch| {
            *bytes += ch.len_utf8();
            (*bytes <= 1024).then_some(ch)
        }).collect();
    }

    pub fn draw(&mut self, frame: &mut TerminalSnapshot, rect: &Rect) -> Vec<Underline> {
        if self.target.is_none() { return Vec::new(); }
        let editor_rect = self.editor_rect(rect);
        let target_label = self.target.clone().unwrap_or_default();
        frame.cursor = None;
        let mut underlines: Vec<Underline> = Vec::new();
        if let Ok(draft) = self.active_mut() {
            draft.editor.draw(frame, &editor_rect, true);
            if editor_rect.rows > 0 && !draft.editor.find_active() {
                paint(frame, &editor_rect, editor_rect.row.saturating_add(editor_rect.rows - 1),
                    "Memory draft | Ctrl+F find | Ctrl+Z undo | Ctrl+Shift+Z redo");
            }
            for cell in draft.editor.visible_byte_cells(&editor_rect) {
                if cell.row >= frame.rows || cell.col >= frame.cols { continue; }
                if !draft.flagged.iter().any(|w| w.range.contains(&cell.byte)) { continue; }
                if let Some(last) = underlines.last_mut() {
                    if last.row == cell.row && last.end_col == cell.col { last.end_col += 1; continue; }
                }
                underlines.push(Underline { row: cell.row, start_col: cell.col, end_col: cell.col + 1, color: [255, 96, 96] });
            }
        }
        if editor_rect.rows > 0 {
            paint(frame, &editor_rect, editor_rect.row, &format!("Compose → {target_label} | memory-only draft"));
        }
        let first = rect.row.saturating_add(editor_rect.rows);
        if let Some(popup) = &self.popup {
            let start = popup.selected.saturating_sub(4);
            for (line, (index, word)) in popup.words.iter().enumerate().skip(start).take(5).enumerate() {
                paint(frame, rect, first.saturating_add(line), &format!("{} {}", if index == popup.selected { ">" } else { " " }, word));
            }
        }
        if rect.rows >= 2 {
            paint(frame, rect, rect.row.saturating_add(rect.rows - 2), "Ctrl+Enter send | Esc close | F7 suggestions | Up/Down, Enter replace | Ctrl+Alt+A add");
        }
        if rect.rows > 0 {
            let mut status = Vec::new();
            if let Some(error) = &self.spell_error { status.push(error.clone()); }
            if !self.personal_enabled { status.push("Add disabled: no personal word-list path".into()); }
            if let Some(error) = &self.error { status.push(error.clone()); }
            if !self.preedit.is_empty() { status.push(format!("IME: {}", self.preedit)); }
            paint(frame, rect, rect.row.saturating_add(rect.rows - 1), &status.join(" | "));
        }
        underlines
    }
}

fn paint(frame: &mut TerminalSnapshot, rect: &Rect, row: usize, text: &str) {
    if row < rect.row || row >= rect.row.saturating_add(rect.rows) || row >= frame.rows { return; }
    let mut chars = text.chars();
    for col in rect.col..rect.col.saturating_add(rect.cols).min(frame.cols) {
        let ch = chars.next().map_or(' ', |ch| if ch.is_control() { ' ' } else { ch });
        if let Some(cell) = row.checked_mul(frame.cols).and_then(|i| i.checked_add(col)).and_then(|i| frame.cells.get_mut(i)) {
            *cell = Cell { ch, fg: [220, 220, 225], bg: NORMAL };
        }
    }
}
