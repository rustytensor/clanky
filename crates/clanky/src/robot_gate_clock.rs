//! Explicitly opt-in clock for the real-window robot gate. Normal windows never
//! read a clock file or poll for it. The gate still uses FrameClock and real input.
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub struct RobotGateClock {
    path: PathBuf,
    origin: Instant,
    elapsed_ms: u64,
    force_redraw: bool,
}
impl RobotGateClock {
    pub fn from_env() -> Result<Option<Self>, String> {
        if std::env::var_os("CLANKY_ROBOT_GATE").as_deref() != Some(std::ffi::OsStr::new("1")) {
            return Ok(None);
        }
        let path = std::env::var_os("CLANKY_ROBOT_CLOCK")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or("robot gate needs an absolute clock path")?;
        let mut clock = Self {
            path,
            origin: Instant::now(),
            elapsed_ms: 0,
            force_redraw: std::env::var_os("CLANKY_GATE_FAULT").as_deref()
                == Some(std::ffi::OsStr::new("robot")),
        };
        clock.poll()?;
        if clock.elapsed_ms != 0 {
            return Err("robot gate clock must start at zero".into());
        }
        Ok(Some(clock))
    }
    pub fn poll(&mut self) -> Result<bool, String> {
        use std::io::Read;
        let file = std::fs::File::open(&self.path).map_err(|_| "cannot read robot gate clock")?;
        let mut bytes = Vec::new();
        file.take(33)
            .read_to_end(&mut bytes)
            .map_err(|_| "cannot read robot gate clock")?;
        if bytes.len() > 32 {
            return Err("invalid robot gate clock".into());
        }
        let time = std::str::from_utf8(&bytes)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .filter(|ms| *ms >= self.elapsed_ms && *ms <= 60000)
            .ok_or("invalid robot gate clock")?;
        let changed = time != self.elapsed_ms;
        self.elapsed_ms = time;
        Ok(changed)
    }
    pub fn now(&self) -> Instant {
        self.origin + Duration::from_millis(self.elapsed_ms)
    }
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }
    pub fn force_redraw(&self) -> bool {
        self.force_redraw
    }
    pub fn poll_deadline() -> Instant {
        Instant::now() + Duration::from_millis(5)
    }
}
