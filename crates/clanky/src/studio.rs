//! Pixel window composition and input. Server trees never contain local file IDs.
use super::*;
use clanky::look::{self, PixelRect, PaneLayout, LookState, PaneState, PanelState, PanelLine, HitAction, Hotspot, CursorMotion};
use clanky_core::layout::Layout as Tree;
use clanky_render::{Layer, TextRun, Shape, Fill, GridLayer};

pub struct Capture {
    layout: look::Layout,
    index: usize,
    revision: u64,
    local: Tree,
}
struct Cursor {
    from: [f32; 2],
    to: [f32; 2],
    start: Instant,
}
struct Review { content: PixelRect, first_row: usize }
struct Scroll {
    old: TerminalSnapshot,
    start: Instant,
}
pub struct Studio {
    pub drag: Option<Capture>,
    theme: look::Theme,
    hits: Vec<Hotspot>,
    metadata: Value,
    permissions: BTreeMap<String, bool>,
    blocks: BTreeMap<String, Value>,
    next_metadata: Instant,
    cursors: BTreeMap<String, Cursor>,
    snapshots: BTreeMap<String, TerminalSnapshot>,
    shown: BTreeMap<String, (TerminalSnapshot, f32)>,
    reviews: BTreeMap<String, Review>,
    scrolls: BTreeMap<String, Scroll>,
    panels: BTreeMap<String, Instant>,
    activity: Vec<f32>,
    started: Instant,
}
impl Default for Studio {
    fn default() -> Self {
        Self { drag: None, theme: look::Theme::default(), hits: Vec::new(), metadata: Value::Null, permissions: BTreeMap::new(),
            blocks: BTreeMap::new(), next_metadata: Instant::now(), cursors: BTreeMap::new(),
            snapshots: BTreeMap::new(), shown: BTreeMap::new(), reviews: BTreeMap::new(), scrolls: BTreeMap::new(), panels: BTreeMap::new(),
            activity: Vec::new(), started: Instant::now() }
    }
}

/// Substitute just the terminal-workspace leaf, preserving both owners' ratios.
pub fn expand(local: &Tree, server: &Tree) -> Tree {
    match local {
        Tree::Leaf(id) if id == TERMINALS => server.clone(),
        Tree::Leaf(_) => local.clone(),
        Tree::Split { axis, ratio, first, second } => Tree::Split { axis: *axis, ratio: *ratio,
            first: Box::new(expand(first, server)), second: Box::new(expand(second, server)) },
    }
}
fn terminal_path(tree: &Tree, path: &mut Vec<bool>) -> bool {
    match tree {
        Tree::Leaf(id) => id == TERMINALS,
        Tree::Split { first, second, .. } => {
            path.push(false);
            if terminal_path(first, path) { return true; }
            path.pop(); path.push(true);
            if terminal_path(second, path) { return true; }
            path.pop(); false
        }
    }
}
fn rgb(color: look::Color) -> [u8;3] { [color[0],color[1],color[2]] }
fn widget(snapshot: &mut TerminalSnapshot, theme: &look::Theme) {
    for cell in &mut snapshot.cells { cell.fg = theme.ui_color(cell.fg,false); cell.bg = theme.ui_color(cell.bg,true); }
}
fn cells(rows: usize, cols: usize, theme: &look::Theme) -> TerminalSnapshot {
    TerminalSnapshot { rows, cols, cells: vec![Cell { ch: ' ', fg: rgb(theme.ink), bg: rgb(theme.panel) }; rows * cols],
        cursor: None, images: Vec::new() }
}
fn local_rect(pane: &PaneLayout) -> Rect { Rect { row: 0, col: 0, rows: pane.rows, cols: pane.cols } }
fn row_text(snapshot: &TerminalSnapshot, row: usize) -> String {
    snapshot.cells.get(row * snapshot.cols..(row + 1) * snapshot.cols).unwrap_or(&[])
        .iter().map(|c| c.ch).collect::<String>().trim_end().to_owned()
}
fn point(pane: &PaneLayout, mouse: (f64, f64), cell: [u32;2], clamp: bool) -> Option<(usize,usize)> {
    if pane.rows == 0 || pane.cols == 0 || (!clamp && !pane.grid_clip.contains(mouse.0, mouse.1)) { return None; }
    let col = ((mouse.0 - pane.grid_origin[0] as f64) / cell[0] as f64).floor().max(0.0) as usize;
    let row = ((mouse.1 - pane.grid_origin[1] as f64) / cell[1] as f64).floor().max(0.0) as usize;
    Some((col.min(pane.cols - 1), row.min(pane.rows - 1)))
}
fn line(text: impl Into<String>) -> PanelLine { PanelLine { text: text.into(), selected: false, action: None } }
fn panel(id: &str, rect: PixelRect, title: impl Into<String>, lines: Vec<PanelLine>) -> PanelState {
    PanelState { id: id.into(), rect, title: title.into(), lines, snapshot: None,
        underlines: Vec::new(), opacity: 1.0, rise_px: 0.0 }
}

