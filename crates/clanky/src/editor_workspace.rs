//! Window-local files. Only the terminal-workspace leaf contains server panes.
use clanky::editor::{DrawInfo, EditorPane, resolve_location};
use clanky_core::{TerminalSnapshot, layout::{Axis, Layout, Rect}};
use serde_json::json;
use std::{collections::BTreeMap, path::{Path, PathBuf}};
use winit::keyboard::{Key, NamedKey};

pub const TERMINALS: &str = "terminal-workspace";
struct Prompt {
    target: String,
    cwd: Option<PathBuf>,
    axis: Axis,
    import: bool,
    absolute_only: bool,
    text: String,
}
pub struct EditorWorkspace {
    pub layout: Layout,
    pub panes: BTreeMap<String, EditorPane>,
    pub focus: String,
    pub dragging: Option<String>,
    views: BTreeMap<String, (Rect, DrawInfo)>,
    prompt: Option<Prompt>,
    preedit: String,
    next: u64,
}
impl Default for EditorWorkspace {
    fn default() -> Self {
        Self { layout: Layout::leaf(TERMINALS), panes: BTreeMap::new(), focus: TERMINALS.into(), dragging: None, views: BTreeMap::new(), prompt: None, preedit: String::new(), next: 0 }
    }
}
impl EditorWorkspace {
    pub fn terminal_rect(&self, root: Rect) -> Rect {
        self.layout.rectangles(root).into_iter().find(|(id, _)| id == TERMINALS)
            .map(|(_, r)| r).unwrap_or(Rect { col: 0, row: 1, cols: 0, rows: 0 })
    }
    pub fn editor(&mut self) -> Option<&mut EditorPane> { self.panes.get_mut(&self.focus) }
    pub fn modal(&self) -> bool { self.prompt.is_some() }
    pub fn dirty(&self) -> bool { self.panes.values().any(EditorPane::dirty) }
    pub fn focus_on(&mut self, id: String) {
        if self.focus != id {
            if let Some(pane) = self.panes.get_mut(&id) { let _ = pane.command("refresh", &json!({})); }
            self.focus = id;
        }
    }
    pub fn prompt(&mut self, target: String, cwd: Option<PathBuf>, axis: Axis, import: bool, absolute_only: bool) -> Result<(), String> {
        if import && !self.panes.contains_key(&target) { return Err("Focus an editor before Load changes".into()); }
        self.prompt = Some(Prompt { target, cwd, axis, import, absolute_only, text: String::new() });
        self.preedit.clear();
        self.dragging = None;
        Ok(())
    }
    pub fn add(&mut self, target: &str, pane: EditorPane, axis: Axis, root: Rect) -> Result<(), String> {
        let id = format!("editor-{}", self.next);
        self.layout.split(target, id.clone(), axis, root)?;
        self.next += 1;
        self.panes.insert(id.clone(), pane);
        self.focus_on(id);
        Ok(())
    }
    pub fn close(&mut self) -> Result<(), String> {
        if let Some(pane) = self.panes.get(&self.focus) {
            if pane.dirty() { return Err("Unsaved file: save or undo before closing".into()); }
            self.layout.remove(&self.focus);
            self.panes.remove(&self.focus);
            self.focus = TERMINALS.into();
            self.dragging = None;
        }
        Ok(())
    }
    pub fn text(&mut self, text: &str) -> Result<(), String> {
        if let Some(prompt) = &mut self.prompt {
            if !text.chars().any(char::is_control) && prompt.text.len().saturating_add(text.len()) <= 8192 { prompt.text.push_str(text); }
        } else if let Some(pane) = self.editor() { pane.insert_text(text)?; }
        Ok(())
    }
    pub fn preedit(&mut self, text: &str) { self.preedit = text.chars().take(128).collect(); }
    pub fn prompt_key(&mut self, key: &Key, text: Option<&str>, composing: bool, root: Rect) -> Result<(), String> {
        match key {
            Key::Named(NamedKey::Escape) => { self.prompt = None; self.preedit.clear(); }
            Key::Named(NamedKey::Backspace) if !composing => { self.prompt.as_mut().unwrap().text.pop(); }
            Key::Named(NamedKey::Enter) if !composing => {
                let p = self.prompt.as_ref().unwrap();
                let (path, line, column) = if p.import { (PathBuf::from(&p.text), 1, 1) } else { parse_location(&p.text)? };
                if p.absolute_only && !path.is_absolute() { return Err("Remote terminal: enter an explicit local absolute path".into()); }
                let path = resolve_location(&clanky_edit::Location { path, line, column }, p.cwd.as_deref())?;
                if p.import {
                    let pane = self.panes.get_mut(&p.target).ok_or("Import target was closed")?;
                    pane.command("load_proposals", &json!({"path":path}))?;
                } else {
                    let pane = EditorPane::open(&path, line, column)?;
                    let target = p.target.clone();
                    let axis = p.axis;
                    self.add(&target, pane, axis, root)?;
                }
                self.prompt = None;
                self.preedit.clear();
            }
            _ if !composing => { if let Some(text) = text { self.text(text)?; } }
            _ => {}
        }
        Ok(())
    }
    pub fn scroll(&mut self, id: &str, row: usize, lines: i32, columns: i32) -> Result<(), String> {
        let review = self.views.get(id).is_some_and(|(rect, info)| row > rect.row.saturating_add(info.text_rows + 1));
        if let Some(pane) = self.panes.get_mut(id) {
            if review { pane.command("review_scroll", &json!({"lines":lines}))?; }
            else { pane.command("scroll", &json!({"lines":lines,"columns":columns}))?; }
        }
        Ok(())
    }
    pub fn draw(&mut self, frame: &mut TerminalSnapshot, root: Rect) {
        self.views.clear();
        for (id, rect) in self.layout.rectangles(root) {
            if let Some(pane) = self.panes.get_mut(&id) {
                let info = pane.draw(frame, &rect, self.focus == id && self.prompt.is_none());
                self.views.insert(id, (rect, info));
            }
        }
        if let Some(prompt) = &self.prompt {
            frame.images.clear();
            frame.cursor = None;
            let label = if prompt.import { "Load changes JSON path" } else { "Open local file path[:line[:col]]" };
            let title = format!("{label} — Enter confirms; Escape cancels — target {}", prompt.target);
            let tail: String = prompt.text.chars().rev().take(frame.cols.saturating_sub(1)).collect::<Vec<_>>().into_iter().rev().collect();
            for (row, text) in [title, format!("{tail}_"), self.preedit.clone()].iter().enumerate() {
                paint(frame, row, text);
            }
        }
    }
}
fn parse_location(text: &str) -> Result<(PathBuf, usize, usize), String> {
    if text.is_empty() { return Err("Enter a file path".into()); }
    let mut path = text;
    let mut suffix = Vec::new();
    for _ in 0..2 {
        if let Some((head, tail)) = path.rsplit_once(':') {
            if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) {
                let n = tail.parse::<usize>().map_err(|_| "Invalid line/column")?;
                if n == 0 { return Err("Line and column start at 1".into()); }
                suffix.push(n);
                path = head;
            } else { break; }
        }
    }
    if path.is_empty() { return Err("Enter a file path".into()); }
    let (line, column) = match suffix.as_slice() { [] => (1,1), [line] => (*line,1), [column,line] => (*line,*column), _ => unreachable!() };
    Ok((Path::new(path).to_path_buf(), line, column))
}
pub fn paint(frame: &mut TerminalSnapshot, row: usize, text: &str) {
    if row >= frame.rows { return; }
    let mut chars = text.chars();
    for col in 0..frame.cols {
        let cell = &mut frame.cells[row * frame.cols + col];
        cell.ch = chars.next().map(|c| if c.is_control() { ' ' } else { c }).unwrap_or(' ');
        cell.fg = [255,255,255]; cell.bg = [50,60,85];
    }
}
