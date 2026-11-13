//! The mouse over a terminal pane: reports to programs that ask for them, and
//! otherwise selection, copy and paste the way VTE (and so Ptyxis) does it.
use super::*;
use clanky::mouse::{self, Clicks, Kind, Mods, Pos, Selection};
use clanky_core::platform::Clipboard;
use winit::window::CursorIcon;

/// A selection in one pane. `lines` holds the selected text of each line as it
/// was when the drag ended; when the program changes that text, the selection goes.
pub struct Held {
    pub pane: String,
    pub sel: Selection,
    pub dragging: bool,
    pub lines: Vec<(i64, String)>,
}

#[derive(Default)]
pub struct TermMouse {
    pub selection: Option<Held>,
    clicks: Clicks,
    /// Buttons held over a pane (bit 0 left, 1 middle, 2 right), and those the
    /// window used itself (selection, paste) instead of reporting them.
    buttons: u8,
    handled: u8,
    /// The pane a press went to; drags and releases stay with it.
    pane: Option<String>,
    last_cell: Option<(String, usize, usize)>,
    wheel: (f64, f64),
    primary: Clipboard,
    clipboard: Clipboard,
    icon: Option<CursorIcon>,
}

/// One cell and which half of it the pointer is over.
struct Hit {
    id: String,
    col: usize,
    row: usize,
    right: bool,
    rows: usize,
}

impl App {
    fn mouse_mods(&self) -> Mods {
        Mods { shift: self.modifiers.shift_key(), alt: self.modifiers.alt_key(), ctrl: self.modifiers.control_key() }
    }

    /// Whether a card, prompt or panel takes the mouse instead of the panes.
    pub(super) fn mouse_overlay(&self) -> bool {
        self.settings_open() || self.palette.open || self.agent_picker.open || self.panels.open || self.files.modal()
            || self.compose.active_target().is_some() || self.search.active_target().is_some()
    }

    /// The terminal pane cell under the pointer; `pane` keeps it to that pane, clamped to its edges.
    fn pointer_cell(&self, pane: Option<&str>) -> Option<Hit> {
        if self.mouse_overlay() || self.context_menu.open { return None; }
        let layout = self.pixel_layout().ok()?;
        let pane = match pane {
            Some(id) => layout.pane(id)?,
            None => layout.pane_at(self.mouse.0, self.mouse.1).filter(|p| p.grid_clip.contains(self.mouse.0, self.mouse.1))?,
        };
        if self.files.panes.contains_key(&pane.id) || pane.rows == 0 || pane.cols == 0 { return None; }
        let (cw, ch) = (layout.cell_px[0] as f64, layout.cell_px[1] as f64);
        let x = ((self.mouse.0 - pane.grid_origin[0] as f64) / cw).max(0.0);
        let y = ((self.mouse.1 - pane.grid_origin[1] as f64) / ch).max(0.0);
        let col = (x.floor() as usize).min(pane.cols - 1);
        let right = x.floor() as usize >= pane.cols || x.fract() >= 0.5;
        Some(Hit { id: pane.id.clone(), col, row: (y.floor() as usize).min(pane.rows - 1), right, rows: pane.rows })
    }

    fn pane_snapshot(&self, id: &str) -> Option<&TerminalSnapshot> { self.studio.snapshots.get(id) }

    fn report(&mut self, id: &str, bytes: Option<Vec<u8>>) -> Result<(), String> {
        if let Some(bytes) = bytes {
            self.engine.input(id, &bytes).map_err(|e| e.message)?;
            self.sent_input = true;
        }
        Ok(())
    }

