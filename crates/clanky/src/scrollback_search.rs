//! Pinned, immutable search views shared by the window and headless gate.
use clanky_api::Client;
use clanky_core::{Cell, TerminalSnapshot, layout::Rect};
use serde_json::json;
use winit::keyboard::{Key, ModifiersState, NamedKey};

struct Position { row: usize, col: usize }
struct Highlight { row: usize, col: usize, selected: bool }
struct Reply {
    total: usize,
    selected: Option<Position>,
    snapshot: TerminalSnapshot,
    highlights: Vec<Highlight>,
}

#[derive(Default)]
pub struct Search {
    target: Option<String>,
    query: String,
    view: Option<Reply>,
}

impl Search {
    pub fn active_target(&self) -> Option<&str> { self.target.as_deref() }
    pub fn close(&mut self) { self.target = None; self.query.clear(); self.view = None; }

    pub fn open(&mut self, client: &Client, tab: &str) -> Result<(), String> {
        if tab.is_empty() { return Err("no search target".into()); }
        let view = Self::request(client, tab, "", None, false)?;
        self.target = Some(tab.to_owned());
        self.query.clear();
        self.view = Some(view);
        Ok(())
    }

    fn request(client: &Client, target: &str, query: &str, after: Option<&Position>, backwards: bool)
        -> Result<Reply, String>
    {
        let after = after.map(|p| json!({"row":p.row,"col":p.col}));
        let value = client.exchange(3, "window_search", &json!({
            "tab_id":target,"query":query,"after":after,"backwards":backwards
        })).map_err(|e| e.message)?;
        // Validate dimensions and cell count before decoding or indexing a remote
        // snapshot. Do not trust a remote server to preserve the local invariant.
        let rows = value["snapshot"]["rows"].as_u64().ok_or("invalid search snapshot")?;
        let cols = value["snapshot"]["cols"].as_u64().ok_or("invalid search snapshot")?;
        if rows == 0 || cols == 0 || rows > 1000 || cols > 1000 || rows * cols > 250000 {
            return Err("invalid search snapshot dimensions".into());
        }
        let count = (rows * cols) as usize;
        if value["snapshot"]["cells"].as_array().map(Vec::len) != Some(count)
            || value["highlights"].as_array().is_none_or(|v| v.len() > count)
        { return Err("invalid search snapshot cells".into()); }
        let coordinate = |value: &serde_json::Value| -> Result<Position, String> {
            let number = |name: &str| value[name].as_u64()
                .and_then(|n| usize::try_from(n).ok()).ok_or_else(|| "invalid search position".to_owned());
            Ok(Position { row: number("row")?, col: number("col")? })
        };
        let total = value["total"].as_u64().and_then(|n| usize::try_from(n).ok())
            .ok_or("invalid search count")?;
        let selected = if value.get("selected").is_some_and(|v| v.is_null()) { None }
            else { Some(coordinate(&value["selected"])?) };
        let highlights = value["highlights"].as_array().unwrap().iter().map(|h| {
            let p = coordinate(h)?;
            Ok(Highlight { row: p.row, col: p.col,
                selected: h["selected"].as_bool().ok_or("invalid search highlight")? })
        }).collect::<Result<Vec<_>, String>>()?;
        let snapshot = serde_json::from_value(value["snapshot"].clone())
            .map_err(|_| "invalid search snapshot")?;
        let mut reply = Reply { total, selected, snapshot, highlights };
        let snapshot = &mut reply.snapshot;
        if snapshot.cursor.is_some_and(|(r, c)| r >= snapshot.rows || c >= snapshot.cols)
            || reply.highlights.iter().any(|h| h.row >= snapshot.rows || h.col >= snapshot.cols)
            || reply.selected.as_ref().is_some_and(|p| p.col >= snapshot.cols)
            || (reply.total == 0) != reply.selected.is_none()
        { return Err("invalid search response coordinates".into()); }
        snapshot.images.clear();
        snapshot.cursor = None;
        Ok(reply)
    }

