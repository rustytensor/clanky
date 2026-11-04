mod panels;
mod editor_workspace;
use editor_workspace::{EditorWorkspace, TERMINALS};
use clanky::editor::{EditorPane, location_at};

use clanky::{
    config::{Config, Metrics, Zoom},
    compose::{Compose, ComposeEvent},
    clipboard::ClipboardBridge,
    scrollback_search::Search,
    window_tools,
    input::{InputState, key_bytes},
};
use clanky_api::Client;
use clanky_core::{
    Cell, TerminalSnapshot, Workspace, WorkspaceGroup as Group,
    layout::{Axis, Rect},
    platform,
};
use clanky_render::{Renderer, Underline, block_on};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowId},
};

fn main() {
    if let Err(error) = run() {
        eprintln!("clanky: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let (mut headless, mut socket, mut config_path) = (false, None, None);
    let mut stdio = false;
    let mut mcp = false;
    let mut host = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--headless" | "--server" => headless = true,
            "--stdio" => stdio = true,
            "mcp" => mcp = true,
            "attach" => host = Some(args.next().ok_or("attach needs a host")?),
            "--socket" => socket = Some(PathBuf::from(args.next().ok_or("--socket needs a path")?)),
            "--config" => {
                config_path = Some(PathBuf::from(args.next().ok_or("--config needs a path")?))
            }
            "--help" | "-h" => {
                println!(
                    "clanky [--server|--headless|--stdio] [--socket PATH] [--config PATH]\nclanky attach HOST [--config PATH]\nCtrl+Shift+T: new tab; Ctrl+Shift+D/E: split right/below\nCtrl+Shift+W: close pane; Ctrl+Tab: next tab; click: focus pane\nCtrl+=/-/0: zoom/reset; Ctrl+Shift+A: toggle focused pane agent access\nCtrl+Shift+P: palette (Open file / Load changes / Compose); Ctrl+Shift+B: sidebar\nCtrl+Shift+O: open file; click path:line: open beside terminal\nEditor: Ctrl+S save; Ctrl+Z/Shift+Z undo/redo; Ctrl+F find\nEditor: Ctrl+Alt+A/R accept/reject (Shift: all); dirty close refuses\nCompose: Ctrl+Shift+Enter opens focused terminal; Enter newline; Ctrl+Enter sends\nCompose: Escape retains draft; F7 suggestions; Ctrl+Alt+A add word\nCompose drafts are memory-only for this window; closing it can lose them\nCtrl+Shift+Escape: revoke agents (works everywhere)\nclanky mcp [--socket PATH]: agent-only MCP"
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    if host.is_some() && (socket.is_some() || headless || stdio || mcp) {
        return Err("attach cannot use --socket, --server, --headless, --stdio or mcp".into());
    }
    if u8::from(headless) + u8::from(stdio) + u8::from(mcp) > 1 {
        return Err("choose server, stdio or MCP mode".into());
    }
    if let Some(host) = host {
        let config = Config::load(config_path.as_deref())?;
        let client = Client::connect_ssh(&host).map_err(|e| e.to_string())?;
        return run_window(client, config, false);
    }
    let socket = match socket {
        Some(path) => path,
        None => platform::default_socket().map_err(|e| e.to_string())?,
    };
    if (headless || stdio) && config_path.is_some() {
        Config::load(config_path.as_deref())?;
    }
    if headless {
        return clanky::session::run_server(&socket);
    }
    if stdio {
        return clanky::session::stdio(&socket);
    }
    if mcp {
        return clanky::mcp::serve(&socket);
    }
    let config = Config::load(config_path.as_deref())?;
    run_window(clanky::session::connect_or_start(&socket)?, config, true)
}

struct App {
    engine: Client,
    revision: u64,
    stop: bool,
    config: Config,
    zoom: Zoom,
    scale: f64,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    groups: Vec<Group>,
    active: usize,
    sizes: BTreeMap<String, (usize, usize)>,
    modifiers: ModifiersState,
    mouse: (f64, f64),
    input: InputState,
    occluded: bool,
    error: Option<String>,
    status: Option<String>,
    next_frame: Instant,
    panels: panels::Panels,
    files: EditorWorkspace,
    local_files: bool,
    palette_editor: Option<String>,
    // Drafts live only in this window; closing it can lose retained drafts.
    compose: Compose,
    compose_rect: Option<Rect>,
    compose_dragging: bool,
    compose_enter_held: bool,
    search: Search,
    url_opener: platform::UrlOpener,
    clipboard_bridge: ClipboardBridge,
}

fn run_window(engine: Client, config: Config, local_files: bool) -> Result<(), String> {
    let workspace = engine.workspace_get().map_err(|e| e.message)?;
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let clipboard_bridge = ClipboardBridge::new(engine.clone(), config.osc52_write, config.osc52_read);
    clipboard_bridge.set_tabs(workspace.groups.iter().flat_map(|g| g.layout.leaves()).collect());
    let mut app = App {
        engine,
        search: Search::default(),
        url_opener: platform::UrlOpener::default(),
        clipboard_bridge,
        stop: false,
        revision: workspace.revision,
        zoom: Zoom::new(config.font_size_pt),
        compose: Compose::new(&config.compose_dictionary_dir, &config.compose_dictionary,
            config.compose_words.clone(), config.compose_send_enter),
        compose_rect: None,
        compose_dragging: false,
        compose_enter_held: false,
        config,
        scale: 1.0,
        window: None,
        renderer: None,
        groups: workspace.groups,
        active: workspace.active,
        sizes: BTreeMap::new(),
        modifiers: ModifiersState::empty(),
        mouse: (0.0, 0.0),
        input: InputState::default(),
        occluded: false,
        error: None,
        status: None,
        next_frame: Instant::now(),
        panels: panels::Panels::default(),
        files: EditorWorkspace::default(),
        local_files,
        palette_editor: None,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.error.map_or(Ok(()), Err)
}

impl App {
    fn call(&self, op: &str, args: Value) -> Result<Value, String> {
        self.engine.request(op, &args, true).map_err(|e| e.message)
    }
    fn metrics(&self) -> Result<Metrics, String> {
        self.zoom.metrics(self.scale)
    }
    fn full_rect(&self) -> Result<Rect, String> {
        let m = self.metrics()?;
        let size = self.window.as_ref().ok_or("window not ready")?.inner_size();
        let cols = ((size.width / m.cell_width) as usize).clamp(1, 1000);
        let rows = ((size.height / m.cell_height).saturating_sub(1) as usize)
            .clamp(1, (250000 / cols - 1).min(1000));
        Ok(Rect {
            col: 0,
            row: 1,
            cols,
            rows,
        })
    }
    fn rect(&self) -> Result<Rect, String> {
        let mut r = self.full_rect()?;
        r.cols -= self.panels.width(r.cols);
        Ok(r)
    }
    fn terminal_rect(&self) -> Result<Rect, String> {
        Ok(self.files.terminal_rect(self.rect()?))
    }
    fn cwd(&self, tab: Option<&str>) -> Result<Option<PathBuf>, String> {
        if !self.local_files || self.engine.is_closed() { return Ok(None); }
        let Some(tab) = tab else { return Ok(None); };
        let result = self.engine.exchange(3, "terminal_cwd", &json!({"tab_id":tab})).map_err(|e| e.message)?;
        Ok(result["cwd"].as_str().map(PathBuf::from).filter(|p| p.is_absolute()))
    }
    fn open_prompt(&mut self, axis: Axis, import: bool, tab: Option<String>, target: String) -> Result<(), String> {
        // Capture cwd and destination now, never when Enter later confirms.
        let cwd = self.cwd(tab.as_deref()).unwrap_or(None);
        self.files.prompt(target, cwd, axis, import, !self.local_files)?;
        self.reset_ime();
        Ok(())
    }
    fn local_request(&mut self) -> Result<(), String> {
        if let Some(request) = self.panels.local.take() {
            let target = self.palette_editor.take().unwrap_or_else(|| self.files.focus.clone());
            let tab = (!request.tab.is_empty()).then_some(request.tab);
            if request.action == "Compose" {
                if target != TERMINALS { return Err("Focus a terminal to open Compose; a file editor is focused".into()); }
                return self.open_compose(tab.as_deref());
            }
            self.open_prompt(Axis::Columns, request.action == "Load changes", tab, target)?;
        }
        Ok(())
    }
    fn open_compose(&mut self, target: Option<&str>) -> Result<(), String> {
        if self.files.focus != TERMINALS {
            return Err("Focus a terminal to open Compose; a file editor is focused".into());
        }
        let target = target.ok_or("No focused terminal for Compose")?;
        let root = self.terminal_rect()?;
        let rect = self.groups.iter()
            .find(|group| group.layout.contains(target))
            .and_then(|group| group.layout.rectangles(root).into_iter().find(|(id, _)| id == target))
            .map(|(_, rect)| rect).ok_or("Compose target was closed")?;
        self.compose.open(target)?;
        self.compose_rect = Some(rect);
        self.compose_dragging = false;
        self.reset_ime();
        Ok(())
    }
    fn compose_bounds(&self) -> Option<Rect> {
        let target = self.compose.active_target()?;
        let full = self.full_rect().ok()?;
        let root = self.terminal_rect().ok()?;
        // Follow geometry changes, not focus changes; a closed target never retargets Send.
        let mut rect = self.groups.iter().find(|g| g.layout.contains(target))
            .and_then(|g| g.layout.rectangles(root).into_iter().find(|(id, _)| id == target))
            .map(|(_, rect)| rect).or_else(|| self.compose_rect.clone())?;
        rect.cols = rect.cols.min(full.cols.saturating_sub(rect.col));
        rect.rows = rect.rows.min((full.row + full.rows).saturating_sub(rect.row));
        if rect.cols == 0 || rect.rows == 0 { rect = full; }
        Some(rect)
    }
    fn has_local_work(&self) -> bool {
        !self.files.panes.is_empty() || self.compose.active_target().is_some()
            || self.compose.has_drafts()
    }
    fn focused(&self) -> Option<&str> {
        self.groups.get(self.active).map(|g| g.focus.as_str())
    }
    fn reset_ime(&mut self) {
        self.input.focus_lost();
        self.compose.preedit("");
        if let Some(w) = &self.window {
            w.set_ime_allowed(false);
            w.set_ime_allowed(true);
        }
    }
    fn load_workspace(&mut self, workspace: Workspace) {
        let previous = self.focused().map(str::to_owned);
        self.revision = workspace.revision;
        self.groups = workspace.groups;
        self.active = workspace.active;
        let ids: Vec<_> = self.groups.iter().flat_map(|g| g.layout.leaves()).collect();
        self.sizes.retain(|id, _| ids.contains(id));
        self.clipboard_bridge.set_tabs(ids);
        if previous.as_deref() != self.focused() {
            self.reset_ime();
        }
    }
    fn sync_groups(&mut self) -> Result<(), String> {
        if self.engine.is_closed() { return Ok(()); }
        let workspace = self.engine.workspace_get().map_err(|e| e.message)?;
        if workspace.revision != self.revision {
            self.load_workspace(workspace);
        }
        Ok(())
    }
    fn save_workspace(&mut self) -> Result<(), String> {
        let workspace = Workspace {
            revision: self.revision,
            groups: self.groups.clone(),
            active: self.active,
        };
        match self.engine.workspace_set(&workspace) {
            Ok(workspace) => {
                self.load_workspace(workspace);
                Ok(())
            }
            Err(error) => {
                // Never retry a stale layout over another window's changes.
                if let Ok(workspace) = self.engine.workspace_get() {
                    self.load_workspace(workspace);
                }
                Err(error.message)
            }
        }
    }
    fn is_suspended(&self) -> bool {
        self.occluded
            || self.window.as_ref().is_none_or(|window| {
                let size = window.inner_size();
                size.width == 0 || size.height == 0
            })
    }
    fn resize_panes(&mut self) -> Result<(), String> {
        // Minimizing must not shrink retained terminal screens to one cell.
        if self.is_suspended() || self.engine.is_closed() {
            return Ok(());
        }
        self.sync_groups()?;
        let root = self.terminal_rect()?;
        for group in &self.groups {
            for (id, r) in group.layout.rectangles(root.clone()) {
                if r.rows == 0 || r.cols == 0 || self.sizes.get(&id) == Some(&(r.rows, r.cols)) {
                    continue;
                }
                match self.engine.resize(&id, r.rows, r.cols) {
                    Ok(()) => {
                        self.sizes.insert(id, (r.rows, r.cols));
                    }
                    Err(e) if e.code == "not_found" => {}
                    Err(e) => return Err(e.message),
                }
            }
        }
        Ok(())
    }
    fn metrics_changed(&mut self) -> Result<(), String> {
        let m = self.metrics()?;
        if let Some(renderer) = &mut self.renderer {
            renderer.set_metrics(m.font_px, m.cell_width, m.cell_height)?;
        }
        self.sizes.clear();
        self.resize_panes()
    }
    fn spawn_terminal(&self, focused: Option<&str>, rows: usize, cols: usize) -> Result<String, String> {
        window_tools::new_terminal(&self.engine, focused, rows, cols)
    }
    fn new_tab(&mut self) -> Result<(), String> {
        let r = self.terminal_rect()?;
        // Capture the displayed source before open_tab changes workspace focus.
        let source = if self.files.focus == TERMINALS { self.focused() } else { None };
        let id = self.spawn_terminal(source, r.rows.max(1), r.cols.max(1))?;
        self.sync_groups()?;
        self.active = self
            .groups
            .iter()
            .position(|g| g.layout.contains(&id))
            .ok_or("new terminal missing from workspace")?;
        self.save_workspace()?;
        self.files.focus_on(TERMINALS.into());
        self.reset_ime();
        self.resize_panes()
    }
    fn split(&mut self, axis: Axis) -> Result<(), String> {
        if self.files.focus != TERMINALS {
            return self.open_prompt(axis, false, self.focused().map(str::to_owned), self.files.focus.clone());
        }
        let Some(target) = self.focused().map(str::to_owned) else {
            return self.new_tab();
        };
        self.sync_groups()?;
        let group = self
            .groups
            .iter()
            .find(|g| g.layout.contains(&target))
            .ok_or("split target was closed")?;
        let root = self.terminal_rect()?;
        let mut candidate = group.layout.clone();
        candidate.split(&target, "__new__".into(), axis.clone(), root.clone())?;
        let r = candidate
            .rectangles(root.clone())
            .into_iter()
            .find(|(id, _)| id == "__new__")
            .ok_or("new pane is not visible")?
            .1;
        let id = self.spawn_terminal(Some(&target), r.rows, r.cols)?;
        // open_tab already added an independent group. Work against its fresh
        // revision; on conflict that terminal must remain open independently.
        self.sync_groups()?;
        let target_group = self
            .groups
            .iter()
            .position(|g| g.layout.contains(&target))
            .ok_or("split target was closed; new terminal remains independent")?;
        if !self
            .groups
            .iter()
            .any(|g| g.layout.leaves() == vec![id.clone()])
        {
            return Err("new terminal layout changed in another window; split cancelled".into());
        }
        let mut layout = self.groups[target_group].layout.clone();
        layout.split(&target, id.clone(), axis, root)?;
        self.groups.retain(|g| !g.layout.contains(&id));
        let target_group = self
            .groups
            .iter()
            .position(|g| g.layout.contains(&target))
            .ok_or("split target missing")?;
        self.groups[target_group].layout = layout;
        self.groups[target_group].focus = id;
        self.active = target_group;
        self.save_workspace()?;
        self.reset_ime();
        self.resize_panes()
    }
    fn close_pane(&mut self) -> Result<(), String> {
        if self.files.focus != TERMINALS { self.files.close()?; self.reset_ime(); return self.resize_panes(); }
        // Use the displayed target, even if another window just changed focus.
        let target = self.focused().map(str::to_owned);
        self.sync_groups()?;
        if let Some(id) = target {
            let last = self
                .groups
                .iter()
                .map(|g| g.layout.leaves().len())
                .sum::<usize>()
                == 1;
            self.call("close_tab", json!({"tab_id":id}))?;
            if last {
                // The final reply is followed by server exit; do not query it.
                self.groups.clear();
                self.sizes.clear();
                self.clipboard_bridge.set_tabs(Vec::new());
                self.stop = !self.has_local_work();
                return Ok(());
            }
        }
        self.sync_groups()?;
        self.reset_ime();
        self.resize_panes()
    }
    fn send(&self, bytes: &[u8]) -> Result<(), String> {
        if self.search.active_target().is_some() || self.compose.active_target().is_some() || self.files.focus != TERMINALS || self.files.modal() || self.panels.open { return Ok(()); }
        if !bytes.is_empty() {
            if let Some(id) = self.focused() {
                self.engine.input(id, bytes).map_err(|e| e.message)?;
            }
        }
        Ok(())
    }
    fn key(&mut self, key: &Key, text: Option<&str>) -> Result<(), String> {
        // The stop key comes first: no panel, prompt or draft may swallow it.
        if self.modifiers.control_key() && self.modifiers.shift_key()
            && *key == Key::Named(NamedKey::Escape)
        {
            self.call("v4.revoke_all", json!({}))?;
            self.status = Some("Agent access revoked; pending waits cancelled".into());
            return Ok(());
        }
        if self.compose.active_target().is_some() {
            // The opening chord is never an editing/send chord, even while open.
            if self.modifiers.control_key() && self.modifiers.shift_key()
                && *key == Key::Named(NamedKey::Enter) { return Ok(()); }
            let event = self.compose.key(key, text, self.modifiers, self.input.composing())?;
            let result = if event == ComposeEvent::Send {
                let groups = &self.groups;
                let engine = &self.engine;
                self.compose.send_with(|target, bytes| {
                    if !groups.iter().any(|g| g.layout.contains(target)) {
                        return Err("Compose target was closed; draft retained".into());
                    }
                    engine.input(target, bytes).map_err(|e| e.message)
                })
            } else { Ok(()) };
            if self.compose.active_target().is_none() {
                self.compose_rect = None;
                self.compose_dragging = false;
                self.reset_ime();
            }
            return result;
        }
        if !self.files.modal() && !self.panels.open && self.search.active_target().is_some() {
            let result = self.search.key(&self.engine, key, text, self.modifiers, self.input.composing());
            if self.search.active_target().is_none() { self.reset_ime(); }
            return result;
        }
        if !self.files.modal() && !self.panels.open && self.files.focus == TERMINALS
            && self.modifiers.control_key() && self.modifiers.shift_key()
            && matches!(key, Key::Character(ch) if ch.eq_ignore_ascii_case("f"))
        {
            if let Some(target) = self.focused().map(str::to_owned) {
                self.search.open(&self.engine, &target)?;
                self.reset_ime();
            }
            return Ok(());
        }
        if !self.files.modal() && !self.panels.open
            && self.modifiers.control_key() && self.modifiers.shift_key()
            && *key == Key::Named(NamedKey::Enter)
        {
            let target = self.focused().map(str::to_owned);
            return self.open_compose(target.as_deref());
        }
        if self.files.modal() {
            let root = self.rect()?;
            let was_open = self.files.modal();
            self.files.prompt_key(key, text, self.input.composing(), root)?;
            if was_open && !self.files.modal() { self.reset_ime(); }
            return Ok(());
        }
        let ctrl = self.modifiers.control_key();
        if ctrl
            && self.modifiers.shift_key()
            && matches!(key, Key::Character(ch) if ch.eq_ignore_ascii_case("p"))
        {
            if !self.panels.open {
                self.palette_editor = Some(self.files.focus.clone());
                let focused = self.focused().map(str::to_owned);
                self.panels.refresh(&self.engine, focused.as_deref());
            }
            self.panels.toggle();
            self.reset_ime();
            return Ok(());
        }
        if self.panels.open {
            self.panels
                .key(&self.engine, key, text, self.input.composing());
            if !self.panels.open {
                self.reset_ime();
            }
            return self.local_request();
        }
        if ctrl && self.modifiers.shift_key() && matches!(key, Key::Character(ch) if ch.eq_ignore_ascii_case("o")) {
            return self.open_prompt(Axis::Columns, false, self.focused().map(str::to_owned), self.files.focus.clone());
        }
        if ctrl
            && self.modifiers.shift_key()
            && matches!(key, Key::Character(ch) if ch.eq_ignore_ascii_case("b"))
        {
            self.panels.visible = !self.panels.visible;
            return self.resize_panes();
        }
        if ctrl {
            if let Key::Character(ch) = key {
                let lower = ch.to_lowercase();
                if self.modifiers.shift_key() {
                    match lower.as_str() {
                        "t" => return self.new_tab(),
                        "d" => return self.split(Axis::Columns),
                        "e" => return self.split(Axis::Rows),
                        "w" => return self.close_pane(),
                        "a" if self.files.focus == TERMINALS => {
                            if let Some(id) = self.focused() {
                                let old =
                                    self.engine.agent_permission(id).map_err(|e| e.message)?;
                                self.engine
                                    .set_agent_permission(id, !old)
                                    .map_err(|e| e.message)?;
                            }
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                match lower.as_str() {
                    "=" | "+" => {
                        self.zoom.adjust(1.0);
                        return self.metrics_changed();
                    }
                    "-" => {
                        self.zoom.adjust(-1.0);
                        return self.metrics_changed();
                    }
                    "0" => {
                        self.zoom.reset();
                        return self.metrics_changed();
                    }
                    _ => {}
                }
            }
            if *key == Key::Named(NamedKey::Tab) {
                if !self.groups.is_empty() {
                    self.active = (self.active + 1) % self.groups.len();
                    self.files.focus_on(TERMINALS.into());
                    self.save_workspace()?;
                    self.reset_ime();
                }
                return Ok(());
            }
        }
        let modifiers = self.modifiers;
        let composing = self.input.composing();
        if let Some(pane) = self.files.editor() {
            pane.key(key, text, modifiers, composing)?;
            return Ok(());
        }
        self.send(&key_bytes(
            key,
            text,
            self.modifiers,
            self.input.composing(),
        ))
    }
    fn mouse_cell(&self) -> Result<(usize, usize), String> {
        let m = self.metrics()?;
        Ok(((self.mouse.0 / m.cell_width as f64) as usize, (self.mouse.1 / m.cell_height as f64) as usize))
    }
    fn click(&mut self) -> Result<(), String> {
        let (col, row) = self.mouse_cell()?;
        self.files.dragging = None;
        if self.compose.active_target().is_some() {
            self.compose_dragging = false;
            if let Some(rect) = self.compose_bounds() {
                if col >= rect.col && col < rect.col + rect.cols && row >= rect.row && row < rect.row + rect.rows {
                    self.compose.click(&rect, col, row, self.modifiers.shift_key())?;
                    self.compose_dragging = true;
                }
            }
            return Ok(());
        }
        if self.panels.open || self.files.modal() || self.search.active_target().is_some() { return Ok(()); }
        if row > 0 && col >= self.rect()?.cols {
            if row < self.full_rect()?.rows {
                self.palette_editor = Some(self.files.focus.clone());
                self.panels.click(&self.engine, row);
                if self.panels.open { self.reset_ime(); }
                self.local_request()?;
            }
            return Ok(());
        }
        if row == 0 {
            let index = col / 16;
            if index < self.groups.len() {
                self.active = index;
                self.files.focus_on(TERMINALS.into());
                self.reset_ime();
                self.save_workspace()?;
            }
            return Ok(());
        }
        let root = self.rect()?;
        let Some(local) = self.files.layout.hit_test(root.clone(), col, row) else { return Ok(()); };
        self.files.focus_on(local.clone());
        self.reset_ime();
        if local != TERMINALS {
            let rect = self.files.layout.rectangles(root).into_iter().find(|(id, _)| *id == local).unwrap().1;
            if let Some(pane) = self.files.panes.get_mut(&local) {
                pane.click(&rect, col, row, self.modifiers.shift_key())?;
                self.files.dragging = Some(local);
            }
            return Ok(());
        }
        let terminal_root = self.terminal_rect()?;
        let Some(group) = self.groups.get(self.active) else { return Ok(()); };
        let Some(id) = group.layout.hit_test(terminal_root.clone(), col, row) else { return Ok(()); };
        let rect = group.layout.rectangles(terminal_root).into_iter().find(|(leaf, _)| *leaf == id).unwrap().1;
        self.groups[self.active].focus = id.clone();
        self.save_workspace()?;
        let snapshot = self.engine.snapshot(&id).map_err(|e| e.message)?;
        let (y, x) = (row - rect.row, col - rect.col);
        let hit = window_tools::url_at(&snapshot, y, x);
        if window_tools::url_click(hit.as_ref(), self.modifiers.control_key(), |url| {
            self.url_opener.open(url).map_err(|_| "Could not open URL".to_owned())
        })? { return Ok(()); }
        if location_at(&snapshot, y, x).is_some() {
            if !self.local_files { return Err("Remote terminal file clicks are not supported".into()); }
            let cwd = self.cwd(Some(&id)).unwrap_or(None);
            let pane = EditorPane::from_click(&snapshot, y, x, cwd.as_deref())?;
            self.files.add(TERMINALS, pane, Axis::Columns, root)?;
            self.reset_ime();
            self.resize_panes()?;
        }
        Ok(())
    }
    fn drag(&mut self) -> Result<(), String> {
        if self.compose.active_target().is_some() {
            if self.compose_dragging {
                if let Some(rect) = self.compose_bounds() {
                    let (col, row) = self.mouse_cell()?;
                    self.compose.drag(&rect, col.clamp(rect.col, rect.col + rect.cols - 1),
                        row.clamp(rect.row, rect.row + rect.rows - 1))?;
                }
            }
            return Ok(());
        }
        if self.files.modal() || self.panels.open || self.search.active_target().is_some() { return Ok(()); }
        let Some(id) = self.files.dragging.clone() else { return Ok(()); };
        let (col, row) = self.mouse_cell()?;
        if let Some((_, rect)) = self.files.layout.rectangles(self.rect()?).into_iter().find(|(leaf, _)| *leaf == id) {
            // Keep captured selection in its own pane; gutters never activate controls.
            let col = col.clamp(rect.col, rect.col + rect.cols - 1);
            let row = row.clamp(rect.row, rect.row + rect.rows - 1);
            if let Some(pane) = self.files.panes.get_mut(&id) { pane.drag(&rect, col, row)?; }
        }
        Ok(())
    }
    fn scroll(&mut self, delta: MouseScrollDelta) -> Result<(), String> {
        if self.files.modal() || self.panels.open || self.search.active_target().is_some() { return Ok(()); }
        let m = self.metrics()?;
        let (col, row) = self.mouse_cell()?;
        let (columns, lines) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (-x * 3.0, -y * 3.0),
            MouseScrollDelta::PixelDelta(p) => (-p.x as f32 / m.cell_width as f32, -p.y as f32 / m.cell_height as f32),
        };
        if self.compose.active_target().is_some() {
            if let Some(rect) = self.compose_bounds() {
                if col >= rect.col && col < rect.col + rect.cols && row >= rect.row && row < rect.row + rect.rows {
                    self.compose.scroll(lines.round() as i64, columns.round() as i64)?;
                }
            }
            return Ok(());
        }
        if col >= self.rect()?.cols { self.panels.scroll(lines.round() as i32); return Ok(()); }
        let Some(local) = self.files.layout.hit_test(self.rect()?, col, row) else { return Ok(()); };
        if self.files.panes.contains_key(&local) {
            self.files.scroll(&local, row, lines.round() as i32, columns.round() as i32)?;
        } else if let Some(id) = self.groups.get(self.active).and_then(|g| g.layout.hit_test(self.terminal_rect().ok()?, col, row)) {
            self.engine.scroll(&id, -lines.round() as i32).map_err(|e| e.message)?;
        }
        Ok(())
    }
    fn draw(&mut self) -> Result<(), String> {
        if self.stop || self.is_suspended() {
            return Ok(());
        }
        self.resize_panes()?;
        let r = self.rect()?;
        let full = self.full_rect()?;
        let focused = self.focused().map(str::to_owned);
        if !self.engine.is_closed() { self.panels.refresh(&self.engine, focused.as_deref()); }
        let terminal_root = self.terminal_rect()?;
        let mut frame = TerminalSnapshot {
            rows: r.rows + 1,
            cols: full.cols,
            cells: vec![
                Cell {
                    ch: ' ',
                    fg: [220, 220, 220],
                    bg: [32, 32, 32]
                };
                (r.rows + 1) * full.cols
            ],
            cursor: None,
            images: Vec::new(),
        };
        for (index, group) in self.groups.iter().enumerate() {
            let label = format!(" {} {} ", index + 1, group.focus);
            for (offset, ch) in label.chars().take(16).enumerate() {
                if let Some(cell) = frame
                    .cells
                    .get_mut(index * 16 + offset)
                    .filter(|_| index * 16 + offset < r.cols)
                {
                    cell.ch = ch;
                    if index == self.active {
                        cell.bg = [50, 80, 120];
                        cell.fg = [255, 255, 255];
                    }
                }
            }
        }
        let mut underlines = Vec::new();
        let hover_enabled = self.compose.active_target().is_none() && self.search.active_target().is_none()
            && !self.panels.open && !self.files.modal();
        let (mouse_col, mouse_row) = self.mouse_cell()?;
        let mut search_rect = None;
        if let Some(group) = self.groups.get(self.active).filter(|_| !self.engine.is_closed()) {
            for (id, pane) in group.layout.rectangles(terminal_root) {
                if self.search.active_target() == Some(id.as_str()) {
                    search_rect = Some(pane);
                    continue;
                }
                let snapshot = match self.engine.snapshot(&id) {
                    Ok(s) => s,
                    Err(e) if e.code == "not_found" => continue,
                    Err(e) => return Err(e.message),
                };
                if hover_enabled && mouse_col >= pane.col && mouse_col < pane.col + pane.cols
                    && mouse_row >= pane.row && mouse_row < pane.row + pane.rows
                {
                    if let Some(hit) = window_tools::url_at(&snapshot, mouse_row - pane.row, mouse_col - pane.col) {
                        let start_col = pane.col + hit.start_col;
                        let end_col = pane.col + hit.end_col.min(pane.cols);
                        if start_col < end_col {
                            underlines.push(Underline { row: pane.row + hit.row, start_col, end_col,
                                color: [80, 160, 255] });
                        }
                    }
                }
                for row in 0..pane.rows.min(snapshot.rows) {
                    for col in 0..pane.cols.min(snapshot.cols) {
                        frame.cells[(pane.row + row) * frame.cols + pane.col + col] =
                            snapshot.cells[row * snapshot.cols + col].clone();
                    }
                }
                if id == group.focus && self.files.focus == TERMINALS {
                    frame.cursor = snapshot
                        .cursor
                        .filter(|(row, col)| *row < pane.rows && *col < pane.cols)
                        .map(|(row, col)| (pane.row + row, pane.col + col));
                }
                for mut image in snapshot.images {
                    image.x += pane.col;
                    image.y += pane.row;
                    image.clip_x = pane.col;
                    image.clip_y = pane.row;
                    image.clip_cols = pane.cols;
                    image.clip_rows = pane.rows;
                    frame.images.push(image);
                }
            }
        }
        self.files.draw(&mut frame, r.clone());
        self.panels.draw(&mut frame, self.panels.width(full.cols));
        if let Some(status) = &self.status {
            let row = frame.rows.saturating_sub(1);
            editor_workspace::paint(&mut frame, row, status);
        }
        // Search paints only its captured visible pane, never the newly focused pane.
        if self.search.active_target().is_some() {
            frame.cursor = None;
            if let Some(rect) = search_rect {
                frame.images.retain(|image| {
                    image.clip_x >= rect.col + rect.cols || image.clip_y >= rect.row + rect.rows
                        || image.clip_x.saturating_add(image.clip_cols) <= rect.col
                        || image.clip_y.saturating_add(image.clip_rows) <= rect.row
                });
                self.search.draw(&mut frame, &rect);
            } else {
                editor_workspace::paint(&mut frame, 0, "Search target is not visible; Escape to close");
            }
        }
        // Images render after cells; no placement may obscure a URL decoration.
        frame.images.retain(|image| {
            let left = image.x.max(image.clip_x);
            let right = image.x.saturating_add(image.cols).min(image.clip_x.saturating_add(image.clip_cols));
            let top = image.y.max(image.clip_y);
            let bottom = image.y.saturating_add(image.rows).min(image.clip_y.saturating_add(image.clip_rows));
            !underlines.iter().any(|line| left < right && top < bottom
                && left < line.end_col && right > line.start_col
                && top <= line.row && bottom > line.row)
        });
        // Paint compose last. Images render after cells, so discard overlapping placements.
        if let Some(target) = self.compose.active_target().map(str::to_owned) {
            if !self.groups.iter().any(|g| g.layout.contains(&target)) {
                self.status = Some("Compose target was closed; draft retained (Escape to close)".into());
            }
            if let Some(rect) = self.compose_bounds() {
                frame.images.retain(|image| {
                    image.clip_x >= rect.col + rect.cols || image.clip_y >= rect.row + rect.rows
                        || image.clip_x.saturating_add(image.clip_cols) <= rect.col
                        || image.clip_y.saturating_add(image.clip_rows) <= rect.row
                });
                underlines = self.compose.draw(&mut frame, &rect);
                if let Some(status) = &self.status {
                    editor_workspace::paint(&mut frame, full.row + full.rows - 1, status);
                }
            }
        }
        if let Some(window) = &self.window {
            let id = if self.files.focus == TERMINALS { self.focused().unwrap_or("no pane") } else { &self.files.focus };
            let allowed = self.files.focus == TERMINALS && !self.engine.is_closed() && self.engine.agent_permission(id).unwrap_or(false);
            let suffix = self.status.as_deref().unwrap_or("");
            window.set_title(&format!(
                "Clanky — {id} — {:.0} pt — agent {} {suffix}",
                self.zoom.points(),
                if allowed { "enabled" } else { "disabled" }
            ));
            if let Some((row, col)) = frame.cursor {
                let m = self.metrics()?;
                window.set_ime_cursor_area(
                    PhysicalPosition::new(col as u32 * m.cell_width, row as u32 * m.cell_height),
                    PhysicalSize::new(m.cell_width, m.cell_height),
                );
            }
        }
        self.renderer.as_mut().unwrap().draw_with_underlines(&frame, &underlines)
    }
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: String) {
        if self.has_local_work() { self.status = Some(error); return; }
        self.error = Some(error);
        event_loop.exit();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| {
            let base = self.zoom.metrics(1.0)?;
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("Clanky")
                            .with_inner_size(LogicalSize::new(
                                self.config.columns as f64 * base.cell_width as f64,
                                (self.config.rows + 1) as f64 * base.cell_height as f64,
                            )),
                    )
                    .map_err(|e| e.to_string())?,
            );
            self.scale = window.scale_factor();
            window.set_ime_allowed(true);
            self.renderer = Some(block_on(Renderer::new(window.clone()))?);
            self.window = Some(window);
            self.metrics_changed()?;
            if self.groups.is_empty() {
                self.new_tab()
            } else {
                Ok(())
            }
        })();
        if let Err(e) = result {
            self.fail(event_loop, e);
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let result = match event {
            WindowEvent::CloseRequested => {
                if self.files.dirty() {
                    self.status = Some("Unsaved files: save or undo before closing the window".into());
                } else { event_loop.exit(); }
                Ok(())
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
                Ok(())
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse = (position.x, position.y);
                if let Err(e) = self.drag() { self.status = Some(e); }
                Ok(())
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Err(e) = self.click() {
                    self.status = Some(e);
                }
                Ok(())
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                self.files.dragging = None;
                self.compose_dragging = false;
                Ok(())
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Err(e) = self.scroll(delta) { self.status = Some(e); }
                Ok(())
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Released && event.logical_key == Key::Named(NamedKey::Enter) => {
                self.compose_enter_held = false;
                Ok(())
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                // Hold the consumed chord until release: releasing Ctrl first must
                // not turn an auto-repeat after Send into ordinary PTY Enter.
                if event.repeat && self.compose_enter_held { return; }
                if event.logical_key == Key::Named(NamedKey::Enter) && self.modifiers.control_key()
                    && (self.compose.active_target().is_some() || self.modifiers.shift_key()) {
                    self.compose_enter_held = true;
                }
                if event.repeat
                    && (self.panels.open || self.files.modal())
                    && event.logical_key == Key::Named(NamedKey::Enter)
                {
                    return;
                }
                self.status = None;
                if let Err(e) = self.key(&event.logical_key, event.text.as_deref()) {
                    self.status = Some(e);
                }
                Ok(())
            }
            WindowEvent::Ime(event) => {
                if self.compose.active_target().is_some() {
                    match &event {
                        Ime::Commit(text) => {
                            if let Err(error) = self.compose.insert_text(text) { self.status = Some(error); }
                            self.compose.preedit("");
                        }
                        Ime::Preedit(text, _) => self.compose.preedit(text),
                        Ime::Disabled => self.compose.preedit(""),
                        _ => {}
                    }
                    self.input.on_ime(&event);
                    Ok(())
                } else if self.panels.open {
                    match &event {
                        Ime::Commit(text) => {
                            self.panels.text(text);
                            self.panels.preedit("");
                        }
                        Ime::Preedit(text, _) => self.panels.preedit(text),
                        _ => {}
                    }
                    self.input.on_ime(&event);
                    Ok(())
                } else if !self.files.modal() && self.search.active_target().is_some() {
                    if let Ime::Commit(text) = &event {
                        if let Err(error) = self.search.text(&self.engine, text) { self.status = Some(error); }
                    }
                    self.input.on_ime(&event);
                    Ok(())
                } else if self.files.modal() || self.files.focus != TERMINALS {
                    match &event {
                        Ime::Commit(text) => {
                            if let Err(error) = self.files.text(text) { self.status = Some(error); }
                            self.files.preedit("");
                        }
                        Ime::Preedit(text, _) => self.files.preedit(text),
                        _ => {}
                    }
                    self.input.on_ime(&event);
                    Ok(())
                } else if let Some(bytes) = self.input.on_ime(&event) {
                    if let Err(e) = self.send(&bytes) {
                        self.status = Some(e);
                    }
                    Ok(())
                } else {
                    Ok(())
                }
            }
            WindowEvent::Focused(false) => {
                self.input.focus_lost();
                self.compose.preedit("");
                self.compose_dragging = false;
                self.compose_enter_held = false;
                self.files.dragging = None;
                self.modifiers = ModifiersState::empty();
                Ok(())
            }
            WindowEvent::Focused(true) => {
                if let Some(pane) = self.files.editor() { let _ = pane.command("refresh", &json!({})); }
                Ok(())
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor;
                self.metrics_changed()
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
                if size.width > 0 && size.height > 0 {
                    self.resize_panes()
                } else {
                    Ok(())
                }
            }
            WindowEvent::Occluded(hidden) => {
                self.occluded = hidden;
                if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
                    let size = window.inner_size();
                    renderer.resize(
                        if hidden { 0 } else { size.width },
                        if hidden { 0 } else { size.height },
                    );
                }
                Ok(())
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => Ok(()),
        };
        if let Err(e) = result {
            self.fail(event_loop, e);
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.engine.is_closed() && self.has_local_work() {
            self.groups.clear();
            self.sizes.clear();
            self.clipboard_bridge.set_tabs(Vec::new());
            if self.status.is_none() { self.status = Some("Session connection closed; local editors and compose drafts retained".into()); }
        }
        if self.stop || (self.engine.is_closed() && !self.has_local_work()) {
            if !self.stop && self.error.is_none() {
                self.error =
                    Some("Session connection closed; sessions may still be running".into());
            }
            event_loop.exit();
            return;
        }
        if Instant::now() >= self.next_frame {
            if self.url_opener.poll().is_err() {
                self.status = Some("URL opener failed".into());
            }
            while let Some(error) = self.clipboard_bridge.take_error() {
                self.status = Some(error);
            }
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            self.next_frame = Instant::now() + Duration::from_millis(16);
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}