    /// A button pressed over a terminal pane (1 left, 2 middle, 3 right). Left
    /// presses arrive after click_studio has focused the pane.
    pub(super) fn terminal_press(&mut self, button: u8) -> Result<(), String> {
        let Some(hit) = self.pointer_cell(None) else { return Ok(()); };
        if button != 1 { self.focus_terminal(&hit.id)?; }
        let Some(snapshot) = self.pane_snapshot(&hit.id).cloned() else { return Ok(()); };
        let mods = self.mouse_mods();
        let reporting = mouse::reporting(snapshot.modes);
        let bit = 1u8 << (button - 1);
        self.term_mouse.buttons |= bit;
        self.term_mouse.pane = Some(hit.id.clone());
        let line = mouse::top_line(&snapshot) + hit.row as i64;
        // Shift overrides reporting: the window selects and pastes instead.
        if !reporting || mods.shift {
            match button {
                1 => {
                    let count = self.term_mouse.clicks.press(Instant::now(), line, hit.col);
                    let at = Pos { line, col: hit.col, right: hit.right };
                    let extend = count == 1 && !reporting && mods.shift;
                    let extended = extend && match &mut self.term_mouse.selection {
                        Some(held) if held.pane == hit.id => { held.sel.head = at; held.dragging = true; true }
                        _ => false,
                    };
                    if !extended {
                        let kind = match count { 2 => Kind::Word, 3 => Kind::Line, _ => Kind::Char };
                        self.term_mouse.selection = Some(Held { pane: hit.id.clone(), sel: Selection::new(kind, kind == Kind::Char && mods.ctrl, at), dragging: true, lines: Vec::new() });
                    }
                    self.term_mouse.handled |= bit;
                    self.frame_clock.invalidate();
                    return Ok(());
                }
                2 => {
                    self.term_mouse.handled |= bit;
                    return self.paste_selection(&hit.id, true);
                }
                // The menu; the selection stays as it is.
                3 => {
                    self.term_mouse.handled |= bit;
                    return self.open_menu(&hit.id);
                }
                _ => {}
            }
        }
        if reporting && snapshot.display_offset == 0 {
            let bytes = mouse::report(snapshot.modes, button, mods, hit.col, hit.row, false, false);
            self.term_mouse.last_cell = Some((hit.id.clone(), hit.col, hit.row));
            self.report(&hit.id, bytes)?;
        }
        Ok(())
    }

    pub(super) fn terminal_release(&mut self, button: u8) -> Result<(), String> {
        let bit = 1u8 << (button - 1);
        // Only a press that went to a pane has a release there.
        if self.term_mouse.buttons & bit == 0 { return Ok(()); }
        let pane = self.term_mouse.pane.clone();
        self.term_mouse.buttons &= !bit;
        if self.term_mouse.buttons == 0 { self.term_mouse.pane = None; }
        if self.term_mouse.handled & bit != 0 {
            self.term_mouse.handled &= !bit;
            if button == 1 { self.end_selection()?; }
            return Ok(());
        }
        let Some(hit) = self.pointer_cell(pane.as_deref()) else { return Ok(()); };
        let Some(snapshot) = self.pane_snapshot(&hit.id) else { return Ok(()); };
        if mouse::reporting(snapshot.modes) && snapshot.display_offset == 0 {
            let bytes = mouse::report(snapshot.modes, button, self.mouse_mods(), hit.col, hit.row, false, true);
            self.report(&hit.id, bytes)?;
        }
        Ok(())
    }

    /// Pointer motion: grow a selection, or report motion when the program asked.
    pub(super) fn terminal_motion(&mut self) -> Result<(), String> {
        self.update_pointer_icon();
        if let Some(held) = self.term_mouse.selection.as_ref().filter(|h| h.dragging && self.term_mouse.handled & 1 != 0) {
            let id = held.pane.clone();
            let Some(hit) = self.pointer_cell(Some(&id)) else { return Ok(()); };
            let Some(top) = self.pane_snapshot(&id).map(mouse::top_line) else { return Ok(()); };
            let head = Pos { line: top + hit.row as i64, col: hit.col, right: hit.right };
            if let Some(held) = &mut self.term_mouse.selection { held.sel.head = head; }
            return Ok(());
        }
        let held = self.term_mouse.buttons & !self.term_mouse.handled;
        let pane = self.term_mouse.pane.clone().filter(|_| held != 0);
        let Some(hit) = self.pointer_cell(pane.as_deref()) else { return Ok(()); };
        let Some(snapshot) = self.pane_snapshot(&hit.id) else { return Ok(()); };
        if snapshot.display_offset != 0 || !mouse::reports_motion(snapshot.modes, held != 0) { return Ok(()); }
        let cell = (hit.id.clone(), hit.col, hit.row);
        if self.term_mouse.last_cell.as_ref() == Some(&cell) { return Ok(()); }
        // As in xterm, motion carries the lowest button held, if any.
        let button = if held == 0 { 0 } else { held.trailing_zeros() as u8 + 1 };
        let bytes = mouse::report(snapshot.modes, button, self.mouse_mods(), hit.col, hit.row, true, false);
        self.term_mouse.last_cell = Some(cell);
        self.report(&hit.id, bytes)
    }

