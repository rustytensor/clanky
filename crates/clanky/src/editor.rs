//! Window-local editor controller, shared by interactive and offscreen clients.
use clanky_core::{layout::Rect, Cell, TerminalSnapshot};
use clanky_edit::{Document, HunkState, Location, Selection};
use serde_json::{json, Value};
use std::{collections::HashMap, path::{Path, PathBuf}};
use winit::keyboard::{Key, ModifiersState, NamedKey};

type Result<T> = std::result::Result<T, String>;
const NORMAL: [u8; 3] = [24, 24, 28];
const SELECTED: [u8; 3] = [40, 70, 120];
const OLD: [u8; 3] = [80, 24, 24];
const NEW: [u8; 3] = [24, 80, 40];

#[derive(Default, Debug)]
pub struct DrawInfo {
    pub source_lines: usize,
    pub text_rows: usize,
    pub old_row: Option<usize>,
    pub new_row: Option<usize>,
}

pub fn location_at(snapshot: &TerminalSnapshot, row: usize, col: usize) -> Option<Location> {
    if row >= snapshot.rows || col >= snapshot.cols { return None; }
    let start = row.checked_mul(snapshot.cols)?;
    let cells = snapshot.cells.get(start..start.checked_add(snapshot.cols)?)?;
    if cells.get(col)?.ch.is_whitespace() { return None; }
    let left = (0..col).rev().find(|&i| cells[i].ch.is_whitespace()).map_or(0, |i| i + 1);
    let right = (col..cells.len()).find(|&i| cells[i].ch.is_whitespace()).unwrap_or(cells.len());
    let token: String = cells[left..right].iter().map(|c| c.ch).collect();
    let mut token = token.trim_end_matches([',', ';']);
    loop {
        let paired = [('(', ')'), ('[', ']'), ('{', '}')].into_iter()
            .find(|(open, close)| token.starts_with(*open) && token.ends_with(*close));
        if paired.is_none() || token.len() < 2 { break; }
        token = &token[1..token.len() - 1];
    }
    Location::parse(token.trim_end_matches([',', ';', ')', ']', '}'])).ok()
}

pub fn resolve_location(location: &Location, cwd: Option<&Path>) -> Result<PathBuf> {
    if location.path.is_absolute() { return Ok(location.path.clone()); }
    cwd.map(|cwd| cwd.join(&location.path)).ok_or_else(|| "unknown directory".into())
}

struct Preview {
    old: String,
    new: String,
    old_rows: Vec<(usize, usize)>,
    new_rows: Vec<(usize, usize)>,
    width: usize,
}

/// A visible source cell at absolute frame coordinates and a UTF-8 byte start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteCell {
    pub byte: usize,
    pub row: usize,
    pub col: usize,
}

pub struct EditorPane {
    document: Document,
    path: PathBuf,
    saved: String,
    dirty: bool,
    lines: Vec<usize>,
    top: usize,
    left: usize,
    page: usize,
    columns: usize,
    newline: &'static str,
    review: usize,
    review_top: usize,
    previews: HashMap<u64, Preview>,
    find: Option<String>,
    query: String,
    error: Option<String>,
    was_focused: bool,
    dragging: bool,
}

fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0).chain(text.bytes().enumerate().filter_map(|(i, b)| (b == b'\n').then_some(i + 1))).collect()
}
fn line_end(text: &str, lines: &[usize], index: usize) -> usize {
    match lines.get(index + 1) {
        Some(&next) => { let end = next - 1; if end > lines[index] && text.as_bytes()[end - 1] == b'\r' { end - 1 } else { end } }
        None => text.len(),
    }
}
fn field<'a>(args: &'a Value, name: &str) -> Result<&'a str> {
    args.get(name).and_then(Value::as_str).ok_or_else(|| format!("invalid {name}"))
}
fn number(args: &Value, name: &str) -> Result<usize> {
    args.get(name).and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok()).ok_or_else(|| format!("invalid {name}"))
}
fn signed(args: &Value, name: &str) -> Result<i64> {
    args.get(name).and_then(Value::as_i64).ok_or_else(|| format!("invalid {name}"))
}
fn shift(value: usize, delta: i64) -> usize {
    if delta < 0 { value.saturating_sub(usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX)) }
    else { value.saturating_add(usize::try_from(delta).unwrap_or(usize::MAX)) }
}

