mod studio;
mod settings_window;
mod panels;
mod editor_workspace;
use editor_workspace::{EditorWorkspace, TERMINALS};
use clanky::editor::{EditorPane, location_at};
mod tui;

use clanky::{
    config::{Config, Metrics, Zoom},
    compose::{Compose, ComposeEvent},
    clipboard::ClipboardBridge,
    scrollback_search::Search,
    window_tools,
    input::{InputState, key_bytes},
    layout_store,
    notifications::NotificationTracker,
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "edit") {
        return tui::run(&args[1..]);
    }
    if args.first().is_some_and(|arg| arg == "--polish-offscreen-window") {
        if std::env::var_os("CLANKY_GATE_OFFSCREEN").as_deref() != Some(std::ffi::OsStr::new("1")) || args.len() != 3 {
            return Err("invalid offscreen gate invocation".into());
        }
        let client = clanky::session::prepare_window(std::path::Path::new(&args[1]), Some(std::path::Path::new(&args[2])))?;
        println!("READY");
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).map_err(|e| e.to_string())?;
        drop(client);
        return Ok(());
    }
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
                    "clanky [--server|--headless|--stdio] [--socket PATH] [--config PATH]\nclanky attach HOST [--config PATH]\nclanky edit FILE[:LINE[:COL]] [--review FILE]\nCtrl+Shift+T: new tab; Ctrl+Shift+D/E: split right/below\nCtrl+Shift+W: close pane; Ctrl+Tab: next tab; click: focus pane\nCtrl+=/-/0: zoom/reset; Ctrl+Shift+A: toggle focused pane agent access\nCtrl+Shift+P: palette (Open file / Load changes / Compose); Ctrl+Shift+B: sidebar\nCtrl+Shift+O: open file; click path:line: open beside terminal\nEditor: Ctrl+S save; Ctrl+Z/Shift+Z undo/redo; Ctrl+F find\nEditor: Ctrl+Alt+A/R accept/reject (Shift: all); dirty close refuses\nCompose: Ctrl+Shift+Enter opens focused terminal; Enter newline; Ctrl+Enter sends\nCompose: Escape retains draft; F7 suggestions; Ctrl+Alt+A add word\nCompose drafts are memory-only for this window; closing it can lose them\nCtrl+Shift+Escape: revoke agents (works everywhere)\nclanky mcp [--socket PATH]: agent-only MCP"
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
        return run_window(client, config, false, None, config_path.or_else(clanky::platform::settings_path));
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
    let persistence = layout_store::default_path().map(|path| (path, socket.clone()));
    let client = clanky::session::prepare_window(&socket, persistence.as_ref().filter(|_|config.restore_layout).map(|(path, _)| path.as_path()))?;
    run_window(client, config, true, persistence, config_path.or_else(clanky::platform::settings_path))
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
    compose_dragging: bool,
    compose_enter_held: bool,
    search: Search,
    url_opener: platform::UrlOpener,
    clipboard_bridge: ClipboardBridge,
    notifications: NotificationTracker,
    window_focused: bool,
    published_focus: Option<Option<String>>,
    next_polish: Instant,
    persistence: Option<(PathBuf, PathBuf)>,
    studio: studio::Studio,
    settings: Option<clanky::settings_panel::SettingsPanel>,
    settings_error: Option<String>,
    next_settings: Instant,
}

