//! Desktop lookups stay behind this boundary; scene/config code is platform-neutral.
use std::path::PathBuf;
use winit::window::{Theme, Window};

/// Ask the desktop's file association to open a settings file without a shell.
pub fn open_settings_file(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "linux")] {
        let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        let mut child = std::process::Command::new("xdg-open").arg(path)
            .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null()).spawn().map_err(|e| e.to_string())?;
        std::thread::spawn(move || { let _ = child.wait(); });
        Ok(())
    }
    #[cfg(not(target_os = "linux"))] { let _ = path; Err("Opening settings is not implemented on this platform".into()) }
}

pub fn private_file(options: &mut std::fs::OpenOptions) {
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(not(unix))] let _ = options;
}

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
/// Per-user state shared by windows on this machine, never an SSH host path.
#[cfg(target_os = "linux")]
pub fn robot_marker_path() -> Option<PathBuf> { linux::robot_marker_path() }
#[cfg(not(target_os = "linux"))]
pub fn robot_marker_path() -> Option<PathBuf> { None }

#[cfg(target_os = "linux")]
mod linux {
    use super::PathBuf;
    pub fn robot_marker_path() -> Option<PathBuf> {
        std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()).map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").filter(|v| !v.is_empty())
                .map(|home| PathBuf::from(home).join(".local/state")))
            .map(|base| base.join("clanky/first-meeting"))
    }
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
