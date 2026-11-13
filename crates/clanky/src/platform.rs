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

/// Open an untrusted theme path without following a final symlink or blocking
/// on a FIFO. The caller checks the opened descriptor is a regular file.
pub fn open_theme_document(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    #[cfg(target_os = "linux")] {
        use std::os::unix::fs::OpenOptionsExt;
        // O_NOFOLLOW | O_NONBLOCK from libc (the raw values differ on ARM64),
        // kept behind the platform boundary.
        std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path)
    }
    #[cfg(not(target_os = "linux"))] {
        let _ = path;
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "safe theme reads are not implemented on this platform"))
    }
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

/// Whether a path is a regular file someone may execute (agent tools on PATH).
pub fn is_executable(path: &std::path::Path) -> bool {
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        path.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))] { path.is_file() }
}

/// Start a detached copy of this executable (a window for `clanky agent`), with its
/// own process group so the caller's terminal signals do not reach it.
pub fn spawn_detached(args: &[std::ffi::OsString]) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let mut command = std::process::Command::new(exe);
    command.args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(unix)] { use std::os::unix::process::CommandExt; command.process_group(0); }
    let mut child = command.spawn()?;
    std::thread::spawn(move || { let _ = child.wait(); });
    Ok(())
}

/// GNOME's cursor blink setting (org.gnome.desktop.interface cursor-blink,
/// cursor-blink-time in ms, cursor-blink-timeout in s), when gsettings can read it.
pub fn gnome_cursor_blink() -> Option<(bool, u32, u32)> {
    #[cfg(target_os = "linux")] {
        let get = |key: &str| -> Option<String> {
            let out = std::process::Command::new("gsettings").args(["get", "org.gnome.desktop.interface", key])
                .stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()?;
            out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        };
        let number = |text: String| text.rsplit(' ').next()?.parse::<u32>().ok();
        let blink = match get("cursor-blink")?.as_str() { "true" => true, "false" => false, _ => return None };
        Some((blink, get("cursor-blink-time").and_then(number)?, get("cursor-blink-timeout").and_then(number)?))
    }
    #[cfg(not(target_os = "linux"))] { None }
}