impl App {
    pub(super) fn layout_with(&self, server: &Tree) -> Result<look::Layout, String> {
        let m = self.metrics()?;
        let size = self.window.as_ref().ok_or("window not ready")?.inner_size();
        Ok(look::layout([size.width, size.height], self.scale, self.panels.visible,
            &expand(&self.files.layout, server), [m.cell_width, m.cell_height]))
    }
    pub(super) fn pixel_layout(&self) -> Result<look::Layout, String> {
        let empty = Tree::leaf("");
        self.pixel_layout_for(self.groups.get(self.active).map(|g| &g.layout).unwrap_or(&empty))
    }
    pub(super) fn pixel_layout_for(&self, tree: &Tree) -> Result<look::Layout,String> {
        let mut layout = self.layout_with(tree)?;
        // Reserve integral terminal rows for the dock, never resize PTYs to a guessed cell rectangle.
        if let Some(target) = self.compose.active_target() {
            if let Some(pane) = layout.panes.iter_mut().find(|p| p.id == target) {
                let reserve = (layout.metrics.compose_dock_height / layout.cell_px[1] as f64).ceil() as usize;
                let reserve = reserve.min(pane.rows.saturating_sub(3));
                pane.rows -= reserve;
                pane.grid_clip.h = pane.rows as f32 * layout.cell_px[1] as f32;
            }
        }
        Ok(layout)
    }
    pub(super) fn dock(&self, layout: &look::Layout) -> Option<(PixelRect, Rect)> {
        let pane = layout.pane(self.compose.active_target()?)?;
        let m = layout.metrics;
        let inset = m.pane_padding as f32 / 2.0;
        let y = pane.grid_origin[1] + pane.grid_clip.h + m.card_inset as f32;
        let rect = PixelRect { x: pane.rect.x + inset, y, w: (pane.rect.w - 2.0*inset).max(0.0),
            h: (pane.rect.y + pane.rect.h - inset - y).max(0.0) };
        if rect.h < m.compose_min_height as f32 { return None; }
        let cols = ((rect.w - 2.0*m.panel_padding as f32).max(0.0) / layout.cell_px[0] as f32).floor() as usize;
        let rows = ((rect.h - (2.0*m.panel_padding+m.panel_header) as f32).max(0.0) / layout.cell_px[1] as f32).floor() as usize;
        (rows > 0 && cols > 0).then_some((rect, Rect { row: 0, col: 0, rows, cols }))
    }
    fn dock_point(&self, layout: &look::Layout, clamp: bool) -> Option<(Rect, usize, usize)> {
        let (mut rect, grid) = self.dock(layout)?;
        let m = layout.metrics;
        let rise = self.studio.panels.get("compose").map_or(m.panel_rise as f32,|t|look::ease_out_cubic(m.panel_rise as f32,0.0,t.elapsed().as_secs_f32()*1000.0,m.panel_ms,self.config.animations));
        rect.y += if self.config.animations { rise } else { 0.0 };
        let x = rect.x + m.panel_padding as f32;
        let y = rect.y + (m.panel_padding+m.panel_header) as f32;
        let pane = PaneLayout { id: String::new(), rect, grid_origin: [x,y],
            grid_clip: PixelRect { x, y,
                w: grid.cols as f32 * layout.cell_px[0] as f32, h: grid.rows as f32 * layout.cell_px[1] as f32 }, rows: grid.rows, cols: grid.cols };
        let (col, row) = point(&pane, self.mouse, layout.cell_px, clamp)?;
        Some((grid, col, row))
    }
    fn refresh_studio(&mut self) {
        let now = Instant::now();
        if now < self.studio.next_metadata { return; }
        self.studio.next_metadata = now + Duration::from_millis(250);
        self.studio.metadata = if self.engine.is_closed() { Value::Null } else {
            self.engine.exchange(3, "look_state", &json!({})).unwrap_or(Value::Null)
        };
        let ids: Vec<_> = self.groups.get(self.active).map(|g| g.layout.leaves()).unwrap_or_default();
        self.studio.permissions.retain(|id, _| ids.contains(id));
        self.studio.blocks.retain(|id, _| ids.contains(id));
        for id in ids {
            match self.engine.agent_permission(&id) {
                Ok(allowed) => { self.studio.permissions.insert(id.clone(), allowed); }
                Err(_) => { self.studio.permissions.remove(&id); }
            }
            if let Ok(view) = self.engine.palette_action("view", &json!({"tab_id":id,"limit":64}), true) {
                self.studio.blocks.insert(id, view);
            } else { self.studio.blocks.remove(&id); }
        }
        // Server-owned minute buckets count successful agent run/rerun operations,
        // never output-event frequency or the timing of GUI polling.
        self.studio.activity = self.studio.metadata["activity"].as_array()
            .map(|values| values.iter().take(30).filter_map(Value::as_u64).map(|n| n as f32).collect())
            .unwrap_or_default();
        let focused = self.focused().map(str::to_owned);
        if !self.engine.is_closed() { self.panels.refresh(&self.engine, focused.as_deref()); }
    }
    fn activate_studio(&mut self, action: HitAction) -> Result<(), String> {
        match action {
            HitAction::Palette => {
                self.palette_editor = Some(self.files.focus.clone());
                let focused = self.focused().map(str::to_owned);
                self.panels.refresh(&self.engine, focused.as_deref());
                if !self.panels.open { self.panels.toggle(); }
                self.reset_ime();
            }
            HitAction::ToggleRails => { self.panels.visible = !self.panels.visible; self.studio.drag = None; self.resize_panes()?; }
            HitAction::Session(id) => {
                let index = self.groups.iter().position(|g| g.layout.contains(&id)).ok_or("Session was closed")?;
                self.active = index; self.files.focus_on(TERMINALS.into()); self.save_workspace()?;
                self.reset_ime(); self.resize_panes()?;
            }
            HitAction::File(id) => { if self.files.panes.contains_key(&id) { self.files.focus_on(id); self.reset_ime(); } }
            HitAction::Agent { tab_id, .. } => {
                if let Some(id) = tab_id {
                    let index = self.groups.iter().position(|g| g.layout.contains(&id)).ok_or("Agent terminal was closed")?;
                    self.active = index; self.groups[index].focus = id; self.files.focus_on(TERMINALS.into());
                    self.save_workspace()?; self.reset_ime(); self.resize_panes()?;
                } else { self.status = Some("This agent has no terminal target".into()); }
            }
            HitAction::Access(id) => {
                let old = self.engine.agent_permission(&id).map_err(|e| e.message)?;
                self.engine.set_agent_permission(&id, !old).map_err(|e| e.message)?;
                self.studio.permissions.insert(id, !old);
            }
            HitAction::PaletteChoice(index) => { self.panels.choose(&self.engine, index); self.local_request()?; self.reset_ime(); }
            HitAction::Block { tab_id, block_id, action } => {
                // Existing owner palette permissions and explicit selection semantics remain authoritative.
                self.panels.block_action(&self.engine, &tab_id, &block_id, &action)?;
                if self.panels.open { self.reset_ime(); }
            }
        }
        Ok(())
    }
    pub(super) fn click_studio(&mut self) -> Result<(), String> {
        self.files.dragging = None; self.studio.drag = None;
        let layout = self.pixel_layout()?;
        if self.compose.active_target().is_some() {
            self.compose_dragging = false;
            if let Some((rect, col, row)) = self.dock_point(&layout, false) {
                self.compose.click(&rect, col, row, self.modifiers.shift_key())?;
                self.compose_dragging = true;
            }
            return Ok(());
        }
        if self.files.modal() || self.search.active_target().is_some() { return Ok(()); }
        if let Some(action) = self.studio.hits.iter().rev().find(|h| h.rect.contains(self.mouse.0, self.mouse.1)).map(|h| h.action.clone()) {
            if !self.panels.open || matches!(action, HitAction::PaletteChoice(_)) { return self.activate_studio(action); }
        }
        if self.panels.open { return Ok(()); }
        if let Some(index) = layout.divider_at(self.mouse.0, self.mouse.1) {
            self.studio.drag = Some(Capture { layout, index, revision: self.revision, local: self.files.layout.clone() });
            return Ok(());
        }
        let Some(pane) = layout.pane_at(self.mouse.0, self.mouse.1) else { return Ok(()); };
        let id = pane.id.clone();
        if self.files.panes.contains_key(&id) {
            self.files.focus_on(id.clone()); self.reset_ime();
            let hit = self.studio.reviews.get(&id).filter(|r|r.content.contains(self.mouse.0,self.mouse.1)).map(|r| {
                (((self.mouse.0-r.content.x as f64)/layout.cell_px[0] as f64) as usize,
                 r.first_row+((self.mouse.1-r.content.y as f64)/layout.cell_px[1] as f64) as usize)
            }).or_else(||point(pane,self.mouse,layout.cell_px,false));
            if let Some((col,row)) = hit {
                self.files.panes.get_mut(&id).unwrap().click(&local_rect(pane), col, row, self.modifiers.shift_key())?;
                self.files.dragging = Some(id);
            }
            return Ok(());
        }
        self.files.focus_on(TERMINALS.into());
        if let Some(group) = self.groups.get_mut(self.active) { group.focus = id.clone(); }
        self.save_workspace()?; self.reset_ime();
        let Some((x,y)) = point(pane, self.mouse, layout.cell_px, false) else { return Ok(()); };
        let (snapshot,y) = if let Some((snapshot,scroll))=self.studio.shown.get(&id) {
            (snapshot.clone(),((self.mouse.1-pane.grid_origin[1] as f64+*scroll as f64)/layout.cell_px[1] as f64).floor().max(0.0) as usize)
        } else { (self.engine.snapshot(&id).map_err(|e|e.message)?,y) };
        let hit = window_tools::url_at(&snapshot, y, x);
        if window_tools::url_click(hit.as_ref(), self.modifiers.control_key(), |url| {
            self.url_opener.open(url).map_err(|_| "Could not open URL".to_owned())
        })? { return Ok(()); }
        if location_at(&snapshot, y, x).is_some() {
            if !self.local_files { return Err("Remote terminal file clicks are not supported".into()); }
            let cwd = self.cwd(Some(&id)).unwrap_or(None);
            let editor = EditorPane::from_click(&snapshot, y, x, cwd.as_deref())?;
            self.files.add(TERMINALS, editor, Axis::Columns, self.rect()?)?;
            self.reset_ime(); self.resize_panes()?;
        }
        Ok(())
    }
    pub(super) fn drag_studio(&mut self) -> Result<(), String> {
        if self.compose.active_target().is_some() {
            if self.compose_dragging {
                if let Some((rect, col, row)) = self.dock_point(&self.pixel_layout()?, true) { self.compose.drag(&rect, col, row)?; }
            }
            return Ok(());
        }
        if let Some(capture) = self.studio.drag.take() {
            self.sync_groups()?;
            if self.revision != capture.revision || self.files.layout != capture.local { return Ok(()); }
            let divider = &capture.layout.dividers[capture.index];
            let pointer = if divider.axis == Axis::Rows { self.mouse.1 } else { self.mouse.0 };
            let moved = look::drag_divider(&capture.layout, capture.index, pointer)?;
            let mut prefix = Vec::new();
            terminal_path(&self.files.layout, &mut prefix);
            if divider.path.starts_with(&prefix) {
                let group = self.groups.get_mut(self.active).ok_or("Session was closed")?;
                group.layout.set_ratio(&divider.path[prefix.len()..], moved.ratio)?;
                self.save_workspace()?;
                self.persist_layout()?;
            } else { self.files.layout.set_ratio(&divider.path, moved.ratio)?; }
            let mut capture = capture;
            capture.revision = self.revision;
            capture.local = self.files.layout.clone();
            // Keep the original pointer geometry for a stable absolute drag.
            self.studio.drag = Some(capture);
            return self.resize_panes();
        }
        if self.files.modal() || self.panels.open || self.search.active_target().is_some() { return Ok(()); }
        if let Some(id) = self.files.dragging.clone() {
            let layout = self.pixel_layout()?;
            if let Some(pane) = layout.pane(&id) {
                if let Some((col,row)) = point(pane, self.mouse, layout.cell_px, true) {
                    if let Some(editor) = self.files.panes.get_mut(&id) { editor.drag(&local_rect(pane), col, row)?; }
                }
            }
        }
        Ok(())
    }
    pub(super) fn scroll_studio(&mut self, delta: MouseScrollDelta) -> Result<(), String> {
        if self.files.modal() || self.search.active_target().is_some() { return Ok(()); }
        let layout = self.pixel_layout()?;
        let (columns, lines) = match delta {
            MouseScrollDelta::LineDelta(x,y) => (-x*3.0,-y*3.0),
            MouseScrollDelta::PixelDelta(p) => (-p.x as f32/layout.cell_px[0] as f32,-p.y as f32/layout.cell_px[1] as f32),
        };
        if self.panels.open { self.panels.scroll(lines.round() as i32); return Ok(()); }
        if self.compose.active_target().is_some() {
            if self.dock_point(&layout, false).is_some() {
                self.compose.scroll(lines.round() as i64, columns.round() as i64)?;
                if self.config.animations { if let Some(old)=self.studio.snapshots.get("compose") { self.studio.scrolls.insert("compose".into(),Scroll { old:old.clone(),start:Instant::now() }); } }
            }
            return Ok(());
        }
        let Some(pane) = layout.pane_at(self.mouse.0,self.mouse.1) else { return Ok(()); };
        let id = pane.id.clone();
        if self.files.panes.contains_key(&id) {
            let row = self.studio.reviews.get(&id).filter(|r|r.content.contains(self.mouse.0,self.mouse.1)).map(|r|r.first_row+((self.mouse.1-r.content.y as f64)/layout.cell_px[1] as f64) as usize)
                .unwrap_or_else(||point(pane,self.mouse,layout.cell_px,true).map_or(0,|(_,row)|row));
            self.files.scroll(&id, row, lines.round() as i32, columns.round() as i32)?;
        } else { self.engine.scroll(&id, -lines.round() as i32).map_err(|e| e.message)?; }
        if self.config.animations {
            if let Some(old) = self.studio.snapshots.get(&id) { self.studio.scrolls.insert(id, Scroll { old: old.clone(), start: Instant::now() }); }
        }
        Ok(())
    }

