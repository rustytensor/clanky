//! Startup hooks kept separate from the terminal's normal event routing.
use super::App;
use clanky::{
    robot::{self, Animation},
    themes,
};
use clanky_render::Scene;
use std::time::{Duration, Instant};

impl App {
    pub(super) fn start_robot(&mut self) {
        if !self.config.startup_animation || !self.config.animations {
            return;
        }
        let kind = clanky::platform::robot_marker_path()
            .ok_or("No robot state directory".to_owned())
            .and_then(|path| robot::claim_kind(&path));
        match kind {
            Ok(kind) => self.startup = Some((Instant::now(), Animation::new(kind))),
            Err(error) => self.status = Some(format!("Startup animation unavailable: {error}")),
        }
    }
    pub(super) fn skip_robot(&mut self) {
        if let Some((_, mut animation)) = self.startup.take() {
            animation.skip();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
    /// The idle scheduler can use this deadline without polling once startup ends.
    pub(super) fn robot_deadline(&self) -> Option<Instant> {
        self.startup.as_ref().map(|(start, a)| {
            (*start + Duration::from_millis(a.kind.duration_ms()))
                .min(Instant::now() + Duration::from_millis(16))
        })
    }
    pub(super) fn draw_robot(&mut self, scene: &mut Scene, rect: [f32; 4]) -> Result<(), String> {
        if !self.config.startup_animation || !self.config.animations {
            self.startup = None;
            return Ok(());
        }
        let Some((start, animation)) = &mut self.startup else {
            return Ok(());
        };
        let elapsed = start.elapsed().as_millis() as u64;
        if elapsed >= animation.kind.duration_ms() {
            // This deadline frame exposes the already-running shell. Never keep
            // a finished robot overlay or schedule a cursor blink afterwards.
            self.startup = None;
            return Ok(());
        }
        let kind = animation.kind;
        let Some(time) = animation.sample(elapsed) else {
            self.startup = None;
            return Ok(());
        };
        // M12 scene builders can supply their selected Theme directly to frame.
        // On this integration base only Stone's dark/light choice is available.
        let theme = themes::builtin(if self.studio.theme.is_light {
            "stone-light"
        } else {
            "stone-dark"
        })?;
        let renderer = self.renderer.as_mut().ok_or("renderer not ready")?;
        scene.layers.extend(
            robot::frame_with_measure(&theme, rect, kind, time, &mut |run| {
                renderer.measure_text(run)
            })
            .layers,
        );
        Ok(())
    }
}
