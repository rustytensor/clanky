use clanky::{
    config::{Config, Metrics, Zoom},
    input::{InputState, key_bytes},
};
use clanky_core::{
    Cell, Engine, TerminalSnapshot,
    layout::{Axis, Layout, Rect},
    platform,
};
use clanky_render::{Renderer, block_on};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
    event::{ElementState, MouseButton, WindowEvent},
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
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--headless" => headless = true,
            "--socket" => socket = Some(PathBuf::from(args.next().ok_or("--socket needs a path")?)),
            "--config" => {
                config_path = Some(PathBuf::from(args.next().ok_or("--config needs a path")?))
            }
            "--help" | "-h" => {
                println!(
                    "clanky [--headless] [--socket PATH] [--config PATH]\nCtrl+Shift+T: new tab; Ctrl+Shift+D/E: split right/below\nCtrl+Shift+W: close pane; Ctrl+Tab: next tab; click: focus pane\nCtrl+=/-/0: zoom/reset; Ctrl+Shift+A: toggle focused pane agent access"
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    // Socket-only operation keeps v1 defaults and does not depend on GUI config.
    let config = if headless && config_path.is_none() {
        Config::default()
    } else {
        Config::load(config_path.as_deref())?
    };
    let socket = socket
        .or_else(|| {
            std::env::var_os("XDG_RUNTIME_DIR").map(|dir| {
                PathBuf::from(dir)
                    .join("clanky")
                    .join(format!("{}.sock", std::process::id()))
            })
        })
        .ok_or("set XDG_RUNTIME_DIR or pass --socket in a private directory")?;
    platform::install_shutdown_signals().map_err(|e| e.to_string())?;
    let engine = Engine::new();
    let stop = Arc::new(AtomicBool::new(false));
    let (server_engine, server_stop) = (engine.clone(), stop.clone());
    let server = std::thread::spawn(move || {
        let result = clanky_api::serve(server_engine, &socket, server_stop.clone());
        server_stop.store(true, Ordering::Release);
        result
    });
    let result = if headless {
        while !platform::shutdown_requested() && !server.is_finished() {
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    } else {
        run_window(engine.clone(), stop.clone(), config)
    };
    stop.store(true, Ordering::Release);
    engine.shutdown();
    let server_result = server
        .join()
        .map_err(|_| "socket thread panicked".to_string())?
        .map_err(|e| e.to_string());
    result.and(server_result)
}

struct Group {
    layout: Layout,
    focus: String,
}
struct App {
    engine: Engine,
    stop: Arc<AtomicBool>,
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
}

fn run_window(engine: Engine, stop: Arc<AtomicBool>, config: Config) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let mut app = App {
        engine,
        stop,
        zoom: Zoom::new(config.font_size_pt),
        config,
        scale: 1.0,
        window: None,
        renderer: None,
        groups: Vec::new(),
        active: 0,
        sizes: BTreeMap::new(),
        modifiers: ModifiersState::empty(),
        mouse: (0.0, 0.0),
        input: InputState::default(),
        occluded: false,
        error: None,
        status: None,
        next_frame: Instant::now(),
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
    fn rect(&self) -> Result<Rect, String> {
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
    fn focused(&self) -> Option<&str> {
        self.groups.get(self.active).map(|g| g.focus.as_str())
    }
    fn reset_ime(&mut self) {
        self.input.focus_lost();
        if let Some(w) = &self.window {
            w.set_ime_allowed(false);
            w.set_ime_allowed(true);
        }
    }
    fn sync_groups(&mut self) {
        let previous = self.focused().map(str::to_owned);
        let ids: BTreeSet<_> = self.engine.tab_ids().into_iter().collect();
        for group in &mut self.groups {
            for id in group.layout.leaves() {
                if !ids.contains(&id) {
                    group.layout.remove(&id);
                }
            }
            if !group.layout.contains(&group.focus) {
                group.focus = group.layout.leaves().first().cloned().unwrap_or_default();
            }
        }
        self.groups.retain(|g| !g.layout.leaves().is_empty());
        let known: BTreeSet<_> = self.groups.iter().flat_map(|g| g.layout.leaves()).collect();
        for id in ids.difference(&known) {
            self.groups.push(Group {
                layout: Layout::leaf(id.clone()),
                focus: id.clone(),
            });
        }
        self.active = previous
            .as_ref()
            .and_then(|id| self.groups.iter().position(|g| g.layout.contains(id)))
            .unwrap_or(self.active.min(self.groups.len().saturating_sub(1)));
        self.sizes.retain(|id, _| ids.contains(id));
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
        if self.is_suspended() {
            return Ok(());
        }
        self.sync_groups();
        let root = self.rect()?;
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
    fn spawn_terminal(&self, rows: usize, cols: usize) -> Result<String, String> {
        let tab = self.call(
            "open_tab",
            json!({"rows":rows,"cols":cols,"allow_agent":false}),
        )?;
        let id = tab["tab_id"].as_str().ok_or("missing tab ID")?.to_owned();
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        if let Err(e) = self.call("run", json!({"tab_id":id,"argv":[shell],"cwd":cwd})) {
            let _ = self.call("close_tab", json!({"tab_id":id}));
            return Err(e);
        }
        Ok(id)
    }
    fn new_tab(&mut self) -> Result<(), String> {
        let r = self.rect()?;
        let id = self.spawn_terminal(r.rows, r.cols)?;
        self.groups.push(Group {
            layout: Layout::leaf(id.clone()),
            focus: id,
        });
        self.active = self.groups.len() - 1;
        self.reset_ime();
        self.resize_panes()
    }
    fn split(&mut self, axis: Axis) -> Result<(), String> {
        self.sync_groups();
        let Some(group) = self.groups.get(self.active) else {
            return self.new_tab();
        };
        let root = self.rect()?;
        let target = group.focus.clone();
        let mut candidate = group.layout.clone();
        candidate.split(&target, "__new__".into(), axis.clone(), root.clone())?;
        let r = candidate
            .rectangles(root.clone())
            .into_iter()
            .find(|(id, _)| id == "__new__")
            .ok_or("new pane is not visible")?
            .1;
        let id = self.spawn_terminal(r.rows, r.cols)?;
        if let Err(e) = self.groups[self.active]
            .layout
            .split(&target, id.clone(), axis, root)
        {
            let _ = self.call("close_tab", json!({"tab_id":id}));
            return Err(e);
        }
        self.groups[self.active].focus = id;
        self.reset_ime();
        self.resize_panes()
    }
    fn close_pane(&mut self) -> Result<(), String> {
        if let Some(id) = self.focused().map(str::to_owned) {
            self.call("close_tab", json!({"tab_id":id}))?;
        }
        self.sync_groups();
        self.reset_ime();
        self.resize_panes()
    }
    fn send(&self, bytes: &[u8]) -> Result<(), String> {
        if !bytes.is_empty() {
            if let Some(id) = self.focused() {
                self.engine.input(id, bytes).map_err(|e| e.message)?;
            }
        }
        Ok(())
    }
    fn key(&mut self, key: &Key, text: Option<&str>) -> Result<(), String> {
        let ctrl = self.modifiers.control_key();
        if ctrl {
            if let Key::Character(ch) = key {
                let lower = ch.to_lowercase();
                if self.modifiers.shift_key() {
                    match lower.as_str() {
                        "t" => return self.new_tab(),
                        "d" => return self.split(Axis::Columns),
                        "e" => return self.split(Axis::Rows),
                        "w" => return self.close_pane(),
                        "a" => {
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
                    self.reset_ime();
                }
                return Ok(());
            }
        }
        self.send(&key_bytes(
            key,
            text,
            self.modifiers,
            self.input.composing(),
        ))
    }
    fn click(&mut self) -> Result<(), String> {
        let m = self.metrics()?;
        let col = (self.mouse.0 / m.cell_width as f64) as usize;
        let row = (self.mouse.1 / m.cell_height as f64) as usize;
        if row == 0 {
            let index = col / 16;
            if index < self.groups.len() {
                self.active = index;
                self.reset_ime();
            }
        } else if let Some(group) = self.groups.get(self.active) {
            if let Some(id) = group.layout.hit_test(self.rect()?, col, row) {
                self.groups[self.active].focus = id;
                self.reset_ime();
            }
        }
        Ok(())
    }
    fn draw(&mut self) -> Result<(), String> {
        if self.is_suspended() {
            return Ok(());
        }
        self.resize_panes()?;
        let r = self.rect()?;
        let mut frame = TerminalSnapshot {
            rows: r.rows + 1,
            cols: r.cols,
            cells: vec![
                Cell {
                    ch: ' ',
                    fg: [220, 220, 220],
                    bg: [32, 32, 32]
                };
                (r.rows + 1) * r.cols
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
        if let Some(group) = self.groups.get(self.active) {
            for (id, pane) in group.layout.rectangles(r.clone()) {
                let snapshot = match self.engine.snapshot(&id) {
                    Ok(s) => s,
                    Err(e) if e.code == "not_found" => continue,
                    Err(e) => return Err(e.message),
                };
                for row in 0..pane.rows.min(snapshot.rows) {
                    for col in 0..pane.cols.min(snapshot.cols) {
                        frame.cells[(pane.row + row) * frame.cols + pane.col + col] =
                            snapshot.cells[row * snapshot.cols + col].clone();
                    }
                }
                if id == group.focus {
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
        if let Some(window) = &self.window {
            let id = self.focused().unwrap_or("no pane");
            let allowed = self.engine.agent_permission(id).unwrap_or(false);
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
        self.renderer.as_mut().unwrap().draw(&frame)
    }
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: String) {
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
            self.new_tab()
        })();
        if let Err(e) = result {
            self.fail(event_loop, e);
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let result = match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                Ok(())
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
                Ok(())
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse = (position.x, position.y);
                Ok(())
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.click(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.status = None;
                if let Err(e) = self.key(&event.logical_key, event.text.as_deref()) {
                    self.status = Some(e);
                }
                Ok(())
            }
            WindowEvent::Ime(event) => {
                if let Some(bytes) = self.input.on_ime(&event) {
                    self.send(&bytes)
                } else {
                    Ok(())
                }
            }
            WindowEvent::Focused(false) => {
                self.input.focus_lost();
                self.modifiers = ModifiersState::empty();
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
        if platform::shutdown_requested() || self.stop.load(Ordering::Acquire) {
            event_loop.exit();
            return;
        }
        if Instant::now() >= self.next_frame {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            self.next_frame = Instant::now() + Duration::from_millis(16);
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}