    pub(super) fn draw_studio(&mut self) -> Result<(), String> {
        if self.stop || self.is_suspended() { return Ok(()); }
        self.resize_panes()?;
        self.refresh_studio();
        let layout = self.pixel_layout()?;
        self.files.viewport = Some(layout.clone());
        let now = Instant::now();
        let mut state = LookState { theme: self.studio.theme.clone(), opacity: self.config.opacity, animations: self.config.animations,
            elapsed_ms: self.studio.started.elapsed().as_secs_f32()*1000.0,
            focused: if self.compose.active_target().is_some() || self.panels.open || self.files.modal() || self.search.active_target().is_some() { None }
                else if self.files.focus == TERMINALS { self.focused().map(str::to_owned) } else { Some(self.files.focus.clone()) },
            ..LookState::default() };
        state.uptime_seconds = self.studio.metadata["uptime_ms"].as_u64().unwrap_or(0)/1000;
        state.masking_ready = self.studio.metadata["masking_ready"].as_bool().unwrap_or(false);
        state.activity = self.studio.activity.clone();
        for (index, group) in self.groups.iter().enumerate() {
            if let Some(id) = group.layout.leaves().first() {
                state.sessions.push(look::SessionItem { id: id.clone(), title: format!("Session {} · {}", index+1, group.focus), active: index == self.active });
            }
        }
        for (id, editor) in &self.files.panes {
            state.files.push(look::FileItem { id: id.clone(), title: format!("{}{}", editor.path().display(), if editor.dirty() { " *" } else { "" }),
                pending: editor.pending_count(), active: self.files.focus == *id });
        }
        if let Some(agents) = self.studio.metadata["agents"].as_array() {
            for agent in agents {
                if let (Some(connection_id),Some(peer_pid)) = (agent["connection_id"].as_u64(),agent["peer_pid"].as_u64().and_then(|n|u32::try_from(n).ok())) {
                    state.agents.push(look::AgentItem { connection_id, peer_pid, tab_id: agent["tab_id"].as_str().map(str::to_owned), running: agent["running"].as_bool().unwrap_or(false) });
                }
            }
        } else { state.timeline.push(look::TimelineItem { label: "Metadata unavailable".into(), detail: "Agent peers, uptime and masking are unknown".into() }); }
        if let Some(events) = self.studio.metadata["events"].as_array() {
            for event in events.iter().rev().take(20) {
                state.timeline.push(look::TimelineItem { label: event["kind"].as_str().unwrap_or("unknown").into(),
                    detail: format!("{} {}",event["tab_id"].as_str().unwrap_or(""),event["command_id"].as_str().unwrap_or("")) });
            }
        }
        self.studio.reviews.clear();
        let mut ui_rows: Vec<(PixelRect,String,Option<String>)> = Vec::new();
        let mut ime = None;
        let mut scroll_clips=Vec::new();
        for pane in &layout.panes {
            if pane.rows == 0 || pane.cols == 0 { continue; }
            let mut snapshot;
            let mut title = pane.id.clone();
            let mut editor_rows = None;
            if self.files.panes.contains_key(&pane.id) {
                snapshot = cells(pane.rows, pane.cols, &state.theme);
                editor_rows = self.files.draw_pane(&pane.id, &mut snapshot, local_rect(pane));
                title = self.files.panes[&pane.id].path().display().to_string();
                // UI rows keep the editor controller's geometry; source and real diff previews stay mono.
                let mut rows = vec![0, pane.rows-1];
                if let Some(text_rows) = editor_rows {
                    let start = text_rows+1;
                    if start+2 < pane.rows-1 {
                        let m = layout.metrics;
                        let inset = m.card_inset as f32;
                        let y = pane.grid_origin[1]+start as f32*layout.cell_px[1] as f32;
                        let rect = PixelRect { x:pane.rect.x+inset,y,w:(pane.rect.w-2.0*inset).max(0.0),h:(pane.rect.y+pane.rect.h-inset-y).max(0.0) };
                        let content = PixelRect { x:rect.x+m.panel_padding as f32,y:rect.y+(m.panel_padding+m.panel_header) as f32,
                            w:(rect.w-2.0*m.panel_padding as f32).max(0.0),h:(rect.h-(2.0*m.panel_padding+m.panel_header) as f32).max(0.0) };
                        let cols = ((content.w/layout.cell_px[0] as f32).floor() as usize).min(pane.cols);
                        let count = ((content.h/layout.cell_px[1] as f32).floor() as usize).min(pane.rows-start-1);
                        let panel_id = format!("review-{}",pane.id);
                        let mut preview = cells(count,cols,&state.theme);
                        for r in 1..count { for c in 0..cols { preview.cells[r*cols+c]=snapshot.cells[(start+1+r)*pane.cols+c].clone(); } }
                        widget(&mut preview,&state.theme);
                        if count>1 {
                            let status=row_text(&snapshot,pane.rows-1);
                            ui_rows.push((PixelRect { x:content.x,y:content.y+(count-1) as f32*layout.cell_px[1] as f32,w:content.w,h:layout.cell_px[1] as f32 },status,Some(panel_id.clone())));
                            for cell in &mut preview.cells[(count-1)*cols..] { cell.ch=' '; }
                        }
                        for (col,label) in [(0,"[Accept]"),(9,"[Reject]"),(18,"[Accept all]"),(31,"[Reject all]"),(44,"[Prev]"),(51,"[Next]")] {
                            let x=content.x+col as f32*layout.cell_px[0] as f32;
                            if x<content.x+content.w { ui_rows.push((PixelRect { x,y:content.y,w:(label.len() as f32*layout.cell_px[0] as f32).min(content.x+content.w-x),h:layout.cell_px[1] as f32 },label.into(),Some(panel_id.clone()))); }
                        }
                        self.studio.reviews.insert(pane.id.clone(),Review { content,first_row:start+1 });
                        let mut card=panel(&panel_id,rect,row_text(&snapshot,start),Vec::new());
                        card.snapshot=Some(preview); state.panels.push(card);
                        for cell in &mut snapshot.cells[start*pane.cols..] { cell.ch=' '; }
                        rows.retain(|r|*r<start);
                    }
                }
                for row in rows {
                    let text = row_text(&snapshot,row);
                    ui_rows.push((PixelRect { x: pane.grid_origin[0], y: pane.grid_origin[1]+row as f32*layout.cell_px[1] as f32,
                        w: pane.grid_clip.w, h: layout.cell_px[1] as f32 }, text, None));
                    for cell in &mut snapshot.cells[row*pane.cols..(row+1)*pane.cols] { cell.ch = ' '; }
                }
                widget(&mut snapshot,&state.theme);
            } else {
                snapshot = match self.engine.snapshot(&pane.id) { Ok(s) => s, Err(e) if e.code == "not_found" => continue, Err(e) => return Err(e.message) };
            }
            if self.search.active_target() == Some(pane.id.as_str()) {
                let mut search = cells(pane.rows+1,pane.cols,&state.theme);
                self.search.draw(&mut search,&Rect { row: 1,col: 0,rows: pane.rows,cols:pane.cols });
                let prompt = row_text(&search,0);
                snapshot = cells(pane.rows,pane.cols,&state.theme);
                snapshot.cells.clone_from_slice(&search.cells[pane.cols..]);
                let mut card = panel("search", modal_rect(&layout,4), "Search retained terminal text", vec![line(prompt)]);
                card.lines.push(line("Snapshot is pinned to the captured terminal · Escape closes"));
                state.panels.push(card);
            }
            let mut view = PaneState::new(pane.id.clone(),snapshot.clone());
            view.title = title;
            view.allow_agent = self.studio.permissions.get(&pane.id).copied().unwrap_or(false);
            view.driver = if self.files.panes.contains_key(&pane.id) { "Local editor".into() } else { "Driver unknown".into() };
            if let Some(drivers) = self.studio.metadata["drivers"].as_array().filter(|_|!self.files.panes.contains_key(&pane.id)) {
                if let Some(driver) = drivers.iter().find(|d| d["tab_id"].as_str() == Some(pane.id.as_str()) && d["running"] == true) {
                    view.agent_running = true;
                    view.driver = format!("Agent {} · PID {}",driver["connection_id"],driver["peer_pid"]);
                } else if !self.studio.metadata.is_null() { view.driver = "Terminal".into(); }
            }
            if state.focused.as_deref() == Some(pane.id.as_str()) {
                if let Some((row,col)) = snapshot.cursor.filter(|(r,c)| *r<pane.rows && *c<pane.cols) {
                    let to = [pane.grid_origin[0]+col as f32*layout.cell_px[0] as f32,pane.grid_origin[1]+row as f32*layout.cell_px[1] as f32];
                    let cursor = self.studio.cursors.entry(pane.id.clone()).or_insert(Cursor { from: to,to,start:now });
                    if cursor.to != to {
                        let elapsed = cursor.start.elapsed().as_secs_f32()*1000.0;
                        cursor.from = [look::ease_out_cubic(cursor.from[0],cursor.to[0],elapsed,layout.metrics.cursor_ms,self.config.animations),look::ease_out_cubic(cursor.from[1],cursor.to[1],elapsed,layout.metrics.cursor_ms,self.config.animations)];
                        cursor.to = to; cursor.start = now;
                    }
                    view.cursor = Some(CursorMotion { from:cursor.from,to:cursor.to,elapsed_ms:cursor.start.elapsed().as_secs_f32()*1000.0 });
                    ime = Some(to);
                } else { self.studio.cursors.remove(&pane.id); }
            } else { self.studio.cursors.remove(&pane.id); }
            if !self.panels.open && !self.files.modal() && self.compose.active_target().is_none() && self.search.active_target().is_none() {
                if let Some((col,row)) = point(pane,self.mouse,layout.cell_px,false) {
                    if let Some(hit) = window_tools::url_at(&snapshot,row,col) { view.underlines.push(Underline { row:hit.row,start_col:hit.start_col,end_col:hit.end_col.min(pane.cols),color:rgb(state.theme.link) }); }
                }
            }
            if editor_rows.is_none() && self.search.active_target().is_none() {
                if let Some(blocks) = self.studio.blocks.get(&pane.id) { view.blocks = matched_blocks(&snapshot,blocks); }
            }
            self.studio.snapshots.insert(pane.id.clone(),snapshot.clone());
            if let Some(scroll) = self.studio.scrolls.get(&pane.id) {
                let elapsed = scroll.start.elapsed().as_secs_f32()*1000.0;
                if self.config.animations && elapsed < layout.metrics.scroll_ms {
                    if let Some((stitched, from, to)) = stitch_region(&scroll.old,&snapshot,editor_rows.map_or(0,|_|1),editor_rows.map_or(snapshot.rows,|n|n+1),layout.cell_px[1] as f32) {
                        view.snapshot = stitched; view.scroll_px = look::ease_out_cubic(from,to,elapsed,layout.metrics.scroll_ms,true);
                        if let Some(rows)=editor_rows { scroll_clips.push(([pane.grid_origin[0],pane.grid_origin[1]-view.scroll_px],PixelRect { x:pane.grid_clip.x,y:pane.grid_clip.y+layout.cell_px[1] as f32,w:pane.grid_clip.w,h:rows as f32*layout.cell_px[1] as f32 })); }
                        view.blocks.clear(); view.underlines.clear(); view.cursor = None; view.snapshot.cursor = None;
                    }
                }
            }
            self.studio.shown.insert(pane.id.clone(),(view.snapshot.clone(),view.scroll_px));
            view.snapshot.images.retain(|image| {
                let left=image.x.max(image.clip_x); let right=image.x.saturating_add(image.cols).min(image.clip_x.saturating_add(image.clip_cols));
                let top=image.y.max(image.clip_y); let bottom=image.y.saturating_add(image.rows).min(image.clip_y.saturating_add(image.clip_rows));
                !view.underlines.iter().any(|u|left<right && top<bottom && left<u.end_col && right>u.start_col && top<=u.row && bottom>u.row)
            });
            state.panes.push(view);
        }
        self.studio.scrolls.retain(|_,s| self.config.animations && s.start.elapsed().as_secs_f32()*1000.0<layout.metrics.scroll_ms);
        self.studio.snapshots.retain(|id,_| layout.pane(id).is_some() || (id=="compose" && self.compose.active_target().is_some()));
        self.studio.shown.retain(|id,_| layout.pane(id).is_some());
        self.studio.cursors.retain(|id,_| layout.pane(id).is_some() || (id == "compose" && self.compose.active_target().is_some()));
        if let Some(card) = self.panels.card(modal_rect(&layout,24),&layout.metrics) { state.panels.push(card); }
        if let Some(card) = self.files.prompt_card(modal_rect(&layout,5),&layout.metrics) { state.panels.push(card); }
        if let Some((rect, grid)) = self.dock(&layout) {
            let mut snapshot = cells(grid.rows,grid.cols,&state.theme);
            let mut underlines = self.compose.draw(&mut snapshot,&grid);
            for row in [0,grid.rows.saturating_sub(2),grid.rows-1] {
                let text=row_text(&snapshot,row);
                ui_rows.push((PixelRect { x:rect.x+layout.metrics.panel_padding as f32,y:rect.y+(layout.metrics.panel_padding+layout.metrics.panel_header) as f32+row as f32*layout.cell_px[1] as f32,
                    w:(rect.w-2.0*layout.metrics.panel_padding as f32).max(0.0),h:layout.cell_px[1] as f32 },text,Some("compose".into())));
                for cell in &mut snapshot.cells[row*grid.cols..(row+1)*grid.cols] { cell.ch=' '; }
            }
            for underline in &mut underlines { underline.color = rgb(state.theme.spelling); }
            widget(&mut snapshot,&state.theme);
            if let Some((row,col)) = snapshot.cursor { ime = Some([rect.x+layout.metrics.panel_padding as f32+col as f32*layout.cell_px[0] as f32,rect.y+(layout.metrics.panel_padding+layout.metrics.panel_header) as f32+row as f32*layout.cell_px[1] as f32]); }
            state.panels.push(PanelState { id:"compose".into(),rect,title:format!("Compose → {} · memory-only",self.compose.active_target().unwrap_or("")),lines:Vec::new(),snapshot:Some(snapshot),underlines,opacity:1.0,rise_px:0.0 });
        } else if self.compose.active_target().is_some() {
            state.panels.push(panel("compose-hidden",modal_rect(&layout,4),"Compose target is not visible",vec![line("Draft retained. Escape closes; Send never retargets another terminal.")]));
        }
        // A card's clock survives redraws but not closure/reopening.
        let active: Vec<_> = state.panels.iter().map(|p|p.id.clone()).collect();
        self.studio.panels.retain(|id,_| active.contains(id));
        for panel in &mut state.panels {
            let start = self.studio.panels.entry(panel.id.clone()).or_insert(now);
            panel.opacity = look::ease_out_cubic(0.0,1.0,start.elapsed().as_secs_f32()*1000.0,layout.metrics.panel_ms,self.config.animations);
            panel.rise_px = layout.metrics.panel_rise as f32*(1.0-panel.opacity);
            if panel.id == "compose" || panel.id.starts_with("review-") { panel.rect.h = (panel.rect.h-panel.rise_px).max(0.0); }
            if let Some(id)=panel.id.strip_prefix("review-") {
                if let Some(review)=self.studio.reviews.get_mut(id) { review.content.y+=panel.rise_px; review.content.h=(review.content.h-panel.rise_px).max(0.0); }
            }
        }
        let mut compose_cursor = None;
        let mut compose_animation=None;
        if let Some(card) = state.panels.iter().find(|p| p.id == "compose") {
            if let Some((row,col)) = card.snapshot.as_ref().and_then(|s|s.cursor) {
                let m = layout.metrics;
                let to = [card.rect.x+m.panel_padding as f32+col as f32*layout.cell_px[0] as f32,
                    card.rect.y+card.rise_px+(m.panel_padding+m.panel_header) as f32+row as f32*layout.cell_px[1] as f32];
                ime = Some(to);
                let c = self.studio.cursors.entry("compose".into()).or_insert(Cursor { from:to,to,start:now });
                if c.to != to {
                    let elapsed = c.start.elapsed().as_secs_f32()*1000.0;
                    c.from = [look::ease_out_cubic(c.from[0],c.to[0],elapsed,m.cursor_ms,self.config.animations),look::ease_out_cubic(c.from[1],c.to[1],elapsed,m.cursor_ms,self.config.animations)];
                    c.to=to; c.start=now;
                }
                let elapsed = c.start.elapsed().as_secs_f32()*1000.0;
                let x=look::ease_out_cubic(c.from[0],c.to[0],elapsed,m.cursor_ms,self.config.animations);
                let y=look::ease_out_cubic(c.from[1],c.to[1],elapsed,m.cursor_ms,self.config.animations)+layout.cell_px[1] as f32-m.cursor_inset_y as f32;
                let bottom=card.rect.y+card.rise_px+card.rect.h-m.panel_padding as f32;
                if y>=card.rect.y && y+m.cursor_width as f32<=bottom {
                    compose_cursor=Some(Shape { rect:[x,y,layout.cell_px[0] as f32,m.cursor_width as f32],radius:m.cursor_radius as f32,fill:Fill::Solid(look::Theme::fade(state.theme.accent,card.opacity)),border:None,shadow:None });
                }
            }
        }
        if let Some(card)=state.panels.iter().find(|p|p.id=="compose") {
            if let Some(snapshot)=&card.snapshot {
                if let Some(scroll)=self.studio.scrolls.get("compose") {
                    let elapsed=scroll.start.elapsed().as_secs_f32()*1000.0;
                    let mut end=snapshot.rows;
                    if snapshot.rows==scroll.old.rows && snapshot.cols==scroll.old.cols {
                        while end>1 && snapshot.cells[(end-1)*snapshot.cols..end*snapshot.cols].iter().zip(&scroll.old.cells[(end-1)*snapshot.cols..end*snapshot.cols]).all(|(a,b)|a.ch==b.ch && a.fg==b.fg && a.bg==b.bg) { end-=1; }
                        if elapsed<layout.metrics.scroll_ms {
                            if let Some((stitched,from,to))=stitch_region(&scroll.old,snapshot,1,end,layout.cell_px[1] as f32) {
                                let offset=look::ease_out_cubic(from,to,elapsed,layout.metrics.scroll_ms,self.config.animations);
                                let origin=[card.rect.x+layout.metrics.panel_padding as f32,card.rect.y+card.rise_px+(layout.metrics.panel_padding+layout.metrics.panel_header) as f32];
                                let clip=PixelRect { x:origin[0],y:origin[1]+layout.cell_px[1] as f32,w:(card.rect.w-2.0*layout.metrics.panel_padding as f32).max(0.0),h:end.saturating_sub(1) as f32*layout.cell_px[1] as f32 };
                                compose_animation=Some((origin,offset,clip,stitched,card.opacity)); compose_cursor=None;
                            }
                        }
                    }
                }
                self.studio.snapshots.insert("compose".into(),snapshot.clone());
            }
        }
        let mut presentation = look::presentation(&layout,&state);
        if let Some((origin,offset,clip,stitched,opacity))=compose_animation {
            for layer in &mut presentation.scene.layers {
                if let Layer::Grid(grid)=layer { if grid.origin==origin {
                    let start=1; let end=start+(clip.h/layout.cell_px[1] as f32) as usize;
                    for cell in &mut grid.snapshot.cells[start*grid.snapshot.cols..end*grid.snapshot.cols] { cell.ch=' '; }
                    grid.underlines.clear();
                } }
            }
            presentation.scene.layers.push(Layer::Grid(GridLayer { origin:[origin[0],origin[1]-offset],clip:clip.array(),snapshot:stitched,underlines:Vec::new(),opacity }));
        }
        for layer in &mut presentation.scene.layers {
            if let Layer::Grid(grid)=layer {
                if let Some((_,clip))=scroll_clips.iter().find(|(origin,_)|*origin==grid.origin) { grid.clip=clip.array(); }
            }
        }
        // The generic builder accepts terminal fixtures. Remove terminal-only controls
        // from local editor cards using its exact shared hit geometry, not guessed pixels.
        let mut remove=Vec::new();
        let mut unknown=Vec::new();
        presentation.hits.retain(|hit| {
            if let HitAction::Access(id)=&hit.action {
                if self.files.panes.contains_key(id) {
                    let rect=layout.right_rail.filter(|rail|rail.contains(hit.rect.x as f64,hit.rect.y as f64))
                        .map(|rail|PixelRect { x:rail.x,y:hit.rect.y,w:rail.w,h:hit.rect.h }).unwrap_or(hit.rect);
                    remove.push(rect); return false;
                }
                if !self.studio.permissions.contains_key(id) { unknown.push(hit.rect); }
            }
            true
        });
        presentation.scene.layers.retain(|layer| match layer {
            Layer::Shape(shape)=>!remove.iter().any(|r|shape.rect[0]>=r.x && shape.rect[1]>=r.y && shape.rect[0]+shape.rect[2]<=r.x+r.w && shape.rect[1]+shape.rect[3]<=r.y+r.h),
            Layer::Text(text)=>!remove.iter().any(|r|r.contains(text.origin[0] as f64,text.origin[1] as f64)),
            _=>true,
        });
        for layer in &mut presentation.scene.layers {
            if let Layer::Text(text)=layer {
                if self.studio.metadata.is_null() && text.text.starts_with("Up ") && layout.top_bar.contains(text.origin[0] as f64,text.origin[1] as f64) { text.text="Uptime unknown".into(); }
                if unknown.iter().any(|r|r.contains(text.origin[0] as f64,text.origin[1] as f64)) { text.text="Unknown".into(); text.color=state.theme.dim; }
            }
        }
        if let Some(cursor) = compose_cursor { presentation.scene.layers.push(Layer::Shape(cursor)); }
        // Editor UI labels are at exactly the same row addresses as their keyboard/mouse controller.
        for (rect,text,card_id) in ui_rows {
            if self.panels.open || self.files.modal() || self.search.active_target().is_some() { continue; }
            let card=card_id.as_ref().and_then(|id|state.panels.iter().find(|p|p.id==*id));
            let rise=card.map_or(0.0,|p|p.rise_px);
            let opacity=card.map_or(1.0,|p|p.opacity);
            let px=if card_id.as_ref().is_some_and(|id|id.starts_with("review-")) { (layout.metrics.small_font_px as f32).min(layout.cell_px[0] as f32) } else { layout.metrics.ui_font_px as f32 };
            if !text.is_empty() { presentation.scene.layers.push(Layer::Text(TextRun { origin:[rect.x,rect.y+rise],text:truncate(&text,rect.w/px),px,color:state.theme.ink,weight:400,mono:false,opacity })); }
        }
        if let Some(status) = &self.status {
            presentation.scene.layers.push(Layer::Text(TextRun { origin:[layout.metrics.outer_gap as f32,layout.window_px[1] as f32-layout.metrics.panel_line_height as f32],text:truncate(status,(layout.window_px[0] as f32-2.0*layout.metrics.outer_gap as f32)/layout.metrics.small_font_px as f32),px:layout.metrics.small_font_px as f32,color:state.theme.accent,weight:500,mono:false,opacity:1.0 }));
        }
        self.studio.hits = presentation.hits;
        if let Some(window) = &self.window {
            window.set_title(&format!("Clanky — {} — {:.0} pt",if self.files.focus == TERMINALS { self.focused().unwrap_or("no terminal") } else { &self.files.focus },self.zoom.points()));
            if let Some([x,y]) = ime { window.set_ime_cursor_area(PhysicalPosition::new(x.max(0.0) as u32,y.max(0.0) as u32),PhysicalSize::new(layout.cell_px[0],layout.cell_px[1])); }
        }
        self.renderer.as_mut().ok_or("renderer not ready")?.draw_scene(&presentation.scene)
    }
}
fn truncate(text: &str, chars: f32) -> String {
    let mut budget = chars.max(0.0) as usize;
    text.chars().map(|c|if c.is_control(){' '}else{c}).take_while(|c| {
        let cost = if c.is_ascii() { 1 } else { 2 };
        if cost > budget { false } else { budget -= cost; true }
    }).collect()
}
fn modal_rect(layout: &look::Layout, lines: usize) -> PixelRect {
    let m = layout.metrics;
    let w = (m.panel_max_width as f32).min((layout.window_px[0] as f32-2.0*m.panel_padding as f32).max(0.0));
    let h = ((2.0*m.panel_padding+m.panel_header+lines as f64*m.panel_line_height).min(m.panel_max_height) as f32)
        .min((layout.window_px[1] as f32-m.top_bar as f32-2.0*m.outer_gap as f32).max(0.0));
    PixelRect { x:(layout.window_px[0] as f32-w)/2.0,y:(m.top_bar+m.outer_gap) as f32,w,h }
}