    fn change(&mut self, client: &Client, query: String, navigate: bool, backwards: bool)
        -> Result<(), String>
    {
        if query.len() > 256 { return Err("search query exceeds 256 bytes".into()); }
        let target = self.target.as_deref().ok_or("search is closed")?;
        let after = if navigate { self.view.as_ref().and_then(|v| v.selected.as_ref()) } else { None };
        let view = Self::request(client, target, &query, after, backwards)?;
        // Failed calls leave the old query and its exact snapshot intact.
        self.query = query;
        self.view = Some(view);
        Ok(())
    }

    pub fn text(&mut self, client: &Client, text: &str) -> Result<(), String> {
        if self.target.is_none() { return Err("search is closed".into()); }
        if text.is_empty() { return Ok(()); }
        if self.query.len().saturating_add(text.len()) > 256 {
            return Err("search query exceeds 256 bytes".into());
        }
        let mut query = self.query.clone();
        query.push_str(text);
        self.change(client, query, false, false)
    }

    pub fn key(&mut self, client: &Client, key: &Key, text: Option<&str>, mods: ModifiersState,
        composing: bool) -> Result<(), String>
    {
        if self.target.is_none() { return Err("search is closed".into()); }
        if *key == Key::Named(NamedKey::Escape) { self.close(); return Ok(()); }
        if composing { return Ok(()); }
        match key {
            Key::Named(NamedKey::Enter) => self.change(client, self.query.clone(), true, mods.shift_key()),
            Key::Named(NamedKey::Backspace) => {
                let mut query = self.query.clone();
                if query.pop().is_none() { return Ok(()); }
                self.change(client, query, false, false)
            }
            _ if mods.control_key() || mods.alt_key() || mods.super_key() => Ok(()),
            Key::Character(ch) => self.text(client, text.unwrap_or(ch.as_str())),
            Key::Named(NamedKey::Space) => self.text(client, " "),
            _ => Ok(()),
        }
    }

    pub fn draw(&self, frame: &mut TerminalSnapshot, rect: &Rect) {
        let Some(view) = &self.view else { return; };
        if self.target.is_none() || frame.cols == 0 || frame.rows == 0
            || frame.rows.checked_mul(frame.cols) != Some(frame.cells.len()) { return; }
        let source = &view.snapshot;
        // Row zero belongs to the prompt, never to source text or hit cells.
        let top = rect.row.max(1);
        let bottom = rect.row.saturating_add(rect.rows).min(frame.rows);
        let right = rect.col.saturating_add(rect.cols).min(frame.cols);
        for row in top..bottom {
            for col in rect.col..right {
                let sr = row - top;
                let sc = col - rect.col;
                frame.cells[row * frame.cols + col] = if sr < source.rows && sc < source.cols {
                    source.cells[sr * source.cols + sc].clone()
                } else { Cell { ch: ' ', fg: [229; 3], bg: [0; 3], color_source: None } };
            }
        }
        for h in &view.highlights {
            let row = top.saturating_add(h.row);
            let col = rect.col.saturating_add(h.col);
            if row < bottom && col < right {
                let cell=&mut frame.cells[row * frame.cols + col];
                cell.bg = if h.selected { [120,85,25] } else { [65,60,30] };
                if let Some(source)=&mut cell.color_source {
                    if source.inverse {source.fg=clanky_core::ColorSource::Truecolor;}
                    else {source.bg=clanky_core::ColorSource::Truecolor;}
                }
            }
        }
        frame.images.retain(|image| {
            image.x.saturating_add(image.cols) <= rect.col || image.x >= right
                || image.y.saturating_add(image.rows) <= rect.row || image.y >= bottom
        });
        frame.cursor = None;
        let status = format!("Search: {}  [{} matches]  Enter/Shift+Enter  Esc", self.query, view.total);
        let mut chars = status.chars();
        for cell in &mut frame.cells[..frame.cols] {
            let ch = chars.next().unwrap_or(' ');
            *cell = Cell { ch: if ch.is_control() { ' ' } else { ch }, fg: [235;3], bg: [35,35,40], color_source: None };
        }
    }
}
