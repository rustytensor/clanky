//! The shell integration Clanky ships for bash and zsh: OSC 133 prompt marks (A, B, C, D) and
//! OSC 7 folder reports, so blocks and the window's command input work without user setup.
//! The user's own rc file is read first. In the command input's editor mode the shell's own
//! prompt is not printed (the block meta line replaces it); in classic mode it is, marks only.
//! The mode is a file per pane the shell tests at every prompt, so a change applies at the next one.
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

const BASH: &str = include_str!("shell/clanky.bash");
const ZSH: &str = include_str!("shell/clanky.zsh");
const ZSHENV: &str = include_str!("shell/zshenv");
const ZSHRC: &str = include_str!("shell/zshrc");

static EDITOR: AtomicBool = AtomicBool::new(true);
static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
static PANES: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// Where the scripts live: `$XDG_RUNTIME_DIR/clanky/shell-<version>`, else the temporary folder.
fn dir() -> Option<&'static Path> {
    DIR.get_or_init(|| {
        let base = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|p| p.is_absolute())
            .unwrap_or_else(std::env::temp_dir);
        let dir = base.join("clanky").join(format!("shell-{}-{}", env!("CARGO_PKG_VERSION"), uid()));
        install(&dir).ok().map(|_| dir)
    }).as_deref()
}
fn uid() -> u32 {
    // SAFETY: getuid has no preconditions and cannot fail.
    unsafe { libc::getuid() }
}
fn install(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir.join("zsh"))?;
    for path in [dir.to_owned(), dir.join("zsh")] {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
    }
    for (name, text) in [("clanky.bash", BASH), ("clanky.zsh", ZSH), ("zsh/.zshenv", ZSHENV), ("zsh/.zshrc", ZSHRC)] {
        let path = dir.join(name);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(text) {
            let tmp = dir.join(format!(".{}.{}", name.replace('/', "_"), std::process::id()));
            std::fs::write(&tmp, text)?;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
            std::fs::rename(&tmp, &path)?;
        }
    }
    Ok(())
}
/// The pane's name in the mode file: the server's process ID and the tab, so two servers never share one.
fn pane_key(tab_id: &str) -> String { format!("{}-{tab_id}", std::process::id()) }
fn write_mode(dir: &Path, key: &str, editor: bool) {
    let path = dir.join(format!("mode-{key}"));
    if editor { let _ = std::fs::write(path, b"editor\n"); } else { let _ = std::fs::remove_file(path); }
}

/// The argv and extra environment that start `argv` (a lone bash or zsh) with the integration,
/// or None to start it as given.
pub(crate) fn integrate(argv: &[String], tab_id: &str) -> Option<(Vec<String>, Vec<(String, String)>)> {
    let [shell] = argv else { return None; };
    let name = Path::new(shell).file_name()?.to_str()?;
    let dir = dir()?;
    let dir_text = dir.to_str()?.to_owned();
    let key = pane_key(tab_id);
    let mut env = vec![("CLANKY_SHELL_DIR".to_owned(), dir_text.clone()), ("CLANKY_TAB".to_owned(), key.clone())];
    let argv = match name {
        "bash" => vec![shell.clone(), "--init-file".into(), format!("{dir_text}/clanky.bash"), "-i".into()],
        "zsh" => {
            if let Some(user) = std::env::var_os("ZDOTDIR").and_then(|v| v.into_string().ok()) {
                env.push(("CLANKY_USER_ZDOTDIR".into(), user));
            }
            env.push(("ZDOTDIR".into(), format!("{dir_text}/zsh")));
            vec![shell.clone(), "-i".into()]
        }
        _ => return None,
    };
    write_mode(dir, &key, EDITOR.load(Ordering::Acquire));
    PANES.lock().ok()?.insert(key);
    Some((argv, env))
}
/// Forget a closed pane's mode file.
pub(crate) fn forget(tab_id: &str) {
    let key = pane_key(tab_id);
    if PANES.lock().is_ok_and(|mut panes| panes.remove(&key)) {
        if let Some(dir) = DIR.get().and_then(|d| d.as_deref()) { write_mode(dir, &key, false); }
    }
}
/// Editor mode (no shell prompt) or classic mode (the shell's own prompt) for every pane, now and later.
pub fn set_editor(editor: bool) {
    if EDITOR.swap(editor, Ordering::AcqRel) == editor { return; }
    let Some(dir) = DIR.get().and_then(|d| d.as_deref()) else { return; };
    if let Ok(panes) = PANES.lock() {
        for key in panes.iter() { write_mode(dir, key, editor); }
    }
}