/// Only plain, complete, uniquely matching command+output sequences get live cards.
/// Raw controls, wrapping, truncation and ambiguous matches require stable server row metadata.
fn matched_blocks(snapshot: &TerminalSnapshot, view: &Value) -> Vec<look::BlockCard> {
    let mut cards = Vec::new();
    if !snapshot.images.is_empty() || snapshot.cols == 0 { return cards; }
    let rows: Vec<_> = (0..snapshot.rows).map(|r|row_text(snapshot,r)).collect();
    let Some(blocks) = view["blocks"].as_array() else { return cards; };
    for block in blocks {
        if block["complete"] != true || block["truncated"] == true { continue; }
        let (Some(id),Some(command),Some(output)) = (block["block_id"].as_str(),block["command"].as_str(),block["output"].as_str()) else { continue; };
        if command.is_empty() || command.chars().count() > snapshot.cols.saturating_sub(52) || output.is_empty() || command.chars().any(char::is_control)
            || output.chars().any(|c| c.is_control() && c!='\n') { continue; }
        let mut expected = vec![command];
        expected.extend(output.trim_end_matches('\n').split('\n'));
        if expected.iter().any(|s|s.is_empty() || s.chars().count()>snapshot.cols) || expected.len()>rows.len() { continue; }
        let matches: Vec<_> = rows.windows(expected.len()).enumerate().filter(|(_,r)|r.iter().map(String::as_str).eq(expected.iter().copied())).map(|(i,_)|i).collect();
        if let [first] = matches.as_slice() {
            let last = first+expected.len()-1;
            if cards.iter().any(|c: &look::BlockCard|*first<=c.last_row && last>=c.first_row) { continue; }
            cards.push(look::BlockCard { id:id.into(),first_row:*first,last_row:last,command:command.into(),exit_code:block["exit_code"].as_i64().and_then(|n|i32::try_from(n).ok()),duration_ms:block["duration_ms"].as_u64(),running:false });
        }
    }
    cards
}

