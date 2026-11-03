use clanky_core::{Cell, Engine, TerminalSnapshot, platform};
use clanky_render::{Renderer, block_on};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

fn main() {
    if let Err(error) = run() {
        eprintln!("clanky: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut headless = false;
    let mut socket = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--headless" => headless = true,
            "--socket" => socket = Some(PathBuf::from(args.next().ok_or("--socket needs a path")?)),
            "--help" | "-h" => {
                println!(
                    "clanky [--headless] [--socket PATH]\nCtrl+Shift+T: new tab; Ctrl+Shift+W: close tab; Ctrl+Tab: next tab\nCtrl+Shift+A: toggle agent permission for the selected tab"
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
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
    let server_engine = engine.clone();
    let server_stop = stop.clone();
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
        run_window(engine.clone(), stop.clone())
    };
    stop.store(true, Ordering::Release);
    engine.shutdown();
    let server_result = server
        .join()
        .map_err(|_| "socket thread panicked".to_string())?
        .map_err(|e| e.to_string());
    result.and(server_result)
}

fn run_window(engine: Engine, stop: Arc<AtomicBool>) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let mut app = App {
        engine,
        stop,
        window: None,
        renderer: None,
        active: None,
        modifiers: ModifiersState::empty(),
        mouse: (0.0, 0.0),
        error: None,
        next_frame: Instant::now(),
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.error.map_or(Ok(()), Err)
}

struct App {
    engine: Engine,
    stop: Arc<AtomicBool>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    active: Option<String>,
    modifiers: ModifiersState,
    mouse: (f64, f64),
    error: Option<String>,
    next_frame: Instant,
}

impl App {
    fn call(&self, op: &str, args: Value) -> Result<Value, String> {
        self.engine.request(op, &args, true).map_err(|e| e.message)
    }

    fn dimensions(&self) -> (usize, usize) {
        let size = self.window.as_ref().unwrap().inner_size();
        (
            ((size.height / 32).saturating_sub(1) as usize).clamp(1, 999),
            ((size.width / 16) as usize).clamp(1, 250),
        )
    }

    fn new_tab(&mut self) -> Result<(), String> {
        let (rows, cols) = self.dimensions();
        let tab = self.call(
            "open_tab",
            json!({"rows":rows,"cols":cols,"allow_agent":false}),
        )?;
        let id = tab["tab_id"].as_str().ok_or("missing tab ID")?.to_string();
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        if let Err(error) = self.call("run", json!({"tab_id":id,"argv":[shell],"cwd":cwd})) {
            let _ = self.call("close_tab", json!({"tab_id":id}));
            return Err(error);
        }
        self.active = Some(id);
        Ok(())
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: String) {
        self.error = Some(error);
        event_loop.exit();
    }

    fn select_next(&mut self) {
        let ids = self.engine.tab_ids();
        if let Some(index) = ids.iter().position(|id| Some(id) == self.active.as_ref()) {
            self.active = Some(ids[(index + 1) % ids.len()].clone());
        } else {
            self.active = ids.first().cloned();
        }
    }

    fn draw(&mut self) -> Result<(), String> {
        let ids = self.engine.tab_ids();
        if !ids.iter().any(|id| Some(id) == self.active.as_ref()) {
            self.active = ids.first().cloned();
        }
        let Some(id) = self.active.as_ref() else {
            return Ok(());
        };
        let mut snapshot = self.engine.snapshot(id).map_err(|e| e.message)?;
        let allowed = self.engine.agent_permission(id).map_err(|e| e.message)?;
        if let Some(window) = self.window.as_ref() {
            window.set_title(&format!(
                "Clanky — {id} — agent {} (Ctrl+Shift+A toggles)",
                if allowed { "enabled" } else { "disabled" }
            ));
        }
        let mut bar = vec![
            Cell {
                ch: ' ',
                fg: [220, 220, 220],
                bg: [32, 32, 32]
            };
            snapshot.cols
        ];
        for (index, tab) in ids.iter().enumerate() {
            let label = format!(" {} {} ", index + 1, tab);
            let start = index * 16;
            for (offset, ch) in label.chars().take(16).enumerate() {
                if let Some(cell) = bar.get_mut(start + offset) {
                    cell.ch = ch;
                    if tab == id {
                        cell.bg = [50, 80, 120];
                        cell.fg = [255, 255, 255];
                    }
                }
            }
        }
        bar.append(&mut snapshot.cells);
        let frame = TerminalSnapshot {
            rows: snapshot.rows + 1,
            cols: snapshot.cols,
            cells: bar,
            cursor: snapshot.cursor.map(|(row, col)| (row + 1, col)),
        };
        self.renderer.as_mut().unwrap().draw(&frame)
    }

    fn key(&mut self, key: &Key, text: Option<&str>) -> Result<(), String> {
        let ctrl = self.modifiers.control_key();
        let shift = self.modifiers.shift_key();
        if ctrl && shift {
            if let Key::Character(ch) = key {
                match ch.to_lowercase().as_str() {
                    "t" => return self.new_tab(),
                    "a" => {
                        if let Some(id) = self.active.as_ref() {
                            let allowed =
                                self.engine.agent_permission(id).map_err(|e| e.message)?;
                            self.engine
                                .set_agent_permission(id, !allowed)
                                .map_err(|e| e.message)?;
                        }
                        return Ok(());
                    }
                    "w" => {
                        if let Some(id) = self.active.clone() {
                            self.call("close_tab", json!({"tab_id":id}))?;
                            self.active = self.engine.tab_ids().first().cloned();
                        }
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
        if ctrl && *key == Key::Named(NamedKey::Tab) {
            self.select_next();
            return Ok(());
        }
        let Some(id) = self.active.as_ref() else {
            return Ok(());
        };
        let mut bytes = match key {
            Key::Named(NamedKey::Enter) => vec![b'\r'],
            Key::Named(NamedKey::Backspace) => vec![127],
            Key::Named(NamedKey::Tab) => vec![b'\t'],
            Key::Named(NamedKey::Escape) => vec![27],
            Key::Named(NamedKey::ArrowUp) => b"\x1b[A".to_vec(),
            Key::Named(NamedKey::ArrowDown) => b"\x1b[B".to_vec(),
            Key::Named(NamedKey::ArrowRight) => b"\x1b[C".to_vec(),
            Key::Named(NamedKey::ArrowLeft) => b"\x1b[D".to_vec(),
            Key::Named(NamedKey::Home) => b"\x1b[H".to_vec(),
            Key::Named(NamedKey::End) => b"\x1b[F".to_vec(),
            Key::Named(NamedKey::Delete) => b"\x1b[3~".to_vec(),
            Key::Named(NamedKey::PageUp) => b"\x1b[5~".to_vec(),
            Key::Named(NamedKey::PageDown) => b"\x1b[6~".to_vec(),
            Key::Character(ch) if ctrl && ch.len() == 1 && ch.as_bytes()[0].is_ascii() => {
                vec![ch.as_bytes()[0] & 0x1f]
            }
            _ => text.unwrap_or("").as_bytes().to_vec(),
        };
        if self.modifiers.alt_key() && !bytes.is_empty() {
            bytes.insert(0, 27);
        }
        if !bytes.is_empty() {
            self.engine.input(id, &bytes).map_err(|e| e.message)?;
        }
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| {
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("Clanky — Ctrl+Shift+T new tab, Ctrl+Tab switch")
                            .with_inner_size(LogicalSize::new(1280.0, 800.0)),
                    )
                    .map_err(|e| e.to_string())?,
            );
            self.renderer = Some(block_on(Renderer::new(window.clone()))?);
            self.window = Some(window);
            self.new_tab()
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let result = match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                Ok(())
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
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
            } => {
                if self.mouse.1 < 32.0 {
                    if let Some(id) = self.engine.tab_ids().get((self.mouse.0 / 256.0) as usize) {
                        self.active = Some(id.clone());
                    }
                }
                Ok(())
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.key(&event.logical_key, event.text.as_deref())
            }
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                if let Some(id) = self.active.as_ref() {
                    self.engine
                        .input(id, text.as_bytes())
                        .map_err(|e| e.message)
                } else {
                    Ok(())
                }
            }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.resize(size.width, size.height);
                    }
                    let (rows, cols) = self.dimensions();
                    for id in self.engine.tab_ids() {
                        if let Err(error) = self.engine.resize(&id, rows, cols) {
                            self.fail(event_loop, error.message);
                            break;
                        }
                    }
                }
                Ok(())
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if platform::shutdown_requested() || self.stop.load(Ordering::Acquire) {
            event_loop.exit();
            return;
        }
        let now = Instant::now();
        if now >= self.next_frame {
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
            self.next_frame = now + Duration::from_millis(16);
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}