impl EditorPane {
    pub fn open(path: &Path, line: usize, column: usize) -> Result<Self> {
        let mut document = Document::open(path).map_err(|e| e.to_string())?;
        let cursor = document.offset(line, column).map_err(|e| e.to_string())?;
        document.select(Selection { anchor: cursor, cursor }).map_err(|e| e.to_string())?;
        Ok(Self::with_document(document, path.to_owned(), line.saturating_sub(1)))
    }
    /// Create an editor with no persistent path; [compose] is only a display label.
    pub fn from_text(text: &str) -> Self {
        Self::with_document(Document::from_text(text), PathBuf::from("[compose]"), 0)
    }
    fn with_document(document: Document, path: PathBuf, top: usize) -> Self {
        let lines = line_starts(document.text());
        let newline = document.text().find('\n').map_or("\n", |i| if i > 0 && document.text().as_bytes()[i - 1] == b'\r' { "\r\n" } else { "\n" });
        let saved = document.text().to_owned();
        Self { document, path, saved, dirty: false, lines,
            top, left: 0, page: 16, columns: 80, newline, review: 0, review_top: 0,
            previews: HashMap::new(), find: None, query: String::new(), error: None,
            was_focused: false, dragging: false }
    }
    pub fn from_click(snapshot: &TerminalSnapshot, row: usize, col: usize, cwd: Option<&Path>) -> Result<Self> {
        let location = location_at(snapshot, row, col).ok_or("no source location")?;
        Self::open(&resolve_location(&location, cwd)?, location.line, location.column)
    }
    pub fn text(&self) -> &str { self.document.text() }
    pub fn path(&self) -> &Path { &self.path }
    pub fn dirty(&self) -> bool { self.dirty }
    pub fn pending_count(&self) -> usize {
        self.document.hunks().iter().filter(|h| h.state == HunkState::Pending).count()
    }
    pub fn selection(&self) -> Selection { self.document.selection() }
    pub fn find_active(&self) -> bool { self.find.is_some() }

