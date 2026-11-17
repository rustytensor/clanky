//! Desktop lookups: the toolkit-free ones live in `clanky-state`; the winit window's own are here.
pub use clanky_state::platform::*;
use winit::window::{Theme, Window};

pub fn system_light(window: &Window) -> bool {
    preference_is_light(window.theme())
}

pub fn preference_is_light(theme: Option<Theme>) -> bool {
    matches!(theme, Some(Theme::Light))
}