    pub(super) fn end_selection(&mut self) -> Result<(), String> {
        let Some(held) = &mut self.term_mouse.selection else { return Ok(()); };
        held.dragging = false;
        let Some(snapshot) = self.studio.snapshots.get(&held.pane) else { return Ok(()); };
        let text = held.sel.text(snapshot);
        if held.sel.spans(snapshot).is_empty() || text.is_empty() {
            self.term_mouse.selection = None;
            self.frame_clock.invalidate();
            return Ok(());
        }
        held.lines = line_texts(&held.sel, snapshot);
        self.term_mouse.primary.copy_to(text.as_bytes(), true).map_err(|_| "Could not set the selection".to_owned())
    }

    pub(super) fn copy_text(&mut self, text: &str) -> Result<(), String> {
        if text.is_empty() { return Ok(()); }
        self.term_mouse.clipboard.copy_to(text.as_bytes(), false).map_err(|_| "Could not copy".to_owned())
    }

    /// Ctrl+Shift+C: the selection to the clipboard.
    pub(super) fn copy_selection(&mut self) -> Result<(), String> {
        let Some(held) = &self.term_mouse.selection else { return Ok(()); };
        let Some(snapshot) = self.studio.snapshots.get(&held.pane) else { return Ok(()); };
        let text = held.sel.text(snapshot);
        if text.is_empty() { return Ok(()); }
        self.term_mouse.clipboard.copy_to(text.as_bytes(), false).map_err(|_| "Could not copy".to_owned())
    }

    /// Ctrl+Shift+V (the clipboard) or a middle click (PRIMARY) into a pane, as VTE pastes.
    pub(super) fn paste_selection(&mut self, id: &str, primary: bool) -> Result<(), String> {
        let bytes = platform::selection_read(primary).map_err(|_| "Nothing to paste".to_owned())?;
        let text = String::from_utf8_lossy(&bytes);
        if text.is_empty() { return Ok(()); }
        let bracketed = self.pane_snapshot(id).is_some_and(|s| s.modes & clanky_core::MODE_BRACKETED_PASTE != 0);
        let bytes = mouse::pastify(&text, bracketed);
        clanky::compose::send_chunked(&bytes, clanky::compose::INPUT_CHUNK, |piece| self.engine.input(id, piece).map_err(|e| e.message))?;
        self.sent_input = true;
        Ok(())
    }

    fn focus_terminal(&mut self, id: &str) -> Result<(), String> {
        self.files.focus_on(TERMINALS.into());
        if let Some(group) = self.groups.get_mut(self.active) {
            if group.focus == id { return Ok(()); }
            group.focus = id.to_owned();
        }
        self.save_workspace()?;
        self.reset_ime();
        Ok(())
    }