fn run_window(engine: Client, config: Config, local_files: bool, persistence: Option<(PathBuf, PathBuf)>, settings_path: Option<PathBuf>) -> Result<(), String> {
    let (settings,settings_error)=match settings_path.map(clanky::settings::Settings::open) {
        Some(Ok(store))=>(Some(clanky::settings_panel::SettingsPanel::new(store)),None),
        Some(Err(error))=>(None,Some(error)),
        None=>(None,Some("No settings path is available".into())),
    };
    let workspace = engine.workspace_get().map_err(|e| e.message)?;
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let clipboard_bridge = ClipboardBridge::new(engine.clone(), config.osc52_write, config.osc52_read);
    clipboard_bridge.set_tabs(workspace.groups.iter().flat_map(|g| g.layout.leaves()).collect());
    let rails = config.rails;
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
        panels: panels::Panels::new(rails),
        files: EditorWorkspace::default(),
        local_files,
        palette_editor: None,
        notifications: NotificationTracker::new(),
        window_focused: false,
        published_focus: None,
        next_polish: Instant::now(),
        persistence,
        studio: studio::Studio::default(),
        settings,settings_error,next_settings:Instant::now(),
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
        self.full_rect()
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
        if self.files.focus != TERMINALS { return Err("Focus a terminal to open Compose; a file editor is focused".into()); }
        let target = target.ok_or("No focused terminal for Compose")?;
        let layout = self.pixel_layout()?;
        let pane = layout.pane(target).ok_or("Compose target is not visible")?;
        if pane.rows < 3 || pane.cols < 8 || pane.grid_clip.h < layout.metrics.compose_min_height as f32 + 3.0*layout.cell_px[1] as f32 { return Err("Compose needs room for its dock and at least 3 terminal rows".into()); }
        self.compose.open(target)?;
        self.compose_dragging = false;
        self.reset_ime();
        self.resize_panes()
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
        if self.revision != workspace.revision { self.studio.drag = None; }
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
    fn persist_layout(&self) -> Result<(), String> {
        if !self.config.restore_layout {return Ok(());}
        if let Some((path, socket)) = &self.persistence {
            let _guard = platform::StartupLock::acquire(socket).map_err(|e| e.to_string())?;
            layout_store::save(&self.engine, path)?;
        }
        Ok(())
    }
    fn publish_focus(&mut self) -> Result<(), String> {
        let focus = if self.window_focused { self.focused().map(str::to_owned) } else { None };
        if self.published_focus.as_ref() != Some(&focus) {
            self.call("v3.focus_set", json!({"tab_id":focus}))?;
            self.published_focus = Some(focus);
        }
        Ok(())
    }
    fn poll_session(&mut self) -> Result<(), String> {
        self.sync_groups()?;
        self.publish_focus()?;
        let ids: Vec<_> = self.groups.iter().flat_map(|g| g.layout.leaves()).collect();
        let focused = self.focused().map(str::to_owned);
        self.notifications.poll_client(&self.engine, &ids, self.config.notify_after_seconds, self.window_focused, focused.as_deref())?;
        self.persist_layout()
    }
    fn is_suspended(&self) -> bool {
        self.occluded
            || self.window.as_ref().is_none_or(|window| {
                let size = window.inner_size();
                size.width == 0 || size.height == 0
            })
    }
    fn resize_panes(&mut self) -> Result<(), String> {
        if self.is_suspended() || self.engine.is_closed() { return Ok(()); }
        self.sync_groups()?;
        let layout = self.pixel_layout()?;
        self.files.viewport = Some(layout.clone());
        if let Some(window) = &self.window {
            window.set_min_inner_size(Some(PhysicalSize::new(layout.minimum_window_px[0], layout.minimum_window_px[1])));
        }
        let layouts=self.groups.iter().map(|g|self.pixel_layout_for(&g.layout)).collect::<Result<Vec<_>,_>>()?;
        for layout in layouts {
            for pane in &layout.panes {
                if self.files.panes.contains_key(&pane.id) { continue; }
                if pane.rows == 0 || pane.cols == 0 || self.sizes.get(&pane.id) == Some(&(pane.rows, pane.cols)) { continue; }
                match self.engine.resize(&pane.id, pane.rows, pane.cols) {
                    Ok(()) => { self.sizes.insert(pane.id.clone(), (pane.rows, pane.cols)); }
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
        let layout=self.layout_with(&clanky_core::layout::Layout::leaf("__new__"))?;
        let r=layout.pane("__new__").ok_or("No room for a terminal pane")?;
        if r.rows<3 || r.cols<8 { return Err("New terminal needs at least 3 rows and 8 columns".into()); }
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
        let proposed = self.layout_with(&candidate)?;
        if !proposed.fits_minimum || proposed.panes.iter().any(|p| p.rows < 3 || p.cols < 8) {
            return Err("Split needs at least 3 rows and 8 columns in every pane".into());
        }
        let r = proposed.pane("__new__").ok_or("new pane is not visible")?;
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
        let proposed=self.layout_with(&layout)?;
        if !proposed.fits_minimum || proposed.panes.iter().any(|p|p.rows<3 || p.cols<8) {
            return Err("Workspace changed: split no longer fits; new terminal remains independent".into());
        }
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
                if let Err(e) = self.persist_layout() { self.status = Some(e); }
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
        if self.modifiers.control_key() && matches!(key,Key::Character(ch) if ch==",") {return self.toggle_settings();}
        if self.settings_open() {
            if self.modifiers.control_key() {
                if matches!(key,Key::Character(ch) if ch.eq_ignore_ascii_case("a")) {self.settings.as_mut().unwrap().select_all();return Ok(());}
                if matches!(key,Key::Character(ch) if ch.eq_ignore_ascii_case("v")) {
                    match platform::clipboard_read() {
                        Ok(bytes)=>{if let Ok(text)=std::str::from_utf8(&bytes){self.settings.as_mut().unwrap().insert(text);}},
                        Err(error)=>self.settings.as_mut().unwrap().error=Some(error.to_string()),
                    }
                    return Ok(());
                }
            }
            let composing=self.input.composing();
            let effect=self.settings.as_mut().unwrap().key(key,text,composing);
            return self.settings_effect(effect);
        }
        if self.card_key(key)? { return Ok(()); }
        if self.modifiers.control_key() && self.modifiers.shift_key()
            && matches!(key, Key::Character(ch) if ch.eq_ignore_ascii_case("b"))
        {
            self.panels.visible = !self.panels.visible;
            self.studio.drag = None;
            return self.resize_panes();
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
                    clanky::compose::send_chunked(bytes, clanky::compose::INPUT_CHUNK, |piece| {
                        engine.input(target, piece).map_err(|e| e.message)
                    })
                })
            } else { Ok(()) };
            if self.compose.active_target().is_none() {
                self.compose_dragging = false;
                self.reset_ime();
                self.resize_panes()?;
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
    fn click(&mut self) -> Result<(), String> {
        if self.settings_open() && self.settings_button_hit(){return self.toggle_settings();}
        if self.settings_open() {let effect=self.settings.as_mut().unwrap().click(self.mouse.0,self.mouse.1);self.settings_effect(effect)} else {self.click_studio()}
    }
    fn drag(&mut self) -> Result<(), String> {
        if self.settings_open() {let effect=self.settings.as_mut().unwrap().drag(self.mouse.0);self.settings_effect(effect)} else {self.drag_studio()}
    }
    fn scroll(&mut self, delta: MouseScrollDelta) -> Result<(), String> {
        if self.settings_open() {
            let lines=match delta {MouseScrollDelta::LineDelta(_,y)=>y.round() as i32,MouseScrollDelta::PixelDelta(p)=>(p.y/20.0).round() as i32};
            self.settings.as_mut().unwrap().wheel(lines);Ok(())
        } else {self.scroll_studio(delta)}
    }
    fn draw(&mut self) -> Result<(), String> { self.draw_studio() }
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
                            .with_transparent(true)
                            .with_inner_size(LogicalSize::new(
                                self.config.columns as f64 * base.cell_width as f64,
                                (self.config.rows + 1) as f64 * base.cell_height as f64,
                            )),
                    )
                    .map_err(|e| e.to_string())?,
            );
            self.scale = window.scale_factor();
            self.studio.theme = self.config.resolved_theme(clanky::platform::system_light(&window));
            window.set_ime_allowed(true);
            let mut renderer = block_on(Renderer::new(window.clone()))?;
            renderer.set_font_families(&self.config.font_family, &self.config.ui_font_family)
                .map_err(|error| format!("Configuration: {error}"))?;
            if !renderer.supports_transparency() {
                self.status = Some("Compositor transparency unavailable; using opaque background".into());
            }
            self.renderer = Some(renderer);
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
            WindowEvent::ThemeChanged(theme) => {
                self.studio.theme = self.config.resolved_theme(clanky::platform::preference_is_light(Some(theme)));
                if let Some(window) = &self.window { window.request_redraw(); }
                Ok(())
            }
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
            WindowEvent::CursorLeft { .. } => { self.mouse=(-1.0,-1.0); if let Some(panel)=&mut self.settings{panel.release();} Ok(()) }
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
                if let Some(panel)=&mut self.settings {panel.release();}
                self.files.dragging = None;
                self.compose_dragging = false;
                self.studio.drag = None;
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
                if event.repeat && self.card_focused() { return; }
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
                if self.settings_open() {
                    match &event {
                        Ime::Commit(text)=>self.settings.as_mut().unwrap().insert(text),
                        Ime::Preedit(text,_)=>self.settings.as_mut().unwrap().preedit(text),
                        _=>(),
                    }
                    let _=self.input.on_ime(&event);
                    return;
                }
                if self.card_focused() { return; }
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
            WindowEvent::Focused(focused) => {
                self.window_focused = focused;
                if focused {
                    if let Some(pane) = self.files.editor() { let _ = pane.command("refresh", &json!({})); }
                } else {
                    self.input.focus_lost();
                    self.compose.preedit("");
                    self.compose_dragging = false;
                    self.compose_enter_held = false;
                    self.files.dragging = None;
                    self.studio.drag = None;
                    self.modifiers = ModifiersState::empty();
                }
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
        } else if let Err(e) = self.publish_focus() {
            self.status = Some(e);
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
        if Instant::now() >= self.next_settings {
            self.poll_settings();
            self.next_settings=Instant::now()+Duration::from_millis(500);
        }
        if Instant::now() >= self.next_polish {
            if let Err(e) = self.poll_session() { self.status = Some(e); }
            self.next_polish = Instant::now() + Duration::from_secs(1);
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
