//! Desktop lookups stay behind this boundary; scene/config code is platform-neutral.
use std::path::PathBuf;
use winit::window::{Theme, Window};

pub fn system_light(window: &Window) -> bool {
    preference_is_light(window.theme())
}

pub fn preference_is_light(theme: Option<Theme>) -> bool {
    matches!(theme, Some(Theme::Light))
}

#[cfg(target_os = "linux")]
pub fn settings_path() -> Option<PathBuf> {
    linux::settings_path()
}
#[cfg(target_os = "linux")]
mod linux {
    use super::PathBuf;
    pub fn settings_path() -> Option<PathBuf> {
        std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").filter(|v| !v.is_empty())
                .map(|home| PathBuf::from(home).join(".config")))
            .map(|base| base.join("clanky/config.json"))
    }
}

// Other desktop ports must supply their usual settings location here.
#[cfg(not(target_os = "linux"))]
pub fn settings_path() -> Option<PathBuf> { None }
