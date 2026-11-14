//! The window side of the command input: which pane's input takes keys, the keys, the mouse,
//! sending a command, and the view each pane's input is drawn from.
use super::*;
use clanky::calm::command_bar::{self, Bar, Geometry, Popup};
use clanky::command_input::{Candidate, Highlighter, History, LineEdit, Word};
use clanky::config::CommandInput;
use clanky::look;

/// Where keys go when the input steps aside, in the mockup's words.
const RUNNING: &str = "A command is running: keys go to it. Ctrl+C stops it.";
const FULL_SCREEN: &str = "A full-screen program has the keys.";
const NO_MARKS: &str = "No shell marks: keys go to the shell.";

#[derive(Default)]
struct PaneInput {
    edit: LineEdit,
    /// The prompt (its B mark's number) a command was last sent at: the input waits for the next one.
    sent_seq: u64,
    /// History recall: the text typed before the first Up, and the entry shown.
    recall: Option<(String, Option<usize>)>,
    popup: Option<(Word, Vec<Candidate>, usize)>,
    /// Escape hid the ghost text until the next edit.
    ghost_off: bool,
    notice: Option<String>,
}
#[derive(Default)]
pub struct Inputs {
    panes: BTreeMap<String, PaneInput>,
    history: History,
    saved: Option<std::sync::mpsc::Receiver<Vec<String>>>,
    highlighter: Option<Option<Highlighter>>,
    seen: std::collections::BTreeSet<(String, String)>,
    /// The pane whose text a mouse drag is selecting.
    pub dragging: Option<String>,
}
impl Inputs {
    /// Read the saved history once, off the window's thread.
    pub fn load_saved(&mut self) {
        let Some(path) = clanky_core::platform::history_path() else { return; };
        let (tx, rx) = std::sync::mpsc::channel();
        self.saved = Some(rx);
        let _ = std::thread::Builder::new().name("clanky-input-history".into())
            .spawn(move || { let _ = tx.send(clanky::command_input::saved_commands(&path, 5000)); });
    }
    fn merge_saved(&mut self) {
        if let Some(rx) = &self.saved {
            match rx.try_recv() {
                Ok(older) => { self.history.add_older(older); self.saved = None; }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.saved = None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
    }
}

impl App {
    pub(super) fn input_on(&self) -> bool { self.config.command_input == CommandInput::Editor }
    /// The pane is a terminal (not a file editor) that shows a command input.
    pub(super) fn has_input(&self, id: &str) -> bool { self.input_on() && !self.files.panes.contains_key(id) }
    /// Why the pane's input steps aside now, or None when it takes keys.
    pub(super) fn input_aside(&self, id: &str) -> Option<&'static str> {
        let prompt = self.studio.blocks.get(id).map(|view| &view["prompt"]);
        let integrated = prompt.is_some_and(|p| p["integrated"] == true);
        if !integrated { return Some(NO_MARKS); }
        if self.studio.snapshots.get(id).is_some_and(|s| s.modes & clanky_core::MODE_ALT_SCREEN != 0) { return Some(FULL_SCREEN); }
        let seq = prompt.and_then(|p| p["seq"].as_u64()).unwrap_or(0);
        let sent = self.bars.panes.get(id).map_or(0, |p| p.sent_seq);
        if prompt.is_some_and(|p| p["at_prompt"] == true) && seq > sent { None } else { Some(RUNNING) }
    }
    /// The focused pane, when its input takes keys.
    fn input_target(&self) -> Option<String> {
        if self.files.focus != TERMINALS || self.files.modal() || self.panels.open || self.search.active_target().is_some()
            || self.compose.active_target().is_some() { return None; }
        let id = self.focused()?.to_owned();
        (self.has_input(&id) && self.input_aside(&id).is_none()).then_some(id)
    }
    /// Keys for the focused pane's input. Ok(false): not taken, they go to the program.
    pub(super) fn command_key(&mut self, key: &Key, text: Option<&str>) -> Result<bool, String> {
        let Some(id) = self.input_target() else { return Ok(false); };
        self.bars.merge_saved();
        if self.input.composing() { return Ok(true); }
        let m = self.modifiers;
        let (ctrl, shift, alt) = (m.control_key(), m.shift_key(), m.alt_key() || m.super_key());
        let pane = self.bars.panes.entry(id.clone()).or_default();
        pane.notice = None;
        // The completion list takes Tab, Enter, Up, Down and Escape; any other key closes it first.
        if let Some((word, items, selected)) = pane.popup.take() {
            match key {
                Key::Named(NamedKey::Tab) | Key::Named(NamedKey::Enter) if !ctrl && !alt => {
                    if let Some(item) = items.get(selected) { pane.edit.replace(word.start..word.end, &item.insert); }
                    if *key == Key::Named(NamedKey::Tab) { return Ok(true); }
                    return self.submit(&id).map(|_| true);
                }
                Key::Named(NamedKey::ArrowDown) => { let n = items.len(); pane.popup = Some((word, items, (selected + 1) % n.max(1))); return Ok(true); }
                Key::Named(NamedKey::ArrowUp) => { let n = items.len(); pane.popup = Some((word, items, (selected + n.max(1) - 1) % n.max(1))); return Ok(true); }
                Key::Named(NamedKey::Escape) => return Ok(true),
                _ => {}
            }
        }
        let pane = self.bars.panes.get_mut(&id).expect("pane input");
        let edited = |pane: &mut PaneInput| { pane.recall = None; pane.ghost_off = false; };
        if ctrl {
            if let Key::Character(ch) = key {
                match ch.to_lowercase().as_str() {
                    "z" => { if shift { pane.edit.redo() } else { pane.edit.undo() } edited(pane); }
                    "y" => { pane.edit.redo(); edited(pane); }
                    "a" => pane.edit.home(shift),
                    "e" => pane.edit.end(shift),
                    "u" => { let end = pane.edit.cursor(); let (line, _) = pane.edit.caret(); let start = pane.edit.offset(line, 0); pane.edit.replace(start..end, ""); edited(pane); }
                    "k" => { let start = pane.edit.cursor(); let (line, _) = pane.edit.caret(); let end = pane.edit.offset(line, usize::MAX); pane.edit.replace(start..end, ""); edited(pane); }
                    // Ctrl+C clears the text; on an empty line it goes to the shell, as in a terminal.
                    "c" if !pane.edit.is_empty() => { pane.edit.clear(); edited(pane); }
                    "c" => return self.input_to_shell(&id, b"\x03"),
                    "d" if pane.edit.is_empty() => return self.input_to_shell(&id, b"\x04"),
                    "d" => { pane.edit.delete(); edited(pane); }
                    "l" => return self.input_to_shell(&id, b"\x0c"),
                    _ => {}
                }
            } else if *key == Key::Named(NamedKey::Enter) && !shift { return self.submit(&id).map(|_| true); }
            return Ok(true);
        }
        match key {
            Key::Named(NamedKey::Enter) if shift => { pane.edit.insert("\n"); edited(pane); }
            Key::Named(NamedKey::Enter) => return self.submit(&id).map(|_| true),
            Key::Named(NamedKey::Tab) if !shift => return self.complete(&id).map(|_| true),
            Key::Named(NamedKey::Escape) => { pane.ghost_off = true; }
            Key::Named(NamedKey::Backspace) => { pane.edit.backspace(); edited(pane); }
            Key::Named(NamedKey::Delete) => { pane.edit.delete(); edited(pane); }
            Key::Named(NamedKey::ArrowLeft) => pane.edit.left(shift),
            Key::Named(NamedKey::Home) => pane.edit.home(shift),
            Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::End) => {
                let ghost = (!shift && pane.edit.at_end() && !pane.ghost_off).then(|| self.bars.history.ghost(pane.edit.text()).map(str::to_owned)).flatten();
                if let Some(rest) = ghost { pane.edit.insert(&rest); edited(pane); }
                else if *key == Key::Named(NamedKey::End) { pane.edit.end(shift) } else { pane.edit.right(shift) }
            }
            Key::Named(NamedKey::ArrowUp) | Key::Named(NamedKey::ArrowDown) => {
                let down = *key == Key::Named(NamedKey::ArrowDown);
                if pane.edit.vertical(down, shift) || shift { return Ok(true); }
                let (prefix, from) = pane.recall.clone().unwrap_or_else(|| (pane.edit.text().to_owned(), None));
                if pane.recall.is_none() && down { return Ok(true); }
                match self.bars.history.step(&prefix, from, !down) {
                    Some(i) => { let entry = self.bars.history.entries()[i].clone(); pane.edit.set(&entry); pane.recall = Some((prefix, Some(i))); }
                    None if down => { pane.edit.set(&prefix); pane.recall = None; }
                    None => {}
                }
            }
            Key::Named(NamedKey::PageUp | NamedKey::PageDown) => {}
            _ if alt => {}
            _ => {
                if let Some(text) = text.filter(|t| !t.is_empty() && !t.chars().any(|c| c.is_control())) { pane.edit.insert(text); edited(pane); }
            }
        }
        Ok(true)
    }
    /// Text from the IME or a paste goes into the input when it takes keys.
    pub(super) fn command_text(&mut self, text: &str) -> bool {
        let Some(id) = self.input_target() else { return false; };
        let clean: String = text.replace("\r\n", "\n").chars().map(|c| if c == '\r' { '\n' } else if c.is_control() && c != '\n' && c != '\t' { ' ' } else { c }).collect();
        let pane = self.bars.panes.entry(id).or_default();
        pane.popup = None;
        pane.edit.insert(&clean);
        pane.recall = None; pane.ghost_off = false;
        self.frame_clock.invalidate();
        true
    }
    fn input_to_shell(&mut self, id: &str, bytes: &[u8]) -> Result<bool, String> {
        self.engine.input(id, bytes).map_err(|e| e.message)?;
        self.sent_input = true;
        Ok(true)
    }
    /// Enter: the text goes to the shell as one bracketed paste and a carriage return.
    fn submit(&mut self, id: &str) -> Result<(), String> {
        let pane = self.bars.panes.entry(id.to_owned()).or_default();
        let text = pane.edit.text().to_owned();
        if text.trim().is_empty() { return Ok(()); }
        let bracketed = self.studio.snapshots.get(id).is_some_and(|s| s.modes & clanky_core::MODE_BRACKETED_PASTE != 0);
        let bytes = match clanky::command_input::submit_bytes(&text, bracketed) {
            Ok(bytes) => bytes,
            Err(why) => { pane.notice = Some(why); return Ok(()); }
        };
        let seq = self.studio.blocks.get(id).and_then(|v| v["prompt"]["seq"].as_u64()).unwrap_or(0);
        clanky::compose::send_chunked(&bytes, clanky::compose::INPUT_CHUNK, |piece| self.engine.input(id, piece).map_err(|e| e.message))?;
        let pane = self.bars.panes.get_mut(id).expect("pane input");
        pane.sent_seq = seq;
        pane.edit = LineEdit::default();
        pane.recall = None; pane.ghost_off = false;
        self.bars.history.push(&text);
        self.terminal_dirty = true;
        Ok(())
    }
    /// Tab: one candidate goes in at once; more open the list.
    fn complete(&mut self, id: &str) -> Result<(), String> {
        let info = self.engine.exchange(3, "terminal_info", &json!({"tab_id":id})).ok();
        let cwd = info.as_ref().and_then(|i| i["cwd"].as_str()).map(std::path::PathBuf::from)
            .or_else(|| self.studio.terminal_info.get(id).and_then(|i| i["cwd"].as_str()).map(std::path::PathBuf::from));
        let Some(cwd) = cwd.filter(|_| self.local_files) else { return Ok(()); };
        let home = clanky_core::platform::home_dir();
        let path = std::env::var_os("PATH");
        let pane = self.bars.panes.entry(id.to_owned()).or_default();
        let (lo, hi) = pane.edit.selection();
        if lo != hi { return Ok(()); }
        let (word, items) = clanky::command_input::complete(pane.edit.text(), hi, &cwd, home.as_deref(), path.as_deref());
        match items.len() {
            0 => {}
            1 => { pane.edit.replace(word.start..word.end, &items[0].insert); pane.recall = None; }
            _ => pane.popup = Some((word, items, 0)),
        }
        Ok(())
    }
    /// A click in a pane's input: focus the pane; on the text, place the caret. True when taken.
    pub(super) fn command_click(&mut self, layout: &look::Layout) -> Result<bool, String> {
        self.bars.dragging = None;
        let Some((id, g)) = self.input_at(layout) else { return Ok(false); };
        if self.files.focus != TERMINALS || self.focused() != Some(id.as_str()) {
            self.files.focus_on(TERMINALS.into());
            if let Some(group) = self.groups.get_mut(self.active) { group.focus = id.clone(); }
            self.save_workspace()?;
            self.reset_ime();
        }
        if self.input_aside(&id).is_some() { return Ok(true); }
        if let Some((line, col)) = self.text_point(&id, &g) {
            let extend = self.modifiers.shift_key();
            let pane = self.bars.panes.entry(id.clone()).or_default();
            pane.popup = None;
            pane.edit.click(line, col, extend);
            self.bars.dragging = Some(id);
        }
        Ok(true)
    }
    pub(super) fn command_drag(&mut self, layout: &look::Layout) -> bool {
        let Some(id) = self.bars.dragging.clone() else { return false; };
        let Some(pane) = layout.pane(&id) else { return false; };
        let lines = self.bars.panes.get(&id).map_or(1, |p| p.edit.lines().len());
        let g = command_bar::geometry(layout, pane.rect, lines);
        let (line, col) = self.text_point_clamped(&id, &g);
        if let Some(pane) = self.bars.panes.get_mut(&id) { pane.edit.click(line, col, true); }
        true
    }
    fn input_at(&self, layout: &look::Layout) -> Option<(String, Geometry)> {
        layout.panes.iter().filter(|p| self.has_input(&p.id)).find_map(|pane| {
            let lines = self.bars.panes.get(&pane.id).map_or(1, |p| p.edit.lines().len());
            let g = command_bar::geometry(layout, pane.rect, lines);
            g.area.contains(self.mouse.0, self.mouse.1).then(|| (pane.id.clone(), g))
        })
    }
    fn first_line(&self, id: &str, g: &Geometry) -> (usize, usize) {
        let pane = self.bars.panes.get(id);
        let (line, col) = pane.map_or((0, 0), |p| p.edit.caret());
        (line.saturating_sub(g.lines - 1), col.saturating_sub(g.cols.saturating_sub(1)))
    }
    fn text_point(&self, id: &str, g: &Geometry) -> Option<(usize, usize)> {
        let (x, y) = self.mouse;
        let h = g.lines as f32 * g.cell[1];
        if (y as f32) < g.line_y || y as f32 >= g.line_y + h || (x as f32) < g.text_x - g.cell[0] { return None; }
        Some(self.text_point_clamped(id, g))
    }
    fn text_point_clamped(&self, id: &str, g: &Geometry) -> (usize, usize) {
        let (top, left) = self.first_line(id, g);
        let row = ((self.mouse.1 as f32 - g.line_y) / g.cell[1]).floor().clamp(0.0, (g.lines - 1) as f32) as usize;
        let col = ((self.mouse.0 as f32 - g.text_x) / g.cell[0]).floor().max(0.0) as usize;
        (top + row, left + col)
    }
    /// The view of every visible terminal pane's input, for drawing.
    pub(super) fn command_bars(&mut self, layout: &look::Layout) -> Vec<Bar> {
        if !self.input_on() { return Vec::new(); }
        self.bars.merge_saved();
        // Commands the panes ran (typed here or in classic mode) join the history once each.
        for (pane, view) in &self.studio.blocks {
            for block in view["blocks"].as_array().into_iter().flatten().filter(|b| b["complete"] == true) {
                let (Some(id), Some(command)) = (block["block_id"].as_str(), block["command"].as_str()) else { continue; };
                if self.bars.seen.insert((pane.clone(), id.to_owned())) && !command.contains("****") { self.bars.history.push(command); }
            }
        }
        if self.bars.seen.len() > 20000 { self.bars.seen.clear(); }
        let focused = self.focused().map(str::to_owned).filter(|_| self.files.focus == TERMINALS);
        let caret_shown = self.blink_shown;
        let target = self.input_target();
        let ids: Vec<String> = layout.panes.iter().map(|p| p.id.clone()).filter(|id| self.has_input(id)).collect();
        self.bars.panes.retain(|id, _| self.groups.iter().any(|g| g.layout.contains(id)));
        let ids: Vec<(String, Option<&'static str>)> = ids.into_iter().map(|id| { let aside = self.input_aside(&id); (id, aside) }).collect();
        let highlighter = self.bars.highlighter.get_or_insert_with(Highlighter::new);
        let mut out = Vec::new();
        for (id, aside) in ids {
            let Some(pane) = layout.pane(&id) else { continue; };
            let state = self.bars.panes.entry(id.clone()).or_default();
            let lines_n = state.edit.lines().len();
            let g = command_bar::geometry(layout, pane.rect, lines_n);
            let text = state.edit.text().to_owned();
            let toks = highlighter.as_mut().map(|h| h.tokens(&text)).unwrap_or_else(|| vec![clanky::command_input::Tok::Plain; text.len()]);
            let (lo, hi) = state.edit.selection();
            let mut lines = Vec::new();
            for (start, end) in state.edit.lines() {
                lines.push(text[start..end].char_indices().map(|(i, ch)| (ch, toks[start + i], start + i >= lo && start + i < hi)).collect::<Vec<_>>());
            }
            let (line, col) = state.edit.caret();
            let top_line = line.saturating_sub(g.lines - 1);
            let left_col = col.saturating_sub(g.cols.saturating_sub(1));
            let ghost = (aside.is_none() && state.popup.is_none() && !state.ghost_off && state.edit.at_end())
                .then(|| self.bars.history.ghost(&text).map(str::to_owned)).flatten().unwrap_or_default();
            let info = self.studio.terminal_info.get(&id);
            let cwd = info.and_then(|i| i["cwd"].as_str()).map(str::to_owned);
            let folder = cwd.as_deref().map(home_short).unwrap_or_default();
            let branch = cwd.as_deref().and_then(studio::git_branch);
            let shell = info.and_then(|i| i["exe"].as_str()).and_then(|e| std::path::Path::new(e).file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let access = (!self.applied_post_ai).then(|| self.studio.permissions.get(&id).copied().unwrap_or(false));
            let is_target = target.as_deref() == Some(id.as_str());
            let caret = (is_target && focused.as_deref() == Some(id.as_str()) && caret_shown).then_some((line, col));
            let popup = state.popup.as_ref().filter(|_| is_target).map(|(_, items, selected)| Popup {
                items: items.iter().map(|c| (c.shown.clone(), c.kind.label())).collect(), selected: *selected });
            out.push(Bar { pane: id.clone(), geometry: Some(g), lines, top_line, left_col, ghost, caret, folder, branch, shell,
                access, aside, notice: state.notice.clone(), popup });
        }
        out
    }
}
fn home_short(cwd: &str) -> String {
    match clanky_core::platform::home_dir().and_then(|home| home.to_str().map(str::to_owned)) {
        Some(home) if cwd == home => "~".into(),
        Some(home) if cwd.starts_with(&(home.clone() + "/")) => format!("~{}", &cwd[home.len()..]),
        _ => cwd.into(),
    }
}
