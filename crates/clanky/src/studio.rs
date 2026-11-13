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
    pub(crate) theme: look::Theme,
    calm_theme: clanky::themes::Theme,
    pub(super) terminal_info:BTreeMap<String,Value>,
    user_name:Option<String>,
    pane_labels:BTreeMap<String,String>,
    seen_files:std::collections::BTreeSet<String>,
    local_recent:std::collections::VecDeque<(Instant,look::TimelineItem)>,
    scene_cache:look::SceneCache,
    title:Option<String>,
    ime:Option<([u32;2],[u32;2])>,
    hits: Vec<Hotspot>,
    metadata: Value,
    pub(super) permissions: BTreeMap<String, bool>,
    blocks: BTreeMap<String, Value>,
    next_metadata: Instant,
    cursors: BTreeMap<String, Cursor>,
    card_clocks: BTreeMap<(String,String),Instant>,
    card_keys: Vec<(String,String)>,
    card_focus: Option<(String,String)>,
    card_action: usize,
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
        Self { drag: None, theme: look::Theme::default(), calm_theme:clanky::themes::builtin("stone-dark").expect("bundled Stone theme"), terminal_info:BTreeMap::new(), user_name:None, pane_labels:BTreeMap::new(),seen_files:Default::default(),local_recent:Default::default(), scene_cache:look::SceneCache::default(), title:None,ime:None,hits: Vec::new(), metadata: Value::Null, permissions: BTreeMap::new(),
            blocks: BTreeMap::new(), next_metadata: Instant::now(), cursors: BTreeMap::new(),
            card_clocks:BTreeMap::new(),card_keys:vec![],card_focus:None,card_action:0,
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
/// Rows the editor core reserves for its own title; the window does not show them.
const EDITOR_TITLE_ROWS: usize = 1;
fn widget(snapshot: &mut TerminalSnapshot, theme: &look::Theme) {
    for cell in &mut snapshot.cells { cell.fg = theme.ui_color(cell.fg,false); cell.bg = theme.ui_color(cell.bg,true); cell.color_source = None; }
}
fn cells(rows: usize, cols: usize, theme: &look::Theme) -> TerminalSnapshot {
    TerminalSnapshot { rows, cols, cells: vec![Cell { ch: ' ', fg: rgb(theme.ink), bg: rgb(theme.panel), color_source: None }; rows * cols],
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
        Ok(clanky::calm::layout([size.width, size.height], self.scale, self.panels.visible,self.applied_post_ai,
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
    pub(super) fn refresh_studio(&mut self) -> bool {
        let now = Instant::now();
        if now < self.studio.next_metadata { return false; }
        self.studio.next_metadata = now + Duration::from_secs(1);
        let stable=|value:&Value| {let mut value=value.clone();
            if let Some(object)=value.as_object_mut(){object.remove("uptime_ms");object.remove("events");}
            if let Some(events)=value.get_mut("recent").and_then(Value::as_array_mut){for event in events{if let Some(object)=event.as_object_mut(){object.remove("age_ms");}}}
            value
        };
        let before=stable(&self.studio.metadata);
        let old_permissions=self.studio.permissions.clone();
        let old_info=self.studio.terminal_info.clone();
        self.studio.metadata = if self.engine.is_closed() { Value::Null } else {
            self.engine.exchange(3, "look_state", &json!({})).unwrap_or(Value::Null)
        };
        let ids: Vec<_> = self.groups.get(self.active).map(|g| g.layout.leaves()).unwrap_or_default();
        self.studio.permissions.retain(|id, _| ids.contains(id));
        self.studio.blocks.retain(|id, _| ids.contains(id));
        let all:Vec<_>=self.groups.iter().flat_map(|group|group.layout.leaves()).collect();
        self.studio.terminal_info.retain(|id,_|all.contains(id));
        for id in ids {
            if let Ok(info)=self.engine.exchange(3,"terminal_info",&json!({"tab_id":id})){self.studio.terminal_info.insert(id.clone(),info);}
            match self.engine.agent_permission(&id) {
                Ok(allowed) => { self.studio.permissions.insert(id.clone(), allowed); }
                Err(_) => { self.studio.permissions.remove(&id); }
            }
        }
        // Server-owned minute buckets count successful agent run/rerun operations,
        // never output-event frequency or the timing of GUI polling.
        self.studio.activity = self.studio.metadata["activity"].as_array()
            .map(|values| values.iter().take(30).filter_map(Value::as_u64).map(|n| n as f32).collect())
            .unwrap_or_default();
        before!=stable(&self.studio.metadata) || old_permissions!=self.studio.permissions || old_info!=self.studio.terminal_info
    }
    fn animation_deadline(&self,state:&LookState)->Option<Instant>{
        if !self.config.animations{return None;}
        let now=Instant::now();let m=look::Metrics::DEFAULT;
        let mut deadlines=Vec::new();
        for (id,cursor) in &self.studio.cursors {
            let visible=id=="compose" && self.compose.active_target().is_some();
            if visible && cursor.from!=cursor.to{deadlines.push(cursor.start+Duration::from_secs_f32(m.cursor_ms/1000.0));}
        }
        deadlines.extend(self.studio.scrolls.values().map(|scroll|scroll.start+Duration::from_secs_f32(m.scroll_ms/1000.0)));
        deadlines.extend(self.studio.panels.values().map(|start|*start+Duration::from_secs_f32(m.panel_ms/1000.0)));
        deadlines.extend(self.studio.card_clocks.values().map(|start|*start+Duration::from_millis(120)));
        if let Some(end)=self.settings.as_ref().and_then(|panel|panel.animation_deadline()){deadlines.push(end);}
        if state.panes.iter().any(|pane|pane.blocks.iter().any(|block|block.running && block.first_row<pane.snapshot.rows)){
            deadlines.push(now+clanky::frame_clock::FRAME_INTERVAL*2);
        }
        deadlines.into_iter().filter(|end|*end>now).max()
    }
    fn update_card_actions(&mut self,layout:&look::Layout,state:&mut LookState,now:Instant) {
        self.studio.card_keys.clear();
        let modal=self.settings_open() || self.panels.open || self.files.modal() || self.compose.active_target().is_some() || self.search.active_target().is_some();
        let mut shown=Vec::new();
        for pane in &layout.panes {
            let Some(data)=state.panes.iter().find(|data|data.id==pane.id) else{continue;};
            for block in &data.blocks {
                let origin=[pane.grid_origin[0],pane.grid_origin[1]-data.scroll_px];
                let Some(rect)=look::block_card_rect(layout,pane,block,data.snapshot.rows,origin) else{continue;};
                let key=(data.id.clone(),block.id.clone());
                self.studio.card_keys.push(key.clone());
                if !modal && (rect.contains(self.mouse.0,self.mouse.1) || self.studio.card_focus.as_ref()==Some(&key)) {
                    let start=self.studio.card_clocks.entry(key.clone()).or_insert(now);
                    let opacity=look::ease_out_cubic(0.0,1.0,start.elapsed().as_secs_f32()*1000.0,120.0,self.config.animations);
                    state.block_actions.insert(key.clone(),opacity);
                    shown.push(key);
                }
            }
        }
        self.studio.card_clocks.retain(|key,_|shown.contains(key));
        if self.studio.card_focus.as_ref().is_some_and(|key|!self.studio.card_keys.contains(key)) || modal {self.studio.card_focus=None;}
        state.block_action_focus=self.studio.card_focus.as_ref().map(|(tab,id)|(tab.clone(),id.clone(),self.studio.card_action));
    }
    pub fn settings_button_hit(&self)->bool {
        self.studio.hits.iter().any(|hit|hit.action==HitAction::Settings && hit.rect.contains(self.mouse.0,self.mouse.1))
    }
    pub(super) fn reset_ime_position(&mut self){self.studio.ime=None;}
    pub(super) fn invalidate_scene_fonts(&mut self){self.studio.scene_cache.clear();}
    pub fn clear_card_focus(&mut self) {self.studio.card_focus=None;}
    pub fn card_focused(&self)->bool {self.studio.card_focus.is_some()}
    pub fn card_key(&mut self,key:&Key)->Result<bool,String> {
        if self.panels.open || self.files.modal() || self.compose.active_target().is_some() || self.search.active_target().is_some() {return Ok(false);}
        if *key==Key::Named(NamedKey::F6) {
            if !self.studio.card_keys.is_empty() {
                let next=self.studio.card_focus.as_ref().and_then(|key|self.studio.card_keys.iter().position(|k|k==key)).map_or(0,|i|(i+1)%self.studio.card_keys.len());
                self.studio.card_focus=Some(self.studio.card_keys[next].clone());self.studio.card_action=0;self.reset_ime();
            }
            return Ok(true);
        }
        let Some((tab,id))=self.studio.card_focus.clone() else{return Ok(false);};
        let count=if self.applied_post_ai{2}else{3};
        match key {
            Key::Named(NamedKey::Escape)=>{self.studio.card_focus=None;self.reset_ime();},
            Key::Named(NamedKey::ArrowLeft)=>self.studio.card_action=(self.studio.card_action+count-1)%count,
            Key::Named(NamedKey::ArrowRight)|Key::Named(NamedKey::Tab)=>self.studio.card_action=(self.studio.card_action+1)%count,
            Key::Named(NamedKey::Enter)=>{let action=["copy","rerun","send"][self.studio.card_action%count];self.card_action(&tab,&id,action)?;},
            _=>(),
        }
        Ok(true)
    }
    /// Card actions from the Command card sheet: Copy, Rerun, Ask agent.
    fn card_action(&mut self, tab: &str, block: &str, action: &str) -> Result<(), String> {
        match action {
            // Copy keeps the owner palette's masked selection path.
            "copy" => { self.panels.block_action(&self.engine, tab, block, "copy")?; self.status = Some("Output copied".into()); }
            "rerun" => {
                let command = self.studio.blocks.get(tab).and_then(|view| view["blocks"].as_array())
                    .and_then(|blocks| blocks.iter().find(|b| b["block_id"].as_str() == Some(block)))
                    .and_then(|b| b["command"].as_str()).ok_or("This command is no longer on screen")?.to_owned();
                if command.trim().is_empty() || command.chars().any(|c| c.is_control()) { return Err("This command cannot be typed again".into()); }
                self.engine.input(tab, format!("{command}\r").as_bytes()).map_err(|e| e.message)?;
                self.sent_input = true;
            }
            "send" if !self.applied_post_ai => {
                self.panels.send_block(&self.engine, tab, block, "Look at this command and its output")?;
                self.status = Some("Sent to the agent queue".into());
            }
            _ => return Err("Unknown card action".into()),
        }
        Ok(())
    }
    pub(super) fn open_palette(&mut self) {
        if self.settings_open() { let _ = self.toggle_settings(); }
        self.palette.show();
    }
    /// The commands a person can run from here, named as the sidebar names them.
    pub(super) fn palette_context(&self) -> clanky::calm::palette::Context {
        let sessions = self.groups.iter().enumerate().filter_map(|(index, group)| {
            let id = group.layout.leaves().first()?.clone();
            let title = self.studio.terminal_info.get(&group.focus).and_then(|info| info["cwd"].as_str())
                .and_then(|cwd| std::path::Path::new(cwd).file_name()).map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| format!("Session {}", index + 1));
            Some((id, title, index == self.active))
        }).collect();
        let terminal = (self.files.focus == TERMINALS).then(|| self.focused()).flatten()
            .map(|id| self.studio.pane_labels.get(id).cloned().unwrap_or_else(|| "this pane".into()));
        let path = std::env::var_os("PATH");
        let agents = self.config.agents.iter().map(|tool| (tool.name.clone(), clanky::agent_launch::installed(&tool.command, path.as_deref()))).collect();
        clanky::calm::palette::Context { sessions, post_ai: self.applied_post_ai, terminal, editor: self.files.panes.contains_key(&self.files.focus), agents }
    }
    pub(super) fn palette_effect(&mut self, effect: clanky::calm::palette::Effect) -> Result<(), String> {
        use clanky::calm::palette::{Command, Effect};
        let Effect::Run(command) = effect else { self.reset_ime(); return Ok(()); };
        self.reset_ime();
        match command {
            Command::NewTab => self.new_tab(),
            Command::SplitRight => self.split(Axis::Columns),
            Command::SplitDown => self.split(Axis::Rows),
            Command::ClosePane => self.close_pane(),
            Command::ToggleAgentAccess => {
                let id = self.focused().ok_or("No terminal has focus")?.to_owned();
                let old = self.engine.agent_permission(&id).map_err(|e| e.message)?;
                self.engine.set_agent_permission(&id, !old).map_err(|e| e.message)?;
                self.studio.permissions.insert(id, !old);
                Ok(())
            }
            Command::StopAgents => {
                self.call("v4.revoke_all", json!({}))?;
                self.status = Some("Agent access revoked; pending waits cancelled".into());
                Ok(())
            }
            Command::ToggleSidebars => { self.panels.visible = !self.panels.visible; self.studio.drag = None; self.resize_panes() }
            Command::ZoomIn => { self.zoom.adjust(1.0); self.metrics_changed() }
            Command::ZoomOut => { self.zoom.adjust(-1.0); self.metrics_changed() }
            Command::ZoomReset => { self.zoom.reset(); self.metrics_changed() }
            Command::Settings => self.toggle_settings(),
            Command::OpenFile => { let tab = self.focused().map(str::to_owned); self.open_prompt(Axis::Columns, false, tab, self.files.focus.clone()) }
            Command::LoadChanges => { let target = self.files.focus.clone(); self.open_prompt(Axis::Columns, true, None, target) }
            Command::Session(id) => self.activate_studio(HitAction::Session(id)),
            Command::StartAgent(None) => { self.open_agent_picker(); Ok(()) }
            Command::StartAgent(Some(index)) => self.start_agent(index, false),
            Command::Run(text) => {
                let id = self.focused().ok_or("No terminal has focus")?.to_owned();
                self.engine.input(&id, format!("{text}\r").as_bytes()).map_err(|e| e.message)?;
                self.sent_input = true;
                Ok(())
            }
        }
    }
    fn activate_studio(&mut self, action: HitAction) -> Result<(), String> {
        match action {
            HitAction::Settings=>{self.toggle_settings()?;},
            HitAction::NewTab=>{self.new_tab()?;},
            HitAction::Split=>{self.split(Axis::Columns)?;},
            HitAction::Palette => { self.open_palette(); self.reset_ime(); }
            HitAction::StartAgent => self.open_agent_picker(),
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
            HitAction::Block { tab_id, block_id, action } => self.card_action(&tab_id, &block_id, &action)?,
            HitAction::Find(0) => { self.search.close(); self.reset_ime(); }
            HitAction::Review { pane, action } => {
                let editor = self.files.panes.get_mut(&pane).ok_or("That editor was closed")?;
                editor.review_action(&action)?;
                self.files.focus_on(pane); self.reset_ime();
            }
            HitAction::Find(direction) => self.search.step(&self.engine, direction < 0)?,
        }
        Ok(())
    }
    pub(super) fn click_studio(&mut self) -> Result<(), String> {
        if self.settings_button_hit() {return self.toggle_settings();}
        self.studio.card_focus=None;
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
        if self.search.active_target().is_some() {
            // Only the find bar takes clicks while the pinned view is shown.
            if let Some(action) = self.studio.hits.iter().rev().find(|h| h.rect.contains(self.mouse.0, self.mouse.1) && matches!(h.action, HitAction::Find(_))).map(|h| h.action.clone()) { return self.activate_studio(action); }
            return Ok(());
        }
        if self.files.modal() { return Ok(()); }
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
            if self.studio.reviews.get(&id).is_some_and(|r|r.content.contains(self.mouse.0,self.mouse.1) && self.mouse.1 < (r.content.y+layout.cell_px[1] as f32) as f64) { return Ok(()); }
            let hit = self.studio.reviews.get(&id).filter(|r|r.content.contains(self.mouse.0,self.mouse.1)).map(|r| {
                (((self.mouse.0-r.content.x as f64)/layout.cell_px[0] as f64) as usize,
                 r.first_row+((self.mouse.1-r.content.y as f64)/layout.cell_px[1] as f64) as usize)
            }).or_else(||point(pane,self.mouse,layout.cell_px,false).map(|(col,row)|(col,row+EDITOR_TITLE_ROWS)));
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
        // File/editor entry stays unavailable until its calm controls are rebuilt.
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
            let moved = clanky::calm::drag_divider(&capture.layout, capture.index, pointer)?;
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
                    if let Some(editor) = self.files.panes.get_mut(&id) { editor.drag(&local_rect(pane), col, row+EDITOR_TITLE_ROWS)?; }
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
                .unwrap_or_else(||point(pane,self.mouse,layout.cell_px,true).map_or(0,|(_,row)|row+EDITOR_TITLE_ROWS));
            self.files.scroll(&id, row, lines.round() as i32, columns.round() as i32)?;
        } else { self.engine.scroll(&id, -lines.round() as i32).map_err(|e| e.message)?; }
        if self.config.animations {
            if let Some(old) = self.studio.snapshots.get(&id) { self.studio.scrolls.insert(id, Scroll { old: old.clone(), start: Instant::now() }); }
        }
        Ok(())
    }

    pub(super) fn draw_studio(&mut self) -> Result<(), String> {
        if self.stop || self.is_suspended() { return Ok(()); }
        let trace_start=Instant::now();
        let layout = self.pixel_layout()?;
        let trace_layout=Instant::now();
        let mut trace_snapshot=0u64;
        if let Some(theme)=self.theme_runtime.as_ref().and_then(|runtime|runtime.active()) {self.studio.calm_theme=theme.clone();}
        self.studio.theme=clanky::calm::legacy_palette(&self.studio.calm_theme);
        self.files.viewport = Some(layout.clone());
        let now = Instant::now();
        let mut state = LookState { post_ai:self.applied_post_ai, theme: self.studio.theme.clone(), opacity: self.config.opacity, animations: self.config.animations,
            elapsed_ms: self.studio.started.elapsed().as_secs_f32()*1000.0,
            focused: if self.settings_open() || self.compose.active_target().is_some() || self.panels.open || self.files.modal() || self.search.active_target().is_some() { None }
                else if self.files.focus == TERMINALS { self.focused().map(str::to_owned) } else { Some(self.files.focus.clone()) },
            ..LookState::default() };
        state.palette_open = self.palette.open;
        state.user_name = self.studio.user_name.get_or_insert_with(|| clanky_core::platform::user_display_name().unwrap_or_default()).clone();
        state.uptime_seconds = self.studio.metadata["uptime_ms"].as_u64().unwrap_or(0)/1000;
        state.masking_ready = self.studio.metadata["masking_ready"].as_bool().unwrap_or(false);
        state.activity = self.studio.activity.clone();
        for (index, group) in self.groups.iter().enumerate() {
            if let Some(id) = group.layout.leaves().first() {
                state.session_counts.insert(id.clone(),group.layout.leaves().len());
                state.sessions.push(look::SessionItem { id: id.clone(), title: self.studio.terminal_info.get(&group.focus).and_then(|info|info["cwd"].as_str()).and_then(|cwd|std::path::Path::new(cwd).file_name()).map(|name|name.to_string_lossy().into_owned()).unwrap_or_else(||format!("Session {}",index+1)), active: index == self.active });
            }
        }
        for (index,group) in self.groups.iter().enumerate(){
            let name=state.sessions.get(index).map(|session|session.title.as_str()).unwrap_or("Session");
            for (pane,id) in group.layout.leaves().iter().enumerate(){self.studio.pane_labels.insert(id.clone(),clanky::calm::pane_label(name,pane));}
        }
        let mut retained:Vec<_>=self.groups.iter().flat_map(|group|group.layout.leaves()).collect();
        if let Some(recent)=self.studio.metadata["recent"].as_array(){retained.extend(recent.iter().filter_map(|event|event["tab_id"].as_str().map(str::to_owned)));}
        self.studio.pane_labels.retain(|id,_|retained.contains(id));state.pane_labels=self.studio.pane_labels.clone();
        self.studio.seen_files.retain(|id|self.files.panes.contains_key(id));
        for (index,(id, editor)) in self.files.panes.iter().enumerate() {
            if self.studio.seen_files.insert(id.clone()){
                let name=editor.path().file_name().map(|name|name.to_string_lossy().into_owned()).unwrap_or_else(||"Untitled".into());
                self.studio.local_recent.push_back((now,look::TimelineItem{label:"File opened".into(),detail:format!("{name} · editor pane {}",index+1)}));
                while self.studio.local_recent.len()>20{self.studio.local_recent.pop_front();}
            }
            // calm.html lists files by name ("compose.rs"); unsaved work is said in words.
            let name=editor.path().file_name().map(|name|name.to_string_lossy().into_owned()).unwrap_or_else(||editor.path().display().to_string());
            state.files.push(look::FileItem { id: id.clone(), title: format!("{name}{}", if editor.dirty() { " · unsaved" } else { "" }),
                pending: editor.pending_count(), active: self.files.focus == *id });
        }
        if let Some(agents) = self.studio.metadata["agents"].as_array() {
            for agent in agents {
                if let (Some(connection_id),Some(peer_pid)) = (agent["connection_id"].as_u64(),agent["peer_pid"].as_u64().and_then(|n|u32::try_from(n).ok())) {
                    if let Some(name)=agent["client_name"].as_str().filter(|s|!s.is_empty()) {
                        let label=agent["model"].as_str().filter(|s|!s.is_empty()).map(|model|format!("{name} · {model}")).unwrap_or_else(||name.into());
                        state.agent_labels.insert(connection_id,label);
                    }
                    state.agents.push(look::AgentItem { connection_id, peer_pid, tab_id: agent["tab_id"].as_str().map(str::to_owned), running: agent["running"].as_bool().unwrap_or(false) });
                }
            }
        }
        let mut recent=clanky::calm::human_recent(&self.studio.metadata["recent"],&state.pane_labels,&self.studio.metadata["agents"],&self.config.agents);
        recent.extend(self.studio.local_recent.iter().map(|(at,item)|(at.elapsed().as_millis().min(u64::MAX as u128) as u64,item.clone(),None)));
        recent.sort_by_key(|(age,_,_)|std::cmp::Reverse(*age));
        for (age,item,command) in recent.into_iter().rev().take(20).collect::<Vec<_>>().into_iter().rev(){
            // The newest finished card in that pane with the same command gives the exit code.
            let exit=command.and_then(|(tab,command)|self.studio.blocks.get(&tab).and_then(|view|view["blocks"].as_array())
                .and_then(|blocks|blocks.iter().rev().find(|b|b["command"].as_str().map(str::trim)==Some(command.as_str()) && b["complete"]==true))
                .and_then(|b|b["exit_code"].as_i64()).and_then(|code|i32::try_from(code).ok()));
            state.timeline_exits.push(exit);
            let seconds=age/1000;
            state.timeline_times.push(if seconds<60{format!("{seconds}s")}else if seconds<3600{format!("{}m",seconds/60)}else{format!("{}h",seconds/3600)});
            state.timeline.push(item);
        }
        self.studio.reviews.clear();
        let mut ui_rows: Vec<(PixelRect,String,Option<String>)> = Vec::new();
        let mut review_controls: Vec<(PixelRect,String,String)> = Vec::new();
        let mut review_hits: Vec<Hotspot> = Vec::new();
        let mut ime = None;
        let mut scroll_clips=Vec::new();
        for pane in &layout.panes {
            if pane.rows == 0 || pane.cols == 0 { continue; }
            let mut snapshot;
            let mut title = self.studio.terminal_info.get(&pane.id).and_then(|info|info["cwd"].as_str()).map(home_short).unwrap_or_else(||pane.id.clone());
            let mut editor_rows = None;
            if self.files.panes.contains_key(&pane.id) {
                snapshot = cells(pane.rows, pane.cols, &state.theme);
                editor_rows = self.files.draw_pane(&pane.id, &mut snapshot, local_rect(pane));
                title = self.files.panes[&pane.id].path().to_str().map(home_short).unwrap_or_else(||self.files.panes[&pane.id].path().display().to_string());
                // UI rows keep the editor controller's geometry; source and real diff previews stay mono.
                let mut rows = vec![0, pane.rows-1];
                if let Some(text_rows) = editor_rows {
                    let start = text_rows+1;
                    if start+2 < pane.rows-1 {
                        let m = layout.metrics;
                        let inset = m.card_inset as f32;
                        let y = pane.grid_origin[1]+(start-1) as f32*layout.cell_px[1] as f32;
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
                            let status=row_text(&snapshot,pane.rows-1).trim().to_owned();
                            let status=if status.starts_with("Ctrl+S save") || status.is_empty() {"Ctrl+S saves the accepted changes".to_owned()} else {status.replace(" | "," · ")};
                            ui_rows.push((PixelRect { x:content.x,y:content.y+(count-1) as f32*layout.cell_px[1] as f32,w:content.w,h:layout.cell_px[1] as f32 },status,Some("editor-hint".into())));
                            for cell in &mut preview.cells[(count-1)*cols..] { cell.ch=' '; }
                        }
                        // Calm buttons on the first content row; they run the review actions themselves.
                        review_controls.push((PixelRect { x:content.x,y:content.y,w:content.w,h:layout.cell_px[1] as f32 },pane.id.clone(),panel_id.clone()));
                        for cell in &mut preview.cells[..cols] { cell.ch=' '; }
                        self.studio.reviews.insert(pane.id.clone(),Review { content,first_row:start+1 });
                        // "Hunk 3 Pending | clipped preview offset 0" -> "Proposed change 3 · pending".
                        let header=row_text(&snapshot,start);let mut words=header.split_whitespace().skip(1);
                        let title=match (words.next(),words.next()) { (Some(id),Some(state)) => format!("Proposed change {id} · {}",state.to_lowercase()), _ => "Proposed changes".into() };
                        let mut card=panel(&panel_id,rect,title,Vec::new());
                        card.snapshot=Some(preview); state.panels.push(card);
                        for cell in &mut snapshot.cells[start*pane.cols..] { cell.ch=' '; }
                        rows.retain(|r|*r<start);
                    }
                }
                // The pane header names the file; the editor core's own title row stays blank,
                // and its status row becomes one muted hint (find prompts and errors still show).
                for row in rows {
                    let text = row_text(&snapshot,row).trim().to_owned();
                    if row > 0 {
                        let dirty = self.files.panes[&pane.id].dirty();
                        let hint = if text.starts_with("Ctrl+S save") || text.is_empty() {
                            if dirty { "Unsaved changes · Ctrl+S saves · Ctrl+F finds".to_owned() } else { "Ctrl+S saves · Ctrl+F finds".to_owned() }
                        } else { text.replace(" | ", " · ") };
                        ui_rows.push((PixelRect { x: pane.grid_origin[0], y: pane.grid_origin[1]+(row-1) as f32*layout.cell_px[1] as f32,
                            w: pane.grid_clip.w, h: layout.cell_px[1] as f32 }, hint, Some("editor-hint".into())));
                    }
                    for cell in &mut snapshot.cells[row*pane.cols..(row+1)*pane.cols] { cell.ch = ' '; }
                }
                widget(&mut snapshot,&state.theme);
                // The core's title row is blank in the window: show the text from the top
                // padding. Mouse rows add EDITOR_TITLE_ROWS back before reaching the editor.
                let cols=snapshot.cols;let blank=snapshot.cells[0].clone();
                snapshot.cells.drain(..cols.min(snapshot.cells.len()));snapshot.cells.extend(std::iter::repeat_n(blank,cols));
                snapshot.cursor=snapshot.cursor.and_then(|(row,col)|row.checked_sub(EDITOR_TITLE_ROWS).map(|row|(row,col)));
            } else {
                let cached=if self.terminal_dirty{None}else{self.engine.cached_snapshot_with_blocks(&pane.id)};
                let trace_snap=Instant::now();
                let fetched=cached.map(Ok).unwrap_or_else(||self.engine.snapshot_with_blocks(&pane.id));
                trace_snapshot+=(Instant::now()-trace_snap).as_micros() as u64;
                let (current, anchors) = match fetched { Ok(s) => s, Err(e) if e.code == "not_found" => continue, Err(e) => return Err(e.message) };
                snapshot = current;
                self.studio.blocks.insert(pane.id.clone(), anchors);
            }
            if self.search.active_target() == Some(pane.id.as_str()) {
                // The pinned search view replaces the live grid; matches use calm stone tints.
                let c=&self.studio.calm_theme.colors;let mix=|t:f32|std::array::from_fn(|i|(c.background[i] as f32*(1.0-t)+c.foreground[i] as f32*t).round() as u8);
                let blank=clanky_core::Cell{ch:' ',fg:[c.foreground[0],c.foreground[1],c.foreground[2]],bg:[c.background[0],c.background[1],c.background[2]],color_source:None};
                if let Some(pinned)=self.search.pinned(pane.rows,pane.cols,blank,mix(0.16),mix(0.34)) { snapshot = pinned; }
                state.find = Some((pane.id.clone(),self.search.query().to_owned(),self.search.total()));
            }
            let mut view = PaneState::new(pane.id.clone(),snapshot.clone());
            view.title = title;
            view.allow_agent = self.studio.permissions.get(&pane.id).copied().unwrap_or(false);
            view.driver = if let Some(editor) = self.files.panes.get(&pane.id) { if editor.dirty() { "Local editor · unsaved".into() } else { "Local editor".into() } } else {
                self.studio.terminal_info.get(&pane.id).and_then(|info|info["program"].as_str()).map(str::to_owned).unwrap_or_default()
            };
            if let Some(drivers) = self.studio.metadata["drivers"].as_array().filter(|_|!self.files.panes.contains_key(&pane.id)) {
                if let Some(driver) = drivers.iter().find(|d| d["tab_id"].as_str() == Some(pane.id.as_str()) && d["running"] == true) {
                    view.agent_running = true;
                    view.driver = driver["connection_id"].as_u64().and_then(|id|state.agent_labels.get(&id)).cloned().unwrap_or_else(||format!("agent {}",driver["peer_pid"]));
                } else if view.driver.is_empty() && !self.studio.metadata.is_null() { view.driver = "Terminal".into(); }
            }
            // A pane opened for an agent tool, with access on, is named for the tool and
            // drawn as an agent pane (amber edge and chip).
            if !view.agent_running && view.allow_agent && !self.applied_post_ai && !self.files.panes.contains_key(&pane.id) {
                if let Some(tool) = self.studio.terminal_info.get(&pane.id).and_then(|info| info["agent_tool"].as_str()) {
                    view.title = format!("{tool} · {}", view.title);
                    view.driver = String::new();
                    view.agent_running = true;
                }
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
        if let Some(card) = self.files.prompt_card(modal_rect(&layout,3),&layout.metrics) { state.panels.push(card); }
        if let Some((rect, grid)) = self.dock(&layout) {
            let mut snapshot = cells(grid.rows,grid.cols,&state.theme);
            let mut underlines = self.compose.draw(&mut snapshot,&grid);
            // The editor's own header and status rows are replaced by the card's title and
            // one calm hint; the rows stay blank so the editor's geometry is unchanged.
            // Errors and IME text arrive on the last row; otherwise the hint names the keys.
            let status=row_text(&snapshot,grid.rows-1).trim().to_owned();
            for cell in &mut snapshot.cells[(grid.rows-1)*grid.cols..grid.rows*grid.cols] { cell.ch=' '; }
            ui_rows.push((PixelRect { x:rect.x+layout.metrics.panel_padding as f32,y:rect.y+(layout.metrics.panel_padding+layout.metrics.panel_header) as f32+grid.rows.saturating_sub(1) as f32*layout.cell_px[1] as f32,
                w:(rect.w-2.0*layout.metrics.panel_padding as f32).max(0.0),h:layout.cell_px[1] as f32 },if status.is_empty(){"Ctrl+Enter sends · Esc closes".into()}else{status},Some("compose-hint".into())));
            for underline in &mut underlines { underline.color = rgb(state.theme.spelling); }
            widget(&mut snapshot,&state.theme);
            if let Some((row,col)) = snapshot.cursor { ime = Some([rect.x+layout.metrics.panel_padding as f32+col as f32*layout.cell_px[0] as f32,rect.y+(layout.metrics.panel_padding+layout.metrics.panel_header) as f32+row as f32*layout.cell_px[1] as f32]); }
            state.panels.push(PanelState { id:"compose".into(),rect,title:format!("Compose for {}",self.compose.active_target().and_then(|id|state.pane_labels.get(id)).cloned().unwrap_or_else(||"this pane".into())),lines:Vec::new(),snapshot:Some(snapshot),underlines,opacity:1.0,rise_px:0.0 });
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
                        while end>1 && snapshot.cells[(end-1)*snapshot.cols..end*snapshot.cols].iter().zip(&scroll.old.cells[(end-1)*snapshot.cols..end*snapshot.cols]).all(|(a,b)|a.ch==b.ch && a.fg==b.fg && a.bg==b.bg && a.color_source==b.color_source) { end-=1; }
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
        self.update_card_actions(&layout,&mut state,now);
        let terminal_px=self.metrics()?.font_px;
        let palette_context=self.palette_context();
        let agent_context=self.agent_picker.open.then(||self.agent_context());
        let renderer=self.renderer.as_mut().ok_or("renderer not ready")?;
        let trace_state=Instant::now();
        let mut presentation = clanky::calm::presentation(&layout,&state,&self.studio.calm_theme,terminal_px,self.mouse,self.pointer_pressed,&mut |run| renderer.measure_text(run));
        let trace_scene=Instant::now();
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
            let hint=matches!(card_id.as_deref(),Some("compose-hint"|"editor-hint"));
            let card=if card_id.as_deref()==Some("compose-hint"){state.panels.iter().find(|p|p.id=="compose")}else{card};let rise=card.map_or(rise,|p|p.rise_px);let opacity=card.map_or(opacity,|p|p.opacity);
            let px=if hint || card_id.as_ref().is_some_and(|id|id.starts_with("review-")) { layout.metrics.small_font_px as f32 } else { layout.metrics.ui_font_px as f32 };
            let color=if hint{self.studio.calm_theme.colors.muted_foreground}else{state.theme.ink};
            if !text.is_empty() { presentation.scene.layers.push(Layer::Text(TextRun { origin:[rect.x,rect.y+rise+if hint{(rect.h-px*1.3)/2.0}else{0.0}],text:truncate(&text,rect.w/(if hint{px*0.5}else{px})),px,color,weight:400,mono:false,opacity,letter_spacing:0.0 })); }
        }
        for (rect,pane,card) in &review_controls {
            let rise=state.panels.iter().find(|p|p.id==*card).map_or(0.0,|p|p.rise_px);
            let (layers,hits)=clanky::calm::review_buttons(&layout,&self.studio.calm_theme,PixelRect{y:rect.y+rise,..*rect},self.mouse,self.pointer_pressed,&mut |run|renderer.measure_text(run));
            presentation.scene.layers.extend(layers);
            review_hits.extend(hits.into_iter().map(|(rect,action)|Hotspot{rect,action:HitAction::Review{pane:pane.clone(),action:action.into()}}));
        }
        let status_layers = self.status.as_deref().map(|status| clanky::calm::notice(&layout,&self.studio.calm_theme,status,&mut |run|renderer.measure_text(run)));
        if let Some(panel)=&mut self.settings {
            panel.forced_post_ai=self.startup_flags.post_ai;
            let access:Vec<_>=state.panes.iter().filter(|pane|!self.files.panes.contains_key(&pane.id)).map(|pane|clanky::calm::settings_panel::PaneAccess{id:pane.id.clone(),label:state.pane_labels.get(&pane.id).cloned().unwrap_or_else(||pane.title.clone()),allowed:pane.allow_agent}).collect();
            presentation.scene.layers.extend(panel.draw(&layout,&self.studio.calm_theme,&self.config,&access,state.masking_ready,self.theme_runtime.as_ref().map(|runtime|&runtime.store),self.window.as_ref().is_some_and(|window|clanky::platform::system_light(window)),self.mouse,self.pointer_pressed,&mut |run|renderer.measure_text(run)));
            if panel.open{ime=panel.ime_position();}
        }
        if let Some(context)=&agent_context {
            presentation.scene.layers.extend(self.agent_picker.draw(&layout,&self.studio.calm_theme,context,self.mouse,self.pointer_pressed,&mut |run|renderer.measure_text(run)));
        }
        if self.palette.open {
            presentation.scene.layers.extend(self.palette.draw(&layout,&self.studio.calm_theme,&palette_context,self.mouse,self.pointer_pressed,&mut |run|renderer.measure_text(run)));
            ime = self.palette.ime_position();
        }
        if let Some(layers) = status_layers { presentation.scene.layers.extend(layers); }
        self.studio.hits = presentation.hits;
        self.studio.hits.extend(review_hits);
        if let Some(window) = &self.window {
            let title=format!("Clanky — {} — {:.0} pt",if self.files.focus == TERMINALS { self.focused().unwrap_or("no terminal") } else { &self.files.focus },self.zoom.points());
            if self.studio.title.as_ref()!=Some(&title){window.set_title(&title);self.studio.title=Some(title);}
            if let Some([x,y])=ime {
                let area=([x.max(0.0) as u32,y.max(0.0) as u32],layout.cell_px);
                if self.studio.ime!=Some(area){window.set_ime_cursor_area(PhysicalPosition::new(area.0[0],area.0[1]),PhysicalSize::new(area.1[0],area.1[1]));self.studio.ime=Some(area);}
            }
        }
        if let Some(pane) = layout.panes.first() {
            let theme = self.studio.calm_theme.clone();
            self.draw_robot(&mut presentation.scene, pane.rect.array(), &theme)?;
        }
        let trace_post=Instant::now();
        let renderer=self.renderer.as_mut().ok_or("renderer not ready")?;
        renderer.draw_scene(&presentation.scene)?;
        if self.frame_trace.enabled() {
            let us=|a:Instant,b:Instant|(b-a).as_micros() as u64;
            let gpu=renderer.frame_timing;
            self.frame_trace.phases(us(trace_start,Instant::now()),[us(trace_start,trace_layout),us(trace_layout,trace_state),us(trace_state,trace_scene),us(trace_scene,trace_post)],gpu,self.terminal_dirty,trace_snapshot);
        }
        self.frame_trace.presented();
        self.frame_trace.gate_frame(self.gate_clock.as_ref().map(|clock| clock.elapsed_ms()));
        self.terminal_dirty=false;
        let animation_end=self.animation_deadline(&state).into_iter().chain(self.robot_deadline()).max();
        let presented_at=self.clock_now();
        self.frame_clock.presented(presented_at,animation_end);
        Ok(())
    }
}
/// Mockup pane titles name the home folder "~" (~/git/clanky).
fn home_short(cwd: &str) -> String {
    match clanky_core::platform::home_dir().and_then(|home| home.to_str().map(str::to_owned)) {
        Some(home) if cwd == home => "~".into(),
        Some(home) if cwd.starts_with(&(home.clone() + "/")) => format!("~{}", &cwd[home.len()..]),
        _ => cwd.into(),
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

/// Only server anchors from this snapshot's generation place cards; never text matching.
fn matched_blocks(snapshot: &TerminalSnapshot, view: &Value) -> Vec<look::BlockCard> {
    let mut cards = Vec::new();
    if !snapshot.images.is_empty() || snapshot.cols == 0 { return cards; }
    let Some(top) = view["viewport_line"].as_u64() else { return cards; };
    let Some(blocks) = view["blocks"].as_array() else { return cards; };
    for block in blocks {
        let (Some(id), Some(command), Some(line)) = (block["block_id"].as_str(), block["command"].as_str(), block["command_line"].as_u64()) else { continue; };
        let Some(prompt) = block["prompt_line"].as_u64() else { continue; };
        // Server lines (fish, measured): the command is typed on the line before output
        // starts (command_line), and the finish mark sits on the next prompt's line.
        let typed = if line > prompt { line - 1 } else { prompt };
        let Some(first) = typed.checked_sub(top).and_then(|n| usize::try_from(n).ok()).filter(|n| *n < snapshot.rows) else { continue; };
        let complete = block["complete"] == true;
        let end = match block["end_line"].as_u64() { Some(end) if complete => end.saturating_sub(1).max(typed), _ => top + snapshot.rows as u64 - 1 };
        let last = end.saturating_sub(top).min(snapshot.rows.saturating_sub(1) as u64) as usize;
        if last < first || cards.iter().any(|c: &look::BlockCard| first <= c.last_row && last >= c.first_row) { continue; }
        cards.push(look::BlockCard { id:id.into(), first_row:first, last_row:last, command:command.into(),
            exit_code:block["exit_code"].as_i64().and_then(|n|i32::try_from(n).ok()), duration_ms:block["duration_ms"].as_u64(), running:block["complete"] != true });
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
    let same = |a: &[Cell],b: &[Cell]| a.iter().zip(b).all(|(a,b)|a.ch==b.ch && a.fg==b.fg && a.bg==b.bg && a.color_source==b.color_source);
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
