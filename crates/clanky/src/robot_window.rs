//! Startup hooks kept separate from the terminal's normal event routing.
use super::App;
use clanky::{
    robot::{self, Startup},
    themes::Theme,
};
use clanky_render::Scene;
use std::time::Instant;

impl App {
    pub(super) fn start_robot(&mut self) {
        if !self.config.startup_animation || !self.config.animations {
            return;
        }
        let kind = clanky::platform::robot_marker_path()
            .ok_or("No robot state directory".to_owned())
            .and_then(|path| robot::claim_kind(&path));
        match kind {
            Ok(kind) => {
                self.startup = Some(Startup::new(kind, Instant::now()));
                self.frame_trace.robot_started(kind.duration_ms());
                self.frame_clock.invalidate();
            }
            Err(error) => self.status = Some(format!("Startup animation unavailable: {error}")),
        }
    }
    pub(super) fn skip_robot(&mut self) {
        if let Some(mut startup) = self.startup.take() {
            startup.skip();
            self.frame_trace.robot_stopped(true);
            self.frame_clock.invalidate();
        }
    }
    /// The idle scheduler can use this deadline without polling once startup ends.
    pub(super) fn robot_deadline(&self) -> Option<Instant> {
        if !self.config.startup_animation || !self.config.animations {
            return None;
        }
        self.startup.as_ref().and_then(Startup::deadline)
    }
    pub(super) fn draw_robot(
        &mut self,
        scene: &mut Scene,
        rect: [f32; 4],
        theme: &Theme,
    ) -> Result<(), String> {
        if !self.config.startup_animation || !self.config.animations {
            if self.startup.take().is_some() {
                self.frame_trace.robot_stopped(true);
            }
            return Ok(());
        }
        let Some(startup) = &mut self.startup else {
            return Ok(());
        };
        let kind = startup.kind();
        let Some(time) = startup.sample(Instant::now()) else {
            // The final deadline frame clears the overlay without postponing
            // the stop to a later tick or retaining a blinking cursor.
            self.startup = None;
            self.frame_trace.robot_stopped(false);
            return Ok(());
        };
        self.frame_trace.robot_sample(time);
        let renderer = self.renderer.as_mut().ok_or("renderer not ready")?;
        scene.layers.extend(
            robot::frame_with_measure(theme, rect, kind, time, &mut |run| {
                renderer.measure_text(run)
            })
            .layers,
        );
        Ok(())
    }
}
