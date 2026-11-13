//    ██
// Fixture for the Q3 syntax gate: lines 12-26 are the approved mockup's editor section.
//                        ██
use std::time::Duration;

const LIMIT: u64 = 0x1F;
static NAME: &str = "blink\n";
// l l l l l l

fn tilt(l: u8) -> u8 { l + l * l }

/// Half cycle and timeout, as VTE: 1200 ms cycle, stop after 10 s idle.
pub struct Blink {
    pub half: Duration,
    pub timeout: Duration,
}

impl Blink {
    pub fn visible(&self, since_key: Duration) -> bool {
        if since_key >= self.timeout { return true; }
        (since_key.as_millis() / self.half.as_millis()) % 2 == 0
    }
}

#[test]
fn stops_after_timeout() { assert!(Blink::vte().visible(Duration::from_secs(11))); }