/// Keep widget chrome stationary while stitching only its actual source rows.
fn stitch_region(old: &TerminalSnapshot,new: &TerminalSnapshot,start: usize,end: usize,cell_h: f32) -> Option<(TerminalSnapshot,f32,f32)> {
    if old.cols!=new.cols || old.rows!=new.rows || start>=end || end>new.rows { return None; }
    if start==0 && end==new.rows { return stitch(old,new,cell_h); }
    let region=|s:&TerminalSnapshot|TerminalSnapshot { rows:end-start,cols:s.cols,cells:s.cells[start*s.cols..end*s.cols].to_vec(),cursor:None,images:Vec::new() };
    let (middle,from,to)=stitch(&region(old),&region(new),cell_h)?;
    let mut result=new.clone(); result.cursor=None; result.images.clear(); result.rows+=middle.rows-(end-start);
    result.cells=new.cells[..start*new.cols].iter().chain(&middle.cells).chain(&new.cells[end*new.cols..]).cloned().collect();
    Some((result,from,to))
}
/// Stitch only verified overlapping rows. Unknown/repainted screens jump rather than invent text.
fn stitch(old: &TerminalSnapshot, new: &TerminalSnapshot, cell_h: f32) -> Option<(TerminalSnapshot,f32,f32)> {
    if old.rows != new.rows || old.cols != new.cols || old.rows<2 || !old.images.is_empty() || !new.images.is_empty() { return None; }
    let same = |a: &[Cell],b: &[Cell]| a.iter().zip(b).all(|(a,b)|a.ch==b.ch && a.fg==b.fg && a.bg==b.bg);
    if same(&old.cells,&new.cells) { return None; }
    for shift in 1..old.rows.min(12) {
        let n = shift*old.cols;
        if same(&old.cells[n..],&new.cells[..new.cells.len()-n]) {
            let mut out = new.clone(); out.rows += shift; out.cursor=None;
            out.cells = old.cells[..n].iter().chain(&new.cells).cloned().collect();
            return Some((out,0.0,shift as f32*cell_h));
        }
        if same(&new.cells[n..],&old.cells[..old.cells.len()-n]) {
            let mut out = new.clone(); out.rows += shift; out.cursor=None;
            out.cells = new.cells.iter().chain(&old.cells[old.cells.len()-n..]).cloned().collect();
            return Some((out,shift as f32*cell_h,0.0));
        }
    }
    None
}
