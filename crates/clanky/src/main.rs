mod studio;
mod robot_window;
mod hello;
mod settings_window;
mod terminal_mouse;
mod terminal_menu;
mod window_events;
mod panels;
mod editor_workspace;
use editor_workspace::{EditorWorkspace, TERMINALS};
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "history") {
        std::process::exit(clanky::history_cli::main(&args[1..]));
    }
    if args.first().is_some_and(|arg| arg == "terminal-colors") {
        std::process::exit(match args.get(1..).unwrap_or_default() {
            [import, path] if import == "import" => match std::fs::read_to_string(path).map_err(|e| e.to_string())
                .and_then(|text| clanky::config::import_ptyxis_palette(&text)) {
                Ok(colors) => { println!("{}", serde_json::to_string_pretty(&colors).unwrap_or_default()); 0 }
                Err(error) => { eprintln!("clanky terminal-colors: {path}: {error}"); 1 }
            },
            _ => { eprintln!("clanky terminal-colors import FILE.palette: print a Ptyxis palette's [Dark] colours as \"terminal_colors\" JSON"); 2 }
        });
    }
    if args.first().is_some_and(|arg| arg == "agent") {
        std::process::exit(clanky::agent_cli::main(&args[1..]));
    }
    if let Err(error) = run() {
        if error == clanky_core::POST_AI_REASON {
            eprintln!("{error}");
            std::process::exit(2);
        }
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
    let mut hello = false;
    let mut host = None;
    let mut startup_flags = clanky::config::StartupOverrides::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--headless" | "--server" => headless = true,
            "--stdio" => stdio = true,
            "mcp" => mcp = true,
            "+hello" => hello = true,
            "attach" => host = Some(args.next().ok_or("attach needs a host")?),
            "--socket" => socket = Some(PathBuf::from(args.next().ok_or("--socket needs a path")?)),
            "--config" => {
                config_path = Some(PathBuf::from(args.next().ok_or("--config needs a path")?))
            }
            "--help" | "-h" => {
                println!(
                    "clanky [--server|--headless|--stdio] [--socket PATH] [--config PATH] [--post-ai]\nclanky history [path|list|search|show|export|delete|clear]: saved command history\nclanky agent [--tab] NAME [DIR]: start an agent tool in a new pane (--list shows them)\nclanky terminal-colors import FILE.palette: print a Ptyxis palette as terminal_colors JSON\nclanky attach HOST [--config PATH]\nclanky +hello [--config PATH]: replay the robot in its own window\nclanky edit FILE[:LINE[:COL]] [--review FILE]\nCtrl+Shift+T: new tab; Ctrl+Shift+D/E: split right/below\nCtrl+Shift+W: close pane; Ctrl+Tab: next tab; click: focus pane\nCtrl+=/-/0: zoom/reset; Ctrl+Shift+A: toggle focused pane agent access; Ctrl+Shift+G: start an agent\nCtrl+Shift+P: palette (Open file / Load changes / Compose); Ctrl+Shift+B: sidebar\nCtrl+Shift+O: open file; click path:line: open beside terminal\nEditor: Ctrl+S save; Ctrl+Z/Shift+Z undo/redo; Ctrl+F find\nEditor: Ctrl+Alt+A/R accept/reject (Shift: all); dirty close refuses\nCompose: Ctrl+Shift+Enter opens focused terminal; Enter newline; Ctrl+Enter sends\nCompose: Escape retains draft; F7 suggestions; Ctrl+Alt+A add word\nCompose drafts are memory-only for this window; closing it can lose them\nCtrl+Shift+Escape: revoke agents (works everywhere)\nclanky mcp [--socket PATH]: agent-only MCP"
                );
                return Ok(());
            }
            _ if startup_flags.parse_flag(&arg) => (),
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    let mut config = Config::load(config_path.as_deref())?;
    startup_flags.apply(&mut config);
    if mcp && config.post_ai { return Err(clanky_core::POST_AI_REASON.into()); }
    if hello {
        if host.is_some() || socket.is_some() || headless || stdio || mcp {
            return Err("+hello is a standalone window; it cannot use server, socket, attach, stdio or MCP mode".into());
        }
        return hello::run(config);
    }
    if host.is_some() && (socket.is_some() || headless || stdio || mcp) {
        return Err("attach cannot use --socket, --server, --headless, --stdio or mcp".into());
    }
    if u8::from(headless) + u8::from(stdio) + u8::from(mcp) > 1 {
        return Err("choose server, stdio or MCP mode".into());
    }
    if let Some(host) = host {
        let client = Client::connect_ssh(&host).map_err(|e| e.to_string())?;
        return run_window(client, config, false, None, config_path.or_else(clanky::platform::settings_path), startup_flags);
    }
    let socket = match socket {
        Some(path) => path,
        None => platform::default_socket().map_err(|e| e.to_string())?,
    };
    if headless {
        return clanky::session::run_server_configured(&socket, config_path.as_deref(), startup_flags);
    }
    if stdio {
        return clanky::session::stdio_configured(&socket, config_path.as_deref(), startup_flags);
    }
    if mcp {
        return clanky::mcp::serve(&socket);
    }
    let persistence = layout_store::default_path().map(|path| (path, socket.clone()));
    let client = clanky::session::prepare_window_configured(&socket, persistence.as_ref().filter(|_|config.restore_layout).map(|(path, _)| path.as_path()), config_path.as_deref(), startup_flags)?;
    run_window(client, config, true, persistence, config_path.or_else(clanky::platform::settings_path), startup_flags)
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
    minimum_window: Option<[u32;2]>,
    modifiers: ModifiersState,
    mouse: (f64, f64),
    pointer_pressed: bool,
    term_mouse: terminal_mouse::TermMouse,
    wheel_releases: u8,
    input: InputState,
    occluded: bool,
    error: Option<String>,
    status: Option<String>,
    frame_clock: clanky::frame_clock::FrameClock,
    window_events: Option<window_events::WindowEvents>,
    fallback_poll: bool,
    terminal_dirty: bool,
    persist_dirty: bool,
    sent_input: bool,
    file_stamps: BTreeMap<PathBuf,(Option<std::time::SystemTime>,u64)>,
    frame_trace: clanky::frame_trace::FrameTrace,
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
    settings: Option<clanky::calm::settings_panel::SettingsPanel>,
    palette: clanky::calm::palette::Palette,
    agent_picker: clanky::calm::agent_picker::AgentPicker,
    context_menu: clanky::calm::context_menu::ContextMenu,
    /// The terminal font's [advance, ascent, descent] in em, once the renderer has loaded it.
    terminal_face: Option<[f32; 3]>,
    /// Cursor blinking: resolved settings, GNOME's values, the last restart, and the state last drawn.
    blink: clanky::cursor_blink::Blink,
    gnome_blink: Option<(bool, u32, u32)>,
    blink_start: Instant,
    blink_shown: bool,
    /// The agent tool the next new terminal starts (set only while start_agent runs).
    pending_agent: Option<clanky::config::AgentTool>,
    settings_error: Option<String>,
    next_settings: Instant,
    startup_flags: clanky::config::StartupOverrides,
    applied_post_ai: bool,
    theme_runtime: Option<clanky::theme_runtime::ThemeRuntime>,
    startup: Option<clanky::robot::Startup>,
    gate_clock: Option<clanky::robot_gate_clock::RobotGateClock>,
}

