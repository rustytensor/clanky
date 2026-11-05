//! Standalone +hello window. No server, PTY or first-meeting marker is touched.
use clanky::{
    config::{Config, ThemePreference},
    robot::{self, Animation, Kind},
    themes::{self, Appearance, Theme},
};
use clanky_render::{Renderer, Scene, block_on};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

pub fn run(config: Config) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let frozen = !config.animations;
    let mut app = Hello {
        config,
        window: None,
        renderer: None,
        theme: themes::builtin("stone-dark")?,
        animation: Animation::new(Kind::FirstMeeting),
        started: Instant::now(),
        next: Instant::now(),
        last_time: if frozen {
            Kind::FirstMeeting.duration_ms()
        } else {
            0
        },
        error: None,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.error.map_or(Ok(()), Err)
}
struct Hello {
    config: Config,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    theme: Theme,
    animation: Animation,
    started: Instant,
    next: Instant,
    last_time: u64,
    error: Option<String>,
}
impl Hello {
    fn draw(&mut self) -> Result<(), String> {
        let window = self.window.as_ref().ok_or("hello window unavailable")?;
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        if self.config.animations {
            self.last_time = self
                .animation
                .sample(self.started.elapsed().as_millis() as u64)
                .unwrap_or(self.last_time);
        }
        let renderer = self.renderer.as_mut().ok_or("hello renderer unavailable")?;
        let frame = robot::frame_with_measure(
            &self.theme,
            [
                24.,
                24.,
                (size.width as f32 - 48.).max(1.),
                (size.height as f32 - 48.).max(1.),
            ],
            Kind::FirstMeeting,
            self.last_time,
            &mut |run| renderer.measure_text(run),
        );
        renderer.draw_scene(&Scene {
            clear: self.theme.colors.sidebar,
            layers: frame.layers,
        })
    }
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: String) {
        self.error = Some(error);
        event_loop.exit();
    }
}
impl ApplicationHandler for Hello {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| {
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("Clanky +hello")
                            .with_inner_size(LogicalSize::new(720., 720.)),
                    )
                    .map_err(|e| e.to_string())?,
            );
            let light = match self.config.theme {
                ThemePreference::Light => true,
                ThemePreference::Dark => false,
                ThemePreference::System => clanky::platform::system_light(&window),
            };
            self.theme = themes::builtin(themes::default_id(if light {
                Appearance::Light
            } else {
                Appearance::Dark
            }))?;
            let mut renderer = block_on(Renderer::new(window.clone()))?;
            renderer.set_font_families(&self.config.font_family, &self.config.ui_font_family)?;
            self.renderer = Some(renderer);
            self.started = Instant::now();
            self.next = self.started;
            window.request_redraw();
            self.window = Some(window);
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                event_loop.exit()
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            } => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.draw() {
                    self.fail(event_loop, error);
                }
            }
            _ => (),
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.last_time >= Kind::FirstMeeting.duration_ms() {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        let now = Instant::now();
        if now >= self.next {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            self.next = (now + Duration::from_millis(16))
                .min(self.started + Duration::from_millis(Kind::FirstMeeting.duration_ms()));
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next));
    }
}