    /// The wheel over a terminal pane. Returns false when the view should scroll its history.
    pub(super) fn terminal_wheel(&mut self, delta: MouseScrollDelta, cell_px: [u32; 2]) -> Result<bool, String> {
        let (dx, dy) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x as f64, y as f64),
            MouseScrollDelta::PixelDelta(p) => (p.x / cell_px[0] as f64, p.y / cell_px[1] as f64),
        };
        // Ctrl+wheel zooms, as in Ptyxis with enable-zoom-scroll-ctrl: one step per event.
        if self.modifiers.control_key() {
            if dy != 0.0 {
                self.zoom.adjust(if dy > 0.0 { 1.0 } else { -1.0 });
                self.metrics_changed()?;
            }
            return Ok(true);
        }
        let Some(hit) = self.pointer_cell(None) else { return Ok(false); };
        let Some(snapshot) = self.pane_snapshot(&hit.id).cloned() else { return Ok(false); };
        let reporting = mouse::reporting(snapshot.modes);
        let keys = |up| mouse::wheel_keys(snapshot.modes, snapshot.app_cursor, hit.rows, up);
        if !reporting && keys(true).is_none() {
            self.term_mouse.wheel = (0.0, 0.0);
            return Ok(false);
        }
        self.term_mouse.wheel.0 += dx;
        self.term_mouse.wheel.1 += dy;
        let (nx, ny) = (self.term_mouse.wheel.0.trunc(), self.term_mouse.wheel.1.trunc());
        self.term_mouse.wheel.0 -= nx;
        self.term_mouse.wheel.1 -= ny;
        let mut bytes = Vec::new();
        let mods = self.mouse_mods();
        for _ in 0..ny.abs() as usize {
            // winit's positive y is away from the user: wheel up, button 4.
            if reporting {
                bytes.extend(mouse::report(snapshot.modes, if ny > 0.0 { 4 } else { 5 }, mods, hit.col, hit.row, false, false).unwrap_or_default());
            } else {
                bytes.extend(keys(ny > 0.0).unwrap_or_default());
            }
        }
        if reporting {
            for _ in 0..nx.abs() as usize {
                bytes.extend(mouse::report(snapshot.modes, if nx > 0.0 { 7 } else { 6 }, mods, hit.col, hit.row, false, false).unwrap_or_default());
            }
        }
        if !bytes.is_empty() { self.report(&hit.id, Some(bytes))?; }
        Ok(true)
    }

    /// Focus in and out reports (1004) for the focused pane.
    pub(super) fn terminal_focus(&mut self, focused: bool) -> Result<(), String> {
        let Some(id) = self.focused().map(str::to_owned) else { return Ok(()); };
        if self.pane_snapshot(&id).is_some_and(|s| s.modes & clanky_core::MODE_FOCUS != 0) {
            self.report(&id, Some(if focused { b"\x1b[I".to_vec() } else { b"\x1b[O".to_vec() }))?;
        }
        Ok(())
    }

    /// An I-beam over terminal text; an arrow while the program takes the mouse.
    fn update_pointer_icon(&mut self) {
        let icon = match self.pointer_cell(None) {
            Some(hit) if !self.pane_snapshot(&hit.id).is_some_and(|s| mouse::reporting(s.modes)) => CursorIcon::Text,
            _ => CursorIcon::Default,
        };
        if self.term_mouse.icon != Some(icon) {
            if let Some(window) = &self.window { window.set_cursor(icon); }
            self.term_mouse.icon = Some(icon);
        }
    }

    /// Selected cells of one pane to draw highlighted, or None. Drops the selection when
    /// the text it covered has changed under it (as VTE does when output rewrites it).
    pub(super) fn selection_spans(&mut self, id: &str, snapshot: &TerminalSnapshot) -> Option<Vec<mouse::Span>> {
        let held = self.term_mouse.selection.as_ref().filter(|h| h.pane == id)?;
        if !held.dragging {
            let now = line_texts(&held.sel, snapshot);
            let top = mouse::top_line(snapshot);
            let visible = |line: i64| line >= top && line < top + snapshot.rows as i64;
            let changed = held.lines.iter().any(|(line, text)| visible(*line) && now.iter().any(|(l, t)| l == line && t != text));
            if changed { self.term_mouse.selection = None; return None; }
        }
        Some(held.sel.spans(snapshot))
    }
}

/// The selected text on each line in view, to notice when output rewrites it.
fn line_texts(sel: &Selection, snapshot: &TerminalSnapshot) -> Vec<(i64, String)> {
    let top = mouse::top_line(snapshot);
    sel.spans(snapshot).into_iter().filter_map(|(line, s, e)| {
        let row = line - top;
        (row >= 0 && (row as usize) < snapshot.rows).then(|| {
            let start = row as usize * snapshot.cols;
            (line, snapshot.cells[start + s..start + e].iter().map(|c| c.ch).collect())
        })
    }).collect()
}