fn run_window(engine: Client, mut config: Config, local_files: bool, persistence: Option<(PathBuf, PathBuf)>, settings_path: Option<PathBuf>, startup_flags: clanky::config::StartupOverrides) -> Result<(), String> {
    startup_flags.apply(&mut config);
    let policy = engine.exchange(4, "post_ai", &json!({"enabled":config.post_ai,"force":startup_flags.post_ai})).map_err(|e| e.message)?;
    config.post_ai = policy["post_ai"].as_bool().ok_or("invalid Post-AI reply")?;
    let applied_post_ai = config.post_ai;
    let (theme_runtime, theme_error) = match clanky::theme_runtime::ThemeRuntime::open() {
        Ok(runtime) => (Some(runtime), None), Err(error) => (None, Some(error)),
    };
    let (settings,settings_error)=match settings_path.map(clanky::settings::Settings::open) {
        Some(Ok(store))=>(Some(clanky::calm::settings_panel::SettingsPanel::new(store)),None),
        Some(Err(error))=>(None,Some(error)),
        None=>(None,Some("No settings path is available".into())),
    };
    let workspace = engine.workspace_get().map_err(|e| e.message)?;
    // Only a local server can run the same executable as this window.
    let build_notice = if local_files { clanky::session::server_build_notice(&engine) } else { None };
    let event_loop = EventLoop::<window_events::WindowNotice>::with_user_event().build().map_err(|e| e.to_string())?;
    let (window_events,feed_error)=match window_events::WindowEvents::start(&engine,event_loop.create_proxy()) {
        Ok(events)=>(Some(events),None),Err(error)=>(None,Some(error)),
    };let fallback_poll=feed_error.is_some();
    let clipboard_bridge = ClipboardBridge::new(engine.clone(), config.osc52_write, config.osc52_read);
    clipboard_bridge.set_tabs(workspace.groups.iter().flat_map(|g| g.layout.leaves()).collect());
    let rails = config.rails;
    let gnome_blink = clanky::platform::gnome_cursor_blink();
    let blink = clanky::cursor_blink::Blink::from_config(&config, gnome_blink);
    let gate_clock = clanky::robot_gate_clock::RobotGateClock::from_env()?;
    let force_redraw = gate_clock.as_ref().is_some_and(|clock| clock.force_redraw());
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
        minimum_window: None,
        modifiers: ModifiersState::empty(),
        mouse: (0.0, 0.0),
        pointer_pressed: false,
        term_mouse: Default::default(),
        wheel_releases: 0,
        input: InputState::default(),
        occluded: false,
        error: None,
        status: feed_error.or(theme_error).or(build_notice),
        frame_clock: clanky::frame_clock::FrameClock::new(force_redraw),
        window_events,fallback_poll,terminal_dirty:true,persist_dirty:true,sent_input:false,file_stamps:BTreeMap::new(),
        frame_trace: clanky::frame_trace::FrameTrace::from_env()?,
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
        settings,settings_error,next_settings:Instant::now(),palette:Default::default(),agent_picker:Default::default(),context_menu:Default::default(),terminal_face:None,blink,gnome_blink,blink_start:Instant::now(),blink_shown:true,pending_agent:None,
        startup_flags, applied_post_ai, theme_runtime,
        startup: None,
        gate_clock,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.error.map_or(Ok(()), Err)
}

