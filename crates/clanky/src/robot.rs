//! Clock-driven robot6 greeting. Input is never consumed by this controller.
//! Geometry and easing are shared by startup, +hello and the offscreen gate.
#[path = "robot_art.rs"]
mod art;
use crate::themes::Theme;
use clanky_render::Layer;
use std::path::Path;

pub const FIRST_MEETING_MS: u64 = 7000;
pub const GREETING_MS: u64 = 1200;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    FirstMeeting,
    Greeting,
}
impl Kind {
    pub fn duration_ms(self) -> u64 {
        match self {
            Self::FirstMeeting => FIRST_MEETING_MS,
            Self::Greeting => GREETING_MS,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Animation {
    pub kind: Kind,
    skipped: bool,
    finished: bool,
}
impl Animation {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            skipped: false,
            finished: false,
        }
    }
    /// Call before the normal key/click handler, then keep routing that event.
    pub fn skip(&mut self) {
        self.skipped = true;
    }
    pub fn running(&self, elapsed_ms: u64) -> bool {
        !self.skipped && !self.finished && elapsed_ms < self.kind.duration_ms()
    }
    /// Emit the final sample once even if a compositor wakes just after the
    /// deadline. Its logical clock is clamped; no subsequent frame is requested.
    pub fn sample(&mut self, elapsed_ms: u64) -> Option<u64> {
        if self.skipped || self.finished {
            return None;
        }
        let time = elapsed_ms.min(self.kind.duration_ms());
        if time == self.kind.duration_ms() {
            self.finished = true;
        }
        Some(time)
    }
}
/// Atomic create-new means two simultaneous windows cannot both claim first
/// meeting. Call only once a real window can show the animation.
pub fn claim_kind(marker: &Path) -> Result<Kind, String> {
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    crate::platform::private_file(&mut options);
    match options.open(marker) {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(b"Clanky first meeting 1\n")
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
            Ok(Kind::FirstMeeting)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(Kind::Greeting),
        Err(e) => Err(e.to_string()),
    }
}

pub struct Frame {
    pub layers: Vec<Layer>,
    pub antenna: [f32; 2],
}
/// Pixel rect is the first pane (or +hello's canvas). This function does no I/O.
pub fn frame(theme: &Theme, rect: [f32; 4], kind: Kind, elapsed_ms: u64) -> Frame {
    frame_with_measure(
        theme,
        rect,
        kind,
        elapsed_ms,
        &mut clanky_render::measure_text,
    )
}
/// Window callers supply Renderer::measure_text so configured fonts centre correctly.
pub fn frame_with_measure(
    theme: &Theme,
    rect: [f32; 4],
    kind: Kind,
    elapsed_ms: u64,
    measure: &mut dyn FnMut(&clanky_render::TextRun) -> clanky_render::TextMetrics,
) -> Frame {
    art::draw(
        theme,
        rect,
        &Pose::at(kind, elapsed_ms.min(kind.duration_ms()) as f32),
        measure,
    )
}

const LINEAR: [f32; 4] = [0., 0., 1., 1.];
const OUT: [f32; 4] = [0., 0., 0.58, 1.];
const IN_OUT: [f32; 4] = [0.42, 0., 0.58, 1.];
/// Solve CSS cubic-bezier's x, then evaluate its y; y may overshoot 1.
fn ease(t: f32, c: [f32; 4]) -> f32 {
    if t <= 0. {
        return 0.;
    }
    if t >= 1. {
        return 1.;
    }
    let b = |u: f32, a: f32, z: f32| {
        3. * (1. - u).powi(2) * u * a + 3. * (1. - u) * u * u * z + u * u * u
    };
    let (mut lo, mut hi) = (0., 1.);
    for _ in 0..24 {
        let u = (lo + hi) * 0.5;
        if b(u, c[0], c[2]) < t {
            lo = u;
        } else {
            hi = u;
        }
    }
    b((lo + hi) * 0.5, c[1], c[3])
}
fn progress(t: f32, start: f32, duration: f32, c: [f32; 4]) -> f32 {
    ease((t - start) / duration, c)
}
fn keys(t: f32, start: f32, duration: f32, c: [f32; 4], points: &[(f32, f32)]) -> f32 {
    let p = ((t - start) / duration).clamp(0., 1.);
    for pair in points.windows(2) {
        if p <= pair[1].0 {
            return pair[0].1
                + (pair[1].1 - pair[0].1) * ease((p - pair[0].0) / (pair[1].0 - pair[0].0), c);
        }
    }
    points.last().unwrap().1
}
#[derive(Clone, Debug)]
struct Pose {
    time: f32,
    roll: f32,
    camera: f32,
    bob: f32,
    brake: f32,
    base_yaw: f32,
    base_z: f32,
    tilt: f32,
    torso_yaw: f32,
    torso_z: f32,
    head_yaw: f32,
    head_z: f32,
    notice_y: f32,
    notice_angle: f32,
    head_tilt: f32,
    lids: f32,
    frown: f32,
    light: f32,
    flash: f32,
    iris: f32,
    ping: f32,
    ping_size: f32,
    bars: [f32; 5],
    wave: f32,
    elbow: f32,
    claw: f32,
    word: f32,
    word_y: f32,
    wheel: f32,
    bloom: f32,
}
impl Pose {
    fn at(kind: Kind, time: f32) -> Self {
        let greeting = kind == Kind::Greeting;
        let t = if greeting { 4900. + time } else { time };
        let roll = progress(t, 0., 1600., [0.2, 0.75, 0.25, 1.]);
        let base = progress(t, 5450., 800., [0.55, 0., 0.25, 1.]);
        let torso = progress(t, 2450., 1200., [0.5, 0., 0.2, 1.12]);
        let head = progress(t, 2650., 1300., [0.45, 0., 0.2, 1.3]);
        let lit = progress(t, 4900., 114., OUT);
        let open = progress(t, 4900., 220., [0.3, 1.5, 0.5, 1.]);
        let frown = progress(t, 3950., 500., OUT);
        let blink_t = if greeting { 6150. + time - 350. } else { t };
        let wave_t = if greeting { 6300. + time - 400. } else { t };
        let word_t = if greeting { 6500. + time - 750. } else { t };
        let blink = if (6150. ..6320.).contains(&blink_t) {
            keys(
                blink_t,
                6150.,
                170.,
                IN_OUT,
                &[(0., 0.), (0.5, 1.), (1., 0.)],
            )
        } else {
            0.
        };
        let lids = if t < 4900. {
            0.3 + 0.42 * frown
        } else {
            0.72 + (0.16 - 0.72) * open
        };
        let mut out = Self {
            time: t,
            roll: -620. * (1. - roll),
            camera: 1. + 0.07 * progress(t, 2400., 3600., [0.45, 0., 0.25, 1.]),
            bob: if t < 1600. {
                keys(
                    t % 320.,
                    0.,
                    320.,
                    LINEAR,
                    &[(0., 0.), (0.5, -2.), (1., 0.)],
                )
            } else {
                0.
            },
            brake: keys(
                t,
                1550.,
                300.,
                OUT,
                &[(0., 0.), (0.35, 5.), (0.7, -1.5), (1., 0.)],
            ),
            base_yaw: -90. * base,
            base_z: -60. - 36. * base,
            tilt: keys(t, 5450., 800., IN_OUT, &[(0., 0.), (0.5, -20.), (1., 0.)]),
            torso_yaw: -90. * torso,
            torso_z: -52. + 20. * torso,
            head_yaw: -90. * head,
            head_z: -54. + 29. * head,
            notice_y: keys(t, 2150., 260., IN_OUT, &[(0., 0.), (0.5, -4.), (1., 0.)]),
            notice_angle: keys(t, 2150., 260., IN_OUT, &[(0., 0.), (0.5, -4.), (1., 0.)]),
            head_tilt: if t < 4900. {
                5. * progress(t, 3950., 500., IN_OUT)
            } else {
                5. - 12. * progress(t, 4900., 300., [0.3, 1.6, 0.5, 1.])
            },
            lids: lids + (1. - lids) * blink,
            frown: 19. * frown * (1. - open),
            light: lit,
            flash: keys(t, 4900., 380., OUT, &[(0., 1.), (0.3, 1.6), (1., 1.)]),
            iris: 1.6 - 0.6 * progress(t, 4900., 300., OUT),
            ping: if t < 4920. {
                0.
            } else {
                0.9 * (1. - progress(t, 4920., 700., OUT))
            },
            ping_size: 0.5 + 2.7 * progress(t, 4920., 700., OUT),
            bars: std::array::from_fn(|i| progress(t, 4950. + i as f32 * 70., 90., OUT)),
            wave: keys(
                wave_t,
                6300.,
                560.,
                IN_OUT,
                &[
                    (0., 0.),
                    (0.3, -112.),
                    (0.55, -96.),
                    (0.8, -112.),
                    (1., -104.),
                ],
            ),
            elbow: keys(
                wave_t,
                6300.,
                560.,
                IN_OUT,
                &[(0., 0.), (0.3, -30.), (0.55, -8.), (0.8, -30.), (1., -30.)],
            ),
            claw: if (6550. ..6830.).contains(&wave_t) {
                keys(
                    (wave_t - 6550.) % 140.,
                    0.,
                    140.,
                    IN_OUT,
                    &[(0., 0.), (0.5, 26.), (1., 0.)],
                )
            } else {
                0.
            },
            word: progress(word_t, 6500., 450., OUT),
            word_y: 8. * (1. - progress(word_t, 6500., 450., OUT)),
            wheel: -1100. * (1. - roll),
            bloom: keys(t, 4850., 900., OUT, &[(0., 0.), (0.35, 1.), (1., 0.75)]),
        };
        if greeting {
            out.roll = 0.;
            out.camera = 1.07;
            out.bob = 0.;
            out.brake = 0.;
            out.base_yaw = -90.;
            out.base_z = -96.;
            out.tilt = 0.;
            out.torso_yaw = -90.;
            out.torso_z = -32.;
            out.head_yaw = -90.;
            out.head_z = -25.;
            out.notice_y = 0.;
            out.notice_angle = 0.;
            out.wheel = 0.;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_marker() {
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target")
            });
        let directory = target.join(format!("robot-marker-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let marker = directory.join("state/first-meeting");
        assert_eq!(claim_kind(&marker).unwrap(), Kind::FirstMeeting);
        assert_eq!(std::fs::read(&marker).unwrap(), b"Clanky first meeting 1\n");
        assert_eq!(claim_kind(&marker).unwrap(), Kind::Greeting);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn robot_frames() {
        let theme = crate::themes::builtin("stone-dark").unwrap();
        for kind in [Kind::FirstMeeting, Kind::Greeting] {
            let mut animation = Animation::new(kind);
            assert_eq!(animation.sample(0), Some(0));
            assert_eq!(
                animation.sample(kind.duration_ms()),
                Some(kind.duration_ms())
            );
            if std::env::var_os("CLANKY_ROBOT_FAULT").is_some() {
                animation.finished = false;
            }
            assert_eq!(
                animation.sample(kind.duration_ms() + 1),
                None,
                "robot must stop"
            );
            assert!(!animation.running(kind.duration_ms() + 2000));
        }
        let mut skipped = Animation::new(Kind::FirstMeeting);
        skipped.skip();
        assert_eq!(skipped.sample(300), None);
        let draw = |ms| {
            let frame = frame(&theme, [0., 0., 400., 400.], Kind::FirstMeeting, ms);
            let scene = clanky_render::Scene {
                clear: theme.colors.sidebar,
                layers: frame.layers,
            };
            let pixels = clanky_render::offscreen_scene(&scene, 400, 400).unwrap();
            (frame.antenna, pixels)
        };
        let (antenna, final_pixels) = draw(7000);
        let pixel = |pixels: &[u8], x: usize, y: usize| -> [u8; 4] {
            pixels[(y * 400 + x) * 4..(y * 400 + x + 1) * 4]
                .try_into()
                .unwrap()
        };
        let light = pixel(
            &final_pixels,
            antenna[0].round() as usize,
            antenna[1].round() as usize,
        );
        assert!(
            light
                .iter()
                .zip([210u8, 164, 118, 255])
                .all(|(a, b)| a.abs_diff(b) <= 3),
            "antenna: {light:?}"
        );
        let pre = draw(5400).1;
        let mid = draw(5850).1;
        let post = draw(6300).1;
        assert_ne!(
            pixel(&mid, 170, 300),
            pixel(&pre, 170, 300),
            "base pre-turn"
        );
        assert_ne!(
            pixel(&mid, 170, 300),
            pixel(&post, 170, 300),
            "base post-turn"
        );
        let pre = draw(2400).1;
        let mid = draw(3200).1;
        let post = draw(4000).1;
        assert_ne!(
            pixel(&mid, 178, 205),
            pixel(&pre, 178, 205),
            "torso pre-turn"
        );
        assert_ne!(
            pixel(&mid, 178, 205),
            pixel(&post, 178, 205),
            "torso post-turn"
        );
        if let Some(path) = std::env::var_os("CLANKY_ROBOT_SAMPLE") {
            std::fs::write(path, &final_pixels).unwrap();
        }
        println!("PASS: 7000/1200 ms stop, skip, opaque antenna, base/torso mid-turn pixels");
    }
}
