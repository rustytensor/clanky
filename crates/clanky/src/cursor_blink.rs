//! Cursor blinking as VTE does it: on for half the blink time, off for half, while the
//! window has focus; a key press restarts it, and it stops, with the cursor shown, once
//! the blink timeout has passed since the last key.
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blink {
    pub enabled: bool,
    /// One on or off phase: half the configured blink time.
    pub half: Duration,
    pub timeout: Duration,
}

impl Blink {
    /// From the config, with "system" following GNOME's setting when it is readable.
    pub fn from_config(config: &crate::config::Config, gnome: Option<(bool, u32, u32)>) -> Self {
        let (enabled, time_ms, timeout_s) = match config.cursor_blink {
            crate::config::CursorBlink::Off => (false, config.cursor_blink_time_ms, config.cursor_blink_timeout_s),
            crate::config::CursorBlink::On => (true, config.cursor_blink_time_ms, config.cursor_blink_timeout_s),
            crate::config::CursorBlink::System => gnome.unwrap_or((true, config.cursor_blink_time_ms, config.cursor_blink_timeout_s)),
        };
        Self { enabled: enabled && time_ms >= 100, half: Duration::from_millis(u64::from(time_ms.max(100)) / 2),
            timeout: Duration::from_secs(u64::from(timeout_s.max(1))) }
    }
    /// Whether the cursor shows `elapsed` after the last restart.
    pub fn visible(&self, elapsed: Duration) -> bool {
        !self.enabled || elapsed >= self.timeout || (elapsed.as_nanos() / self.half.as_nanos()) % 2 == 0
    }
    /// When, after `start`, the cursor next appears or disappears; None once it rests.
    pub fn next_change(&self, start: Instant, now: Instant) -> Option<Instant> {
        if !self.enabled { return None; }
        let elapsed = now.saturating_duration_since(start);
        if elapsed >= self.timeout { return None; }
        let next = self.half * (elapsed.as_nanos() / self.half.as_nanos() + 1) as u32;
        if next < self.timeout { return Some(start + next); }
        // The last phase ends at the timeout only if the cursor is hidden then.
        (!self.visible(self.timeout - Duration::from_nanos(1))).then_some(start + self.timeout)
    }
}

#[cfg(test)]
#[test]
fn blinks_like_vte() {
    // GNOME defaults (VTE's): blink time 1200 ms -> 600 ms phases; timeout 10 s.
    let blink = Blink { enabled: true, half: Duration::from_millis(600), timeout: Duration::from_secs(10) };
    let start = Instant::now();
    assert!(blink.visible(Duration::from_millis(599)) && !blink.visible(Duration::from_millis(600)));
    // 10 s / 600 ms: changes at 0.6, 1.2 ... 9.6 s (16); phase 16 (9.6-10 s) is an on phase, so it rests shown.
    let (mut at, mut changes) = (start, 0);
    while let Some(next) = blink.next_change(start, at) { at = next; changes += 1; }
    assert_eq!((changes, at - start), (16, Duration::from_millis(9600)));
    assert!(blink.visible(Duration::from_secs(11)));
}