    fn changed(&mut self) {
        self.lines = line_starts(self.text());
        self.dirty = self.text() != self.saved;
        self.reveal();
    }
    fn cursor_line(&self) -> usize {
        self.lines.partition_point(|&start| start <= self.selection().cursor).saturating_sub(1)
    }
    fn reveal(&mut self) {
        let line = self.cursor_line();
        if line < self.top { self.top = line; }
        if line >= self.top.saturating_add(self.page.max(1)) { self.top = line + 1 - self.page.max(1); }
        let end = line_end(self.text(), &self.lines, line);
        let visual = self.text()[self.lines[line]..self.selection().cursor.min(end)].chars()
            .fold(0usize, |v, c| v + if c == '\t' { 4 - v % 4 } else { 1 });
        if visual < self.left { self.left = visual; }
        if visual >= self.left.saturating_add(self.columns.max(1)) { self.left = visual + 1 - self.columns.max(1); }
    }
    fn boundary(&self, offset: usize, right: bool) -> usize {
        let text = self.text();
        if right {
            if text.get(offset..).is_some_and(|s| s.starts_with("\r\n")) { return offset + 2; }
            text.get(offset..).and_then(|s| s.chars().next()).map_or(offset, |c| offset + c.len_utf8())
        } else {
            if text.get(..offset).is_some_and(|s| s.ends_with("\r\n")) { return offset - 2; }
            text.get(..offset).and_then(|s| s.chars().next_back()).map_or(offset, |c| offset - c.len_utf8())
        }
    }
    fn move_cursor(&mut self, kind: &str, extend: bool) -> Result<()> {
        let selection = self.selection();
        let cursor = selection.cursor;
        let line = self.cursor_line();
        let end = line_end(self.text(), &self.lines, line);
        let column = self.text()[self.lines[line]..cursor.min(end)].chars().count();
        let target = match kind {
            "left" if !extend && selection.anchor != cursor => selection.anchor.min(cursor),
            "right" if !extend && selection.anchor != cursor => selection.anchor.max(cursor),
            "left" => self.boundary(cursor, false),
            "right" => self.boundary(cursor, true),
            "home" => self.lines[line],
            "end" => end,
            "up" | "down" | "pageup" | "pagedown" => {
                let distance = if kind.starts_with("page") { self.page.max(1) } else { 1 };
                let row = if kind == "up" || kind == "pageup" { line.saturating_sub(distance) }
                    else { line.saturating_add(distance).min(self.lines.len() - 1) };
                let start = self.lines[row];
                let content = &self.text()[start..line_end(self.text(), &self.lines, row)];
                start + content.char_indices().map(|(i, _)| i).nth(column).unwrap_or(content.len())
            }
            _ => return Err("invalid move kind".into()),
        };
        self.document.select(Selection { anchor: if extend { selection.anchor } else { target }, cursor: target }).map_err(|e| e.to_string())?;
        self.reveal();
        Ok(())
    }
    fn replace_selection(&mut self, text: &str) -> Result<()> {
        let selection = self.selection();
        self.document.edit(selection.anchor.min(selection.cursor)..selection.anchor.max(selection.cursor), text).map_err(|e| e.to_string())?;
        self.changed();
        Ok(())
    }
    fn remove(&mut self, forward: bool) -> Result<()> {
        let s = self.selection();
        let (start, end) = if s.anchor != s.cursor { (s.anchor.min(s.cursor), s.anchor.max(s.cursor)) }
            else if forward { (s.cursor, self.boundary(s.cursor, true)) }
            else { (self.boundary(s.cursor, false), s.cursor) };
        self.document.edit(start..end, "").map_err(|e| e.to_string())?;
        self.changed();
        Ok(())
    }
    fn refresh(&mut self) -> Result<()> {
        if self.document.refresh().map_err(|e| e.to_string())? { Ok(()) }
        else { Err("file changed on disk; reopen before saving or accepting".into()) }
    }
    fn propose(&mut self, start: usize, end: usize, replacement: &str) -> Result<u64> {
        let old = self.text().get(start..end).ok_or("range is not ordered UTF-8 boundaries")?.to_owned();
        let id = self.document.propose(start..end, replacement).map_err(|e| e.to_string())?;
        self.previews.insert(id, Preview { old_rows: preview_rows(&old, self.columns), new_rows: preview_rows(replacement, self.columns), width: self.columns, old, new: replacement.to_owned() });
        self.review = self.document.hunks().len() - 1;
        self.review_top = 0;
        Ok(id)
    }
    fn find_next(&mut self) -> Result<()> {
        if self.query.is_empty() { return Err("empty find query".into()); }
        let start = self.selection().anchor.max(self.selection().cursor);
        let found = self.text()[start..].find(&self.query).map(|i| i + start)
            .or_else(|| self.text().find(&self.query));
        let start = found.ok_or("no match")?;
        self.document.select(Selection { anchor: start, cursor: start + self.query.len() }).map_err(|e| e.to_string())?;
        self.reveal();
        Ok(())
    }
    fn load_proposals(&mut self, path: &str) -> Result<()> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        check_fields(&value, &["base", "edits"])?;
        if field(&value, "base")? != self.text() { return Err("proposal base does not match current text".into()); }
        let edits = value.get("edits").and_then(Value::as_array).ok_or("invalid edits")?;
        let mut checked = Vec::with_capacity(edits.len());
        for edit in edits {
            check_fields(edit, &["start", "end", "text"])?;
            let start = number(edit, "start")?;
            let end = number(edit, "end")?;
            let text = field(edit, "text")?;
            if self.text().get(start..end).is_none() { return Err("range is not ordered UTF-8 boundaries".into()); }
            checked.push((start, end, text));
        }
        self.refresh()?;
        for (start, end, text) in checked { self.propose(start, end, text)?; }
        Ok(())
    }

    pub fn command(&mut self, action: &str, args: &Value) -> Result<Value> {
        let result = self.perform(action, args);
        self.error = result.as_ref().err().cloned();
        result
    }
    fn perform(&mut self, action: &str, args: &Value) -> Result<Value> {
        let allowed: &[&str] = match action {
            "insert" => &["text"], "select" => &["anchor", "cursor"], "move" => &["kind", "extend"],
            "find" => &["query"], "propose" => &["start", "end", "text"], "accept" | "reject" => &["id"],
            "scroll" => &["lines", "columns"], "review_scroll" => &["lines"], "load_proposals" => &["path"],
            "backspace" | "delete" | "undo" | "redo" | "save" | "find_next" | "accept_all" | "reject_all" |
            "review_next" | "review_previous" | "refresh" => &[],
            _ => return Err(format!("unknown editor action: {action}")),
        };
        check_fields(args, allowed)?;
        match action {
            "insert" => self.replace_selection(field(args, "text")?)?,
            "select" => {
                let selection = Selection { anchor: number(args, "anchor")?, cursor: number(args, "cursor")? };
                self.document.select(selection).map_err(|e| e.to_string())?;
                self.reveal();
            }
            "move" => {
                let extend = match args.get("extend") { None => false, Some(v) => v.as_bool().ok_or("invalid extend")? };
                self.move_cursor(field(args, "kind")?, extend)?;
            }
            "backspace" => self.remove(false)?, "delete" => self.remove(true)?,
            "undo" => { self.document.undo().map_err(|e| e.to_string())?; self.changed(); }
            "redo" => { self.document.redo().map_err(|e| e.to_string())?; self.changed(); }
            "save" => { self.document.save().map_err(|e| e.to_string())?; self.saved = self.text().to_owned(); self.dirty = false; }
            "find" => { self.query = field(args, "query")?.to_owned(); self.find_next()?; }
            "find_next" => self.find_next()?,
            "propose" => return self.propose(number(args, "start")?, number(args, "end")?, field(args, "text")?).map(|id| json!({"id": id})),
            "accept" | "reject" => {
                let id = args.get("id").and_then(Value::as_u64).ok_or("invalid id")?;
                self.refresh()?;
                if action == "accept" { self.document.accept(id).map_err(|e| e.to_string())?; self.changed(); }
                else { self.document.reject(id).map_err(|e| e.to_string())?; }
            }
            "accept_all" | "reject_all" => {
                self.refresh()?;
                let ids: Vec<_> = self.document.hunks().iter().filter(|h| h.state == HunkState::Pending).map(|h| h.id).collect();
                let decisions: Result<()> = (|| {
                    for id in ids {
                        // An accepted overlapping hunk can invalidate a later one.
                        if !self.document.hunks().iter().any(|h| h.id == id && h.state == HunkState::Pending) { continue; }
                        if action == "accept_all" { self.document.accept(id).map_err(|e| e.to_string())?; }
                        else { self.document.reject(id).map_err(|e| e.to_string())?; }
                    }
                    Ok(())
                })();
                self.changed();
                decisions?;
            }
            "review_next" | "review_previous" => {
                let refreshed = self.refresh();
                let count = self.document.hunks().len();
                if count > 0 { self.review = if action == "review_next" { (self.review + 1) % count } else { (self.review + count - 1) % count }; }
                self.review_top = 0;
                refreshed?;
            }
            "scroll" => {
                let lines = signed(args, "lines")?;
                let columns = if args.get("columns").is_some() { signed(args, "columns")? } else { 0 };
                self.top = shift(self.top, lines).min(self.lines.len() - 1);
                self.left = shift(self.left, columns);
            }
            "review_scroll" => {
                let delta = signed(args, "lines")?;
                let limit = self.document.hunks().get(self.review).and_then(|h| self.previews.get(&h.id))
                    .map_or(0, |p| p.old_rows.len().max(p.new_rows.len()).saturating_sub(1));
                self.review_top = shift(self.review_top, delta).min(limit);
                self.refresh()?;
            }
            "refresh" => self.refresh()?,
            "load_proposals" => self.load_proposals(field(args, "path")?)?,
            _ => return Err("unknown editor action".into()),
        }
        Ok(json!({}))
    }

    pub fn insert_text(&mut self, text: &str) -> Result<()> {
        if let Some(prompt) = &mut self.find { prompt.push_str(text); Ok(()) }
        else { self.command("insert", &json!({"text": text})).map(|_| ()) }
    }
    pub fn key(&mut self, key: &Key, text: Option<&str>, modifiers: ModifiersState, composing: bool) -> Result<()> {
        if composing { return Ok(()); }
        let ctrl = modifiers.control_key();
        let extend = modifiers.shift_key();
        if let Some(prompt) = &mut self.find {
            match key {
                Key::Named(NamedKey::Escape) => { self.find = None; }
                Key::Named(NamedKey::Backspace) => { prompt.pop(); }
                Key::Named(NamedKey::Enter) => {
                    self.query = prompt.clone();
                    self.command("find_next", &json!({}))?;
                }
                _ if !ctrl && !modifiers.alt_key() && !modifiers.super_key() => { if let Some(text) = text { self.insert_text(text)?; } }
                _ => {}
            }
            return Ok(());
        }
        if ctrl {
            if let Key::Character(ch) = key {
                let lower = ch.to_lowercase();
                let action = match lower.as_str() {
                    "s" if !modifiers.alt_key() => Some("save"),
                    "z" if !modifiers.alt_key() => Some(if extend { "redo" } else { "undo" }),
                    "f" if !modifiers.alt_key() => { self.find = Some(self.query.clone()); None }
                    "a" | "r" if modifiers.alt_key() => {
                        let accept = lower == "a";
                        if extend { Some(if accept { "accept_all" } else { "reject_all" }) }
                        else {
                            if let Some(h) = self.document.hunks().get(self.review) {
                                let id = h.id;
                                self.command(if accept { "accept" } else { "reject" }, &json!({"id": id}))?;
                            }
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(action) = action { self.command(action, &json!({}))?; }
            }
            return Ok(());
        }
        if modifiers.alt_key() || modifiers.super_key() { return Ok(()); }
        let movement = match key {
            Key::Named(NamedKey::ArrowLeft) => Some("left"), Key::Named(NamedKey::ArrowRight) => Some("right"),
            Key::Named(NamedKey::ArrowUp) => Some("up"), Key::Named(NamedKey::ArrowDown) => Some("down"),
            Key::Named(NamedKey::Home) => Some("home"), Key::Named(NamedKey::End) => Some("end"),
            Key::Named(NamedKey::PageUp) => Some("pageup"), Key::Named(NamedKey::PageDown) => Some("pagedown"),
            _ => None,
        };
        if let Some(kind) = movement { return self.command("move", &json!({"kind": kind, "extend": extend})).map(|_| ()); }
        match key {
            Key::Named(NamedKey::Backspace) => { self.command("backspace", &json!({}))?; }
            Key::Named(NamedKey::Delete) => { self.command("delete", &json!({}))?; }
            Key::Named(NamedKey::Tab) => self.insert_text("\t")?,
            Key::Named(NamedKey::Space) => self.insert_text(" ")?,
            Key::Named(NamedKey::Enter) => self.insert_text(self.newline)?,
            Key::Character(_) => { if let Some(text) = text { self.insert_text(text)?; } }
            _ => {}
        }
        Ok(())
    }

    fn geometry(&self, rect: &Rect) -> (usize, usize) {
        let review_rows = if self.document.hunks().is_empty() { 0 } else { rect.rows.saturating_sub(3).min(8) };
        (rect.rows.saturating_sub(2 + review_rows), review_rows)
    }
    fn mouse_offset(&self, rect: &Rect, col: usize, row: usize, text_rows: usize) -> Option<usize> {
        if col < rect.col || col >= rect.col.saturating_add(rect.cols) || row <= rect.row || row > rect.row.saturating_add(text_rows) { return None; }
        let line = self.top.saturating_add(row - rect.row - 1).min(self.lines.len() - 1);
        let start = self.lines[line];
        let end = line_end(self.text(), &self.lines, line);
        let target = self.left.saturating_add(col - rect.col);
        let mut visual = 0usize;
        for (i, ch) in self.text()[start..end].char_indices() {
            let width = if ch == '\t' { 4 - visual % 4 } else { 1 };
            if visual.saturating_add(width) > target { return Some(start + i); }
            visual += width;
        }
        Some(end)
    }
    pub fn click(&mut self, rect: &Rect, col: usize, row: usize, extend: bool) -> Result<()> {
        self.dragging = false;
        let (text_rows, review_rows) = self.geometry(rect);
        if let Some(cursor) = self.mouse_offset(rect, col, row, text_rows) {
            let anchor = if extend { self.selection().anchor } else { cursor };
            self.command("select", &json!({"anchor": anchor, "cursor": cursor}))?;
            self.dragging = true;
            return Ok(());
        }
        if review_rows >= 2 && row == rect.row.saturating_add(2 + text_rows) && col >= rect.col && col < rect.col.saturating_add(rect.cols) {
            let position = col - rect.col;
            if let Some((_, _, action)) = review_buttons().into_iter().find(|(start, label, _)| position >= *start && position < start + label.len()) {
                let args = if action == "accept" || action == "reject" {
                    match self.document.hunks().get(self.review) { Some(h) => json!({"id": h.id}), None => return Ok(()) }
                } else { json!({}) };
                self.command(action, &args)?;
            }
        }
        Ok(())
    }
    pub fn drag(&mut self, rect: &Rect, col: usize, row: usize) -> Result<()> {
        if !self.dragging { return Ok(()); }
        let (text_rows, _) = self.geometry(rect);
        if let Some(cursor) = self.mouse_offset(rect, col, row, text_rows) {
            let anchor = self.selection().anchor;
            self.command("select", &json!({"anchor": anchor, "cursor": cursor}))?;
        }
        Ok(())
    }

    /// Enumerate source cells only, excluding title, status and review rows.
    /// Cached line starts restrict traversal to the current viewport.
    pub fn visible_byte_cells(&self, rect: &Rect) -> Vec<ByteCell> {
        let (text_rows, _) = self.geometry(rect);
        let mut cells = Vec::new();
        if rect.cols == 0 { return cells; }
        for visible in 0..text_rows {
            let index = self.top.saturating_add(visible);
            let Some(&start) = self.lines.get(index) else { break; };
            let end = line_end(self.text(), &self.lines, index);
            let row = rect.row.saturating_add(1 + visible);
            let mut visual = 0usize;
            for (offset, ch) in self.text()[start..end].char_indices() {
                let width = if ch == '\t' { 4 - visual % 4 } else { 1 };
                for step in 0..width {
                    let column = visual.saturating_add(step);
                    if column >= self.left && column - self.left < rect.cols {
                        cells.push(ByteCell { byte: start + offset, row,
                            col: rect.col.saturating_add(column - self.left) });
                    }
                }
                visual = visual.saturating_add(width);
                if visual >= self.left.saturating_add(rect.cols) { break; }
            }
        }
        cells
    }

    pub fn draw(&mut self, frame: &mut TerminalSnapshot, rect: &Rect, focused: bool) -> DrawInfo {
        if focused && !self.was_focused {
            if let Err(error) = self.refresh() { self.error = Some(error); }
        }
        self.was_focused = focused;
        let (text_rows, review_rows) = self.geometry(rect);
        self.page = text_rows.max(1);
        self.columns = rect.cols.max(1);
        let mut info = DrawInfo { text_rows, ..DrawInfo::default() };
        for row in rect.row..rect.row.saturating_add(rect.rows).min(frame.rows) {
            for col in rect.col..rect.col.saturating_add(rect.cols).min(frame.cols) {
                put(frame, row, col, ' ', NORMAL);
            }
        }
        let title = format!("{}{}  column scroll {}", self.path.display(), if self.dirty { " *" } else { "" }, self.left);
        paint(frame, rect, rect.row, &title, NORMAL);
        let selection = self.selection();
        let low = selection.anchor.min(selection.cursor);
        let high = selection.anchor.max(selection.cursor);
        for visible in 0..text_rows {
            let index = self.top.saturating_add(visible);
            let Some(&start) = self.lines.get(index) else { break; };
            let end = line_end(self.text(), &self.lines, index);
            let row = rect.row.saturating_add(1 + visible);
            if row >= frame.rows { break; }
            info.source_lines += 1;
            let mut visual = 0usize;
            for (offset, ch) in self.text()[start..end].char_indices() {
                let width = if ch == '\t' { 4 - visual % 4 } else { 1 };
                let bg = if start + offset >= low && start + offset < high { SELECTED } else { NORMAL };
                for step in 0..width {
                    let column = visual.saturating_add(step);
                    if column >= self.left && column - self.left < rect.cols {
                        put(frame, row, rect.col.saturating_add(column - self.left), if ch == '\t' { ' ' } else { ch }, bg);
                    }
                }
                visual = visual.saturating_add(width);
                if visual >= self.left.saturating_add(rect.cols) { break; }
            }
            if focused && self.cursor_line() == index {
                let caret = selection.cursor.min(end);
                let visual = self.text()[start..caret].chars().fold(0usize, |v, c| v + if c == '\t' { 4 - v % 4 } else { 1 });
                if visual >= self.left && visual - self.left < rect.cols {
                    let col = rect.col.saturating_add(visual - self.left);
                    if col < frame.cols { frame.cursor = Some((row, col)); }
                }
            }
        }
        if review_rows > 0 {
            let row = rect.row.saturating_add(1 + text_rows);
            if let Some(hunk) = self.document.hunks().get(self.review) {
                let header = format!("Hunk {} {:?} | clipped preview offset {}", hunk.id, hunk.state, self.review_top);
                paint(frame, rect, row, &header, NORMAL);
                if review_rows >= 2 {
                    for (start, label, _) in review_buttons() {
                        if start < rect.cols {
                            let button_rect = Rect { col: rect.col.saturating_add(start), row: rect.row, cols: rect.cols - start, rows: rect.rows };
                            paint(frame, &button_rect, row.saturating_add(1), label, NORMAL);
                        }
                    }
                }
                if let Some(preview) = self.previews.get_mut(&hunk.id) {
                    if preview.width != self.columns {
                        preview.old_rows = preview_rows(&preview.old, self.columns);
                        preview.new_rows = preview_rows(&preview.new, self.columns);
                        preview.width = self.columns;
                    }
                    let available = review_rows.saturating_sub(2);
                    let old_rows = available / 2;
                    let new_rows = available - old_rows;
                    for (text, lines, count, first, bg, old) in [
                        (&preview.old, &preview.old_rows, old_rows, row.saturating_add(2), OLD, true),
                        (&preview.new, &preview.new_rows, new_rows, row.saturating_add(2 + old_rows), NEW, false),
                    ] {
                        for n in 0..count {
                            let actual = first.saturating_add(n);
                            if actual >= frame.rows { break; }
                            for col in rect.col..rect.col.saturating_add(rect.cols).min(frame.cols) { put(frame, actual, col, ' ', bg); }
                            if n == 0 && rect.cols > 0 && rect.col < frame.cols { if old { info.old_row = Some(actual); } else { info.new_row = Some(actual); } }
                            let index = self.review_top.saturating_add(n);
                            if let Some(&(start, end)) = lines.get(index) {
                                paint(frame, rect, actual, &text[start..end], bg);
                            }
                        }
                    }
                }
            }
        }
        if rect.rows > 0 {
            let status = if let Some(prompt) = &self.find { format!("Find: {prompt} | Enter next, Esc close{}", self.error.as_ref().map_or(String::new(), |e| format!(" | {e}"))) }
                else if let Some(error) = &self.error { error.clone() }
                else { "Ctrl+S save | Ctrl+F find | review clipped: scroll preview".into() };
            paint(frame, rect, rect.row.saturating_add(rect.rows - 1), &status, NORMAL);
        }
        info
    }
}

// Review wrapping is cached at proposal time and recomputed only on pane resize.
fn preview_rows(text: &str, width: usize) -> Vec<(usize, usize)> {
    let starts = line_starts(text);
    let mut rows = Vec::new();
    let width = width.max(1);
    for (line, &start) in starts.iter().enumerate() {
        let end = line_end(text, &starts, line);
        let mut beginning = start;
        let mut visual = 0usize;
        for (offset, ch) in text[start..end].char_indices() {
            let byte = start + offset;
            let cells = if ch == '\t' { 4 - visual % 4 } else { 1 };
            if visual > 0 && visual.saturating_add(cells) > width {
                rows.push((beginning, byte));
                beginning = byte;
                visual = 0;
            }
            visual += if ch == '\t' { 4 - visual % 4 } else { 1 };
        }
        rows.push((beginning, end));
    }
    rows
}

fn check_fields(value: &Value, fields: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or("expected argument object")?;
    if let Some(key) = object.keys().find(|key| !fields.contains(&key.as_str())) { return Err(format!("unknown field: {key}")); }
    Ok(())
}
fn review_buttons() -> [(usize, &'static str, &'static str); 6] {
    [(0, "[Accept]", "accept"), (9, "[Reject]", "reject"), (18, "[Accept all]", "accept_all"),
        (31, "[Reject all]", "reject_all"), (44, "[Prev]", "review_previous"), (51, "[Next]", "review_next")]
}
fn put(frame: &mut TerminalSnapshot, row: usize, col: usize, ch: char, bg: [u8; 3]) {
    if row >= frame.rows || col >= frame.cols { return; }
    if let Some(index) = row.checked_mul(frame.cols).and_then(|i| i.checked_add(col)) {
        if let Some(cell) = frame.cells.get_mut(index) { *cell = Cell { ch, fg: [220, 220, 225], bg }; }
    }
}
fn paint(frame: &mut TerminalSnapshot, rect: &Rect, row: usize, text: &str, bg: [u8; 3]) {
    if row < rect.row || row >= rect.row.saturating_add(rect.rows) { return; }
    let mut visual = 0usize;
    for ch in text.chars() {
        let width = if ch == '\t' { 4 - visual % 4 } else { 1 };
        for _ in 0..width {
            if visual >= rect.cols { return; }
            put(frame, row, rect.col.saturating_add(visual), if ch == '\t' { ' ' } else { ch }, bg);
            visual += 1;
        }
    }
}
