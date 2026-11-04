//! M11 look-gate entry point. The app's layout and scene API is not merged yet.
//!
//! Keep this fail-closed until the real app builds the fixture; an empty scene
//! cannot stand in for the fixed Studio state or establish any gate result.
use clanky_render::{Scene, offscreen_scene};

fn main() {
    // Compile against the renderer's public scene contract without treating its
    // intentionally unfinished renderer or a synthetic scene as a passing gate.
    let _render: fn(&Scene, u32, u32) -> Result<Vec<u8>, String> = offscreen_scene;
    let _scene = Scene::default();
    eprintln!("FAIL: M11 app layout and scene APIs are not available yet");
    std::process::exit(1);
}