impl App {
    fn sync_post_ai(&mut self) {
        self.startup_flags.apply(&mut self.config);
        if self.config.post_ai != self.applied_post_ai {
            let requested=self.config.post_ai;
            match self.engine.exchange(4, "post_ai", &json!({"enabled":self.config.post_ai})) {
                Ok(reply) => {
                    self.applied_post_ai = reply["post_ai"].as_bool().unwrap_or(true);
                    self.config.post_ai = self.applied_post_ai;
                    if !requested && self.applied_post_ai{if let Some(panel)=&mut self.settings{panel.error=Some("The server keeps Post-AI on until restarted.".into());}}
                    self.studio.drag=None;
                    if let Err(error)=self.resize_panes(){self.status=Some(error);}
                }
                Err(error) => { self.config.post_ai=self.applied_post_ai;self.status = Some(error.message); }
            }
            self.frame_clock.invalidate();
        }
    }
    fn call(&self, op: &str, args: Value) -> Result<Value, String> {
        self.engine.request(op, &args, true).map_err(|e| e.message)
    }
    /// Whether the focused pane's cursor shows now (always, when unfocused or not blinking).
    fn cursor_visible(&self) -> bool {
        !self.window_focused || self.blink.visible(self.blink_start.elapsed())
    }
    fn metrics(&self) -> Result<Metrics, String> {
        clanky::calm::font_metrics(&self.zoom,self.scale,self.terminal_face,[self.config.cell_width_scale,self.config.cell_height_scale])
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
        self.reset_ime_position();
        self.input.focus_lost();
        self.compose.preedit("");
        if let Some(w) = &self.window {
            w.set_ime_allowed(false);
            w.set_ime_allowed(true);
        }
    }
    fn load_workspace(&mut self, workspace: Workspace) {
        self.frame_clock.invalidate();self.terminal_dirty=true;self.persist_dirty=true;
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
    /// A pane closes when the shell the window started in it exits, as in Ptyxis.
    fn close_finished_shells(&mut self) -> Result<(), String> {
        if self.engine.is_closed() { return Ok(()); }
        let finished = self.engine.exchange(3, "finished_shells", &json!({})).map_err(|e| e.message)?;
        for id in finished["tab_ids"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if self.stop { break; }
            self.sync_groups()?;
            if !self.groups.iter().any(|g| g.layout.leaves().iter().any(|leaf| leaf == id)) { continue; }
            if self.context_menu.pane == id { self.context_menu.close(); }
            self.close_terminal(Some(id.to_owned()))?;
            self.frame_clock.invalidate();
        }
        Ok(())
    }
    fn poll_session(&mut self) -> Result<(), String> {
        self.sync_groups()?;
        self.close_finished_shells()?;
        if self.stop { return Ok(()); }
        self.publish_focus()?;
        let ids: Vec<_> = self.groups.iter().flat_map(|g| g.layout.leaves()).collect();
        let focused = self.focused().map(str::to_owned);
        self.notifications.poll_client(&self.engine, &ids, self.config.notify_after_seconds, self.window_focused, focused.as_deref())?;
        if self.persist_dirty {self.persist_layout()?;self.persist_dirty=false;}
        Ok(())
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
        if self.minimum_window!=Some(layout.minimum_window_px) {
            if let Some(window)=&self.window {
                window.set_min_inner_size(Some(PhysicalSize::new(layout.minimum_window_px[0],layout.minimum_window_px[1])));
                self.minimum_window=Some(layout.minimum_window_px);
            }
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
        match &self.pending_agent {
            Some(tool) => clanky::agent_launch::start(&self.engine, tool, focused, None, self.config.agent_pane_access, rows, cols),
            None => window_tools::new_terminal(&self.engine, focused, rows, cols),
        }
    }
    /// Start a configured agent tool in a split to the right of the focused pane, or a new tab.
    fn start_agent(&mut self, index: usize, tab: bool) -> Result<(), String> {
        if self.applied_post_ai { return Ok(()); }
        let tool = self.config.agents.get(index).cloned().ok_or("That agent is no longer configured")?;
        if !clanky::agent_launch::installed(&tool.command, std::env::var_os("PATH").as_deref()) {
            return Err(format!("{} is not installed: {} is not on PATH", tool.name, clanky::agent_launch::program(&tool.command)));
        }
        self.pending_agent = Some(tool);
        let result = if tab || self.files.focus != TERMINALS { self.new_tab() } else { self.split(Axis::Columns) };
        self.pending_agent = None;
        result
    }
    fn agent_context(&self) -> clanky::calm::agent_picker::Context {
        let path = std::env::var_os("PATH");
        let tools = self.config.agents.iter().map(|tool| clanky::calm::agent_picker::Choice {
            name: tool.name.clone(), command: tool.command.clone(),
            found: clanky::agent_launch::installed(&tool.command, path.as_deref()) }).collect();
        let folder = self.focused().and_then(|id| self.studio.terminal_info.get(id)).and_then(|info| info["cwd"].as_str())
            .map(|cwd| match clanky_core::platform::home_dir().and_then(|home| std::path::Path::new(cwd).strip_prefix(home).ok().map(|rest| rest.to_owned())) {
                Some(rest) if rest.as_os_str().is_empty() => "~".into(),
                Some(rest) => format!("~/{}", rest.display()),
                None => cwd.to_owned(),
            }).unwrap_or_else(|| "the focused pane's folder".into());
        clanky::calm::agent_picker::Context { tools, folder, access: self.config.agent_pane_access }
    }
    fn open_agent_picker(&mut self) {
        if self.applied_post_ai { return; }
        if self.settings_open() { let _ = self.toggle_settings(); }
        self.palette.close();
        let context = self.agent_context();
        self.agent_picker.show(&context);
        self.reset_ime();
    }
    fn agent_picker_effect(&mut self, effect: clanky::calm::agent_picker::Effect) -> Result<(), String> {
        match effect {
            clanky::calm::agent_picker::Effect::Start { index, tab } => { self.reset_ime(); self.start_agent(index, tab) }
            clanky::calm::agent_picker::Effect::Close => { self.reset_ime(); Ok(()) }
            clanky::calm::agent_picker::Effect::None => Ok(()),
        }
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
        self.close_terminal(target)
    }
    /// Close one terminal pane, its tab when it was the tab's last pane, and after the last
    /// pane the window and its server.
    fn close_terminal(&mut self, target: Option<String>) -> Result<(), String> {
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
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.search.active_target().is_some() || self.compose.active_target().is_some() || self.files.focus != TERMINALS || self.files.modal() || self.panels.open { return Ok(()); }
        if !bytes.is_empty() {
            if let Some(id) = self.focused() {
                self.engine.input(id, bytes).map_err(|e| e.message)?;
                self.sent_input=true;
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
        // An open menu takes every key: Up, Down, Enter, Escape; the rest do nothing.
        if self.context_menu.open {
            let effect = self.context_menu.key(key);
            return self.menu_effect(effect);
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
        if self.modifiers.control_key() && self.modifiers.shift_key() && matches!(key,Key::Character(ch) if ch.eq_ignore_ascii_case("p")) {
            if self.palette.open {self.palette.close();} else {self.open_palette();}
            self.reset_ime();return Ok(());
        }
        if self.palette.open {
            let context=self.palette_context();
            let effect=self.palette.key(key,text,self.input.composing(),&context);
            return self.palette_effect(effect);
        }
        if self.modifiers.control_key() && self.modifiers.shift_key() && matches!(key,Key::Character(ch) if ch.eq_ignore_ascii_case("g")) {
            if self.agent_picker.open {self.agent_picker.close();self.reset_ime();} else {self.open_agent_picker();}
            return Ok(());
        }
        if self.agent_picker.open {
            let context=self.agent_context();
            let effect=self.agent_picker.key(key,&context);
            return self.agent_picker_effect(effect);
        }
        if self.modifiers.control_key() && self.modifiers.shift_key() && *key == Key::Named(NamedKey::F6) {
            self.cycle_card_focus();
            return Ok(());
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
                        "c" if self.files.focus == TERMINALS => return self.copy_selection(),
                        "v" if self.files.focus == TERMINALS => {
                            let Some(id) = self.focused().map(str::to_owned) else { return Ok(()); };
                            return self.paste_selection(&id, false);
                        }
                        "a" if self.files.focus == TERMINALS && !self.applied_post_ai => {
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
        let app_cursor = self.focused().is_some_and(|id| self.app_cursor(id));
        self.send(&key_bytes(
            key,
            text,
            self.modifiers,
            self.input.composing(),
            app_cursor,
        ))
    }
    fn click(&mut self) -> Result<(), String> {
        // A click outside the menu only closes it.
        if self.context_menu.open {
            let effect = self.context_menu.click(self.mouse.0, self.mouse.1);
            return self.menu_effect(effect);
        }
        if self.agent_picker.open {
            let context=self.agent_context();
            let effect=self.agent_picker.click(self.mouse.0,self.mouse.1,&context);
            return self.agent_picker_effect(effect);
        }
        if self.palette.open {
            let context=self.palette_context();
            let effect=self.palette.click(self.mouse.0,self.mouse.1,&context);
            return self.palette_effect(effect);
        }
        if self.settings_open() && self.settings_button_hit(){return self.toggle_settings();}
        if self.settings_open() {let effect=self.settings.as_mut().unwrap().click(self.mouse.0,self.mouse.1);self.settings_effect(effect)} else {self.click_studio()}
    }
    fn drag(&mut self) -> Result<(), String> {
        if self.settings_open() {let effect=self.settings.as_mut().unwrap().drag(self.mouse.0);self.settings_effect(effect)} else {self.drag_studio()}
    }
    fn scroll(&mut self, delta: MouseScrollDelta) -> Result<(), String> {
        if self.palette.open {
            let lines=match delta {MouseScrollDelta::LineDelta(_,y)=>y.round() as i32,MouseScrollDelta::PixelDelta(p)=>(p.y/32.0).round() as i32};
            self.palette.wheel(lines);return Ok(());
        }
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

impl ApplicationHandler<window_events::WindowNotice> for App {
    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _device: winit::event::DeviceId, event: winit::event::DeviceEvent) {
        if let winit::event::DeviceEvent::Button { button: 4..=7, state: ElementState::Released } = event {
            self.wheel_releases = self.wheel_releases.saturating_add(1).min(8);
        }
    }
    fn user_event(&mut self,_event_loop:&ActiveEventLoop,event:window_events::WindowNotice){
        match event {
            window_events::WindowNotice::Changed=>{
                if let Some(events)=&self.window_events{events.received();}
                let revision=self.revision;
                if let Err(error)=self.sync_groups(){self.status=Some(error);}
                if revision!=self.revision{if let Err(error)=self.resize_panes(){self.status=Some(error);}}
                self.terminal_dirty=true;self.persist_dirty=true;self.frame_clock.invalidate();
            },
            window_events::WindowNotice::Unavailable(error)=>{
                self.fallback_poll=true;
                self.status=Some(format!("Update/restart server for immediate output notifications: {error}"));
                self.frame_clock.invalidate();
            },
        }
    }
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| {
            let base = clanky::calm::font_metrics(&self.zoom,1.0,None,[self.config.cell_width_scale,self.config.cell_height_scale])?;
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("Clanky")
                            .with_transparent(true)
                            .with_inner_size(LogicalSize::new(
                                (self.config.columns as f64 * base.cell_width as f64 + 592.0).max(1240.0),
                                (self.config.rows as f64 * base.cell_height as f64 + 228.0).max(760.0),
                            )),
                    )
                    .map_err(|e| e.to_string())?,
            );
            self.scale = window.scale_factor();
            self.studio.theme = self.config.resolved_theme(clanky::platform::system_light(&window));
            window.set_ime_allowed(true);
            let mut renderer = block_on(Renderer::new(window.clone()))?;
            if self.frame_trace.enabled() { eprintln!("clanky: frame trace on {} ({})", renderer.adapter_summary, renderer.surface_summary); }
            renderer.set_font_families(&self.config.font_family, &self.config.ui_font_family)
                .map_err(|error| format!("Configuration: {error}"))?;
            self.terminal_face = renderer.terminal_face();
            if let Some(panel) = &mut self.settings { panel.mono_fonts = renderer.monospace_families(); }
            if !renderer.supports_transparency() {
                self.status = Some("Compositor transparency unavailable; using opaque background".into());
            }
            self.renderer = Some(renderer);
            self.window = Some(window);
            self.metrics_changed()?;
            if self.groups.is_empty() {
                self.new_tab()?;
                self.split(Axis::Rows)?;
            }
            self.start_robot();
            Ok(())
        })();
        if let Err(e) = result {
            self.fail(event_loop, e);
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let visual_event=match &event {
            WindowEvent::RedrawRequested=>false,
            WindowEvent::KeyboardInput{event,..} if event.state==ElementState::Released=>false,
            _=>true,
        };
        if matches!(event,WindowEvent::Resized(_)|WindowEvent::ScaleFactorChanged{..}){self.terminal_dirty=true;}
        self.sent_input=false;
        if matches!(&event, WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed)
            || matches!(&event, WindowEvent::MouseInput { state: ElementState::Pressed, .. } | WindowEvent::Ime(Ime::Commit(_))) {
            self.skip_robot(); // Do not return: the same event follows the normal input path below.
        }
        let result = match event {
            WindowEvent::ThemeChanged(theme) => {
                self.studio.theme = self.config.resolved_theme(clanky::platform::preference_is_light(Some(theme)));
                self.frame_clock.invalidate();
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
            WindowEvent::CursorLeft { .. } => { self.pointer_pressed=false; self.mouse=(-1.0,-1.0); if let Some(panel)=&mut self.settings{panel.release();} Ok(()) }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse = (position.x, position.y);
                if self.palette.open { self.palette.pointer(position.x, position.y); }
                if self.agent_picker.open { let context=self.agent_context(); self.agent_picker.pointer(position.x, position.y, &context); }
                if self.context_menu.open { self.context_menu.pointer(position.x, position.y); self.frame_clock.invalidate(); }
                if let Err(e) = self.drag() { self.status = Some(e); }
                if let Err(e) = self.terminal_motion() { self.status = Some(e); }
                Ok(())
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.pointer_pressed=true;
                if let Err(e) = self.click() {
                    self.status = Some(e);
                }
                Ok(())
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                self.pointer_pressed=false;
                if let Some(panel)=&mut self.settings {panel.release();}
                self.files.dragging = None;
                self.compose_dragging = false;
                self.studio.drag = None;
                if let Err(e) = self.terminal_release(1) { self.status = Some(e); }
                Ok(())
            }
            WindowEvent::MouseInput { state, button: button @ (MouseButton::Middle | MouseButton::Right), .. } => {
                let button = if button == MouseButton::Middle { 2 } else { 3 };
                let result = if state == ElementState::Pressed && self.context_menu.open {
                    // Another right-click moves the menu; a middle click only closes it.
                    self.context_menu.close();
                    self.frame_clock.invalidate();
                    if button == 3 { self.secondary_press() } else { Ok(()) }
                } else if state == ElementState::Pressed && button == 3 { self.secondary_press() }
                else if state == ElementState::Pressed { self.terminal_press(button) } else { self.terminal_release(button) };
                if let Err(e) = result { self.status = Some(e); }
                Ok(())
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // winit 0.30 on X11 reports a wheel button's release as a second notch when the
                // button is not emulated (XTEST, mice without scroll valuators); its raw release
                // arrived as a device event, so drop one wheel event for it.
                if self.wheel_releases > 0 && matches!(delta, MouseScrollDelta::LineDelta(..)) {
                    self.wheel_releases -= 1;
                } else if self.context_menu.open {
                    self.context_menu.close();
                    self.frame_clock.invalidate();
                } else if let Err(e) = self.scroll(delta) { self.status = Some(e); }
                Ok(())
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Released && event.logical_key == Key::Named(NamedKey::Enter) => {
                self.compose_enter_held = false;
                Ok(())
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.frame_trace.key_event();
                self.blink_start = Instant::now();
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
                if self.palette.open {
                    match &event {
                        Ime::Commit(text)=>self.palette.insert(text),
                        Ime::Preedit(text,_)=>self.palette.preedit(text),
                        Ime::Disabled=>self.palette.preedit(""),
                        _=>(),
                    }
                    let _=self.input.on_ime(&event);
                    self.frame_clock.invalidate();
                    return;
                }
                if self.settings_open() {
                    match &event {
                        Ime::Commit(text)=>self.settings.as_mut().unwrap().insert(text),
                        Ime::Preedit(text,_)=>self.settings.as_mut().unwrap().preedit(text),
                        _=>(),
                    }
                    let _=self.input.on_ime(&event);
                    self.frame_clock.invalidate();
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
                if !focused{self.pointer_pressed=false;}
                if focused != self.window_focused { if let Err(e) = self.terminal_focus(focused) { self.status = Some(e); } }
                self.window_focused = focused;
                self.blink_start = Instant::now();
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
            WindowEvent::RedrawRequested => {self.frame_clock.begin_frame();self.draw()},
            _ => Ok(()),
        };
        // Settings controls may have applied config during this input event.
        self.sync_post_ai();
        if visual_event && (!self.sent_input || self.fallback_poll){
            if self.sent_input{self.terminal_dirty=true;}
            self.frame_clock.invalidate();
        }
        if let Err(e) = result {
            self.frame_clock.invalidate();
            self.fail(event_loop, e);
        } else if let Err(e) = self.publish_focus() {
            self.status = Some(e);
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(clock) = &mut self.gate_clock {
            if let Err(error) = clock.poll() { self.fail(event_loop, error); return; }
        }
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
            self.sync_post_ai();
            let system_light = self.window.as_ref().is_some_and(|w| clanky::platform::system_light(w));
            if let Some(runtime) = &mut self.theme_runtime {
                if runtime.poll(&self.config, system_light) {
                    if let Some(error) = runtime.error() { self.status = Some(error.to_owned()); }
                    self.frame_clock.invalidate();
                }
            }
            self.next_settings=Instant::now()+Duration::from_millis(500);
        }
        if Instant::now() >= self.next_polish {
            let old_status=self.status.clone();
            if let Err(e) = self.poll_session() { self.status = Some(e); }
            if self.refresh_studio(){self.frame_clock.invalidate();}
            let stamps=self.files.panes.values().filter_map(|pane|std::fs::metadata(pane.path()).ok()
                .map(|metadata|(pane.path().to_owned(),(metadata.modified().ok(),metadata.len())))).collect::<BTreeMap<_,_>>();
            if stamps!=self.file_stamps{self.file_stamps=stamps;self.frame_clock.invalidate();}
            if self.fallback_poll {
                let ids=self.groups.get(self.active).map(|g|g.layout.leaves()).unwrap_or_default();
                for id in ids {
                    let old=self.engine.cached_generation(&id);
                    if self.engine.snapshot_with_blocks(&id).is_ok() && old!=self.engine.cached_generation(&id){self.frame_clock.invalidate();}
                }
            }
            if self.url_opener.poll().is_err(){self.status=Some("URL opener failed".into());}
            while let Some(error)=self.clipboard_bridge.take_error(){self.status=Some(error);}
            if old_status!=self.status{self.frame_clock.invalidate();}
            self.next_polish = Instant::now() + Duration::from_secs(1);
        }
        let now=self.clock_now();
        let mut wake=self.next_settings.min(self.next_polish);
        if !self.is_suspended(){
            // A blink phase change is a frame; once blinking rests there are none.
            if self.cursor_visible() != self.blink_shown { self.frame_clock.invalidate(); }
            if self.window_focused {
                if let Some(at) = self.blink.next_change(self.blink_start, Instant::now()) { wake = wake.min(at); }
            }
            if self.frame_clock.request_due(now){if let Some(window)=&self.window{window.request_redraw();}}
            if self.gate_clock.is_none() {
                if let Some(deadline)=self.frame_clock.deadline(now){wake=wake.min(deadline);}
            }
        }
        if self.gate_clock.is_some() { wake=wake.min(clanky::robot_gate_clock::RobotGateClock::poll_deadline()); }
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
    }
}
