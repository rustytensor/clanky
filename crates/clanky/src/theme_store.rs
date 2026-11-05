//! Settings-facing theme data. Call reload from the app's existing settings
//! timer; this module owns no watcher thread, draw loop or window state.
use crate::themes::{Appearance, BUILTINS, Color, MAX_THEME_BYTES, Theme};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const MAX_USER_THEMES: usize = 128;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewSwatches {
    pub background: Color,
    pub foreground: Color,
    pub muted: Color,
    pub agent: Color,
    pub ansi: [Color; 16],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerEntry {
    /// Built-in ID or user:<filename stem>. Names never identify files.
    pub id: String,
    pub name: String,
    pub appearance: Appearance,
    pub builtin: bool,
    pub description: Option<String>,
    pub swatches: PreviewSwatches,
    pub window_gradient: Option<Vec<Color>>,
    pub glow: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeIssue {
    pub path: PathBuf,
    pub message: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct UserTheme {
    theme: Theme,
    bytes: Vec<u8>,
}

type Observed = BTreeMap<PathBuf, Result<Vec<u8>, String>>;
pub struct ThemeStore {
    directory: PathBuf,
    users: BTreeMap<String, UserTheme>,
    observed: Observed,
    issues: Vec<ThemeIssue>,
}

/// The standard config directory, independent of an explicit config-file override.
pub fn default_directory() -> Option<PathBuf> {
    crate::platform::settings_path()?
        .parent()
        .map(|p| p.join("themes"))
}
fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 64
        && slug.as_bytes()[0].is_ascii_alphanumeric()
        && slug
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
/// Validate a saved picker ID without requiring a user file to exist yet.
pub fn validate_id(id: &str) -> Result<(), String> {
    if id.strip_prefix("user:").is_some_and(valid_slug) || BUILTINS.iter().any(|b| b.id == id) {
        Ok(())
    } else {
        Err("theme ID must name a built-in or user:<safe filename stem>".into())
    }
}

impl ThemeStore {
    /// Load a directory without creating it. Invalid documents appear in issues;
    /// valid neighbours and built-ins remain usable.
    pub fn open(directory: impl Into<PathBuf>) -> Result<Self, String> {
        let mut store = Self {
            directory: directory.into(),
            users: BTreeMap::new(),
            observed: BTreeMap::new(),
            issues: Vec::new(),
        };
        store.reload()?;
        Ok(store)
    }
    pub fn open_default() -> Result<Self, String> {
        Self::open(default_directory().ok_or("no theme directory is available")?)
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn issues(&self) -> &[ThemeIssue] {
        &self.issues
    }

    /// Content-based hot reload, including edits of identical size/timestamp.
    /// Invalid files keep their last-good theme; deleted files leave the picker.
    /// Returns true on new file content or issues, false for unchanged input.
    /// A directory-level failure leaves the entire previous snapshot intact.
    pub fn reload(&mut self) -> Result<bool, String> {
        let observed = scan(&self.directory)?;
        if observed == self.observed {
            return Ok(false);
        }
        let mut users = BTreeMap::new();
        let mut issues = Vec::new();
        for (path, source) in &observed {
            let slug = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let id = format!("user:{slug}");
            let result = if !valid_slug(slug) {
                Err("theme filename must be 1 to 64 letters, digits, - or _, starting with a letter or digit".into())
            } else {
                source.clone().and_then(|bytes| {
                    let theme = parse_bytes(&bytes)?;
                    Ok(UserTheme { theme, bytes })
                })
            };
            match result {
                Ok(theme) => {
                    users.insert(id, theme);
                }
                Err(message) => {
                    if let Some(previous) = self.users.get(&id) {
                        users.insert(id, previous.clone());
                    }
                    issues.push(ThemeIssue {
                        path: path.clone(),
                        message,
                    });
                }
            }
        }
        self.users = users;
        self.issues = issues;
        self.observed = observed;
        Ok(true)
    }
    /// Return an independent editable copy; never mutate embedded themes.
    pub fn get(&self, id: &str) -> Result<Theme, String> {
        validate_id(id)?;
        if id.starts_with("user:") {
            self.users
                .get(id)
                .map(|u| u.theme.clone())
                .ok_or_else(|| format!("user theme not loaded: {id}"))
        } else {
            crate::themes::builtin(id)
        }
    }
    /// Built-ins first in their declared order, then user themes sorted by name/id.
    pub fn picker(&self, appearance: Option<Appearance>) -> Result<Vec<PickerEntry>, String> {
        let entry = |id: String, builtin: bool, theme: Theme| PickerEntry {
            id,
            builtin,
            name: theme.name,
            appearance: theme.appearance,
            description: theme.description,
            swatches: PreviewSwatches {
                background: theme.colors.background,
                foreground: theme.colors.foreground,
                muted: theme.colors.muted,
                agent: theme.colors.agent,
                ansi: theme.ansi,
            },
            window_gradient: theme.window_gradient,
            glow: theme.glow,
        };
        let mut entries = BUILTINS
            .iter()
            .map(|b| b.load().map(|t| entry(b.id.into(), true, t)))
            .collect::<Result<Vec<_>, _>>()?;
        let mut users = self
            .users
            .iter()
            .map(|(id, u)| entry(id.clone(), false, u.theme.clone()))
            .collect::<Vec<_>>();
        users.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        entries.extend(users);
        entries.retain(|entry| appearance.is_none_or(|a| entry.appearance == a));
        Ok(entries)
    }
    /// Save as a new user theme; caller edits name/colours on an independent Theme.
    /// Refuses an existing file, including a built-in-named user file or symlink.
    pub fn save_new(&mut self, slug: &str, theme: &Theme) -> Result<String, String> {
        self.install(slug, theme.to_json()?.into_bytes())
    }
    /// Import a validated file byte-for-byte under a new safe filename stem.
    pub fn import(&mut self, source: &Path, slug: &str) -> Result<String, String> {
        self.install(slug, read_document(source)?)
    }
    /// Export last-good user bytes (or canonical built-in JSON) to a NEW path.
    /// Existing files are never replaced; the UI must choose another filename.
    pub fn export(&self, id: &str, destination: &Path) -> Result<(), String> {
        let theme = self.get(id)?;
        let bytes = self
            .users
            .get(id)
            .map(|u| u.bytes.clone())
            .unwrap_or(theme.to_json()?.into_bytes());
        create_document(destination, &bytes)
    }
    fn install(&mut self, slug: &str, bytes: Vec<u8>) -> Result<String, String> {
        if !valid_slug(slug) {
            return Err("invalid user theme filename stem".into());
        }
        let theme = parse_bytes(&bytes)?;
        // Check disk limits too: an external writer may have added files since reload.
        if scan(&self.directory)?.len() >= MAX_USER_THEMES {
            return Err("user theme directory exceeds 128 documents".into());
        }
        let path = self.directory.join(format!("{slug}.json"));
        create_document(&path, &bytes)?;
        let id = format!("user:{slug}");
        self.users.insert(
            id.clone(),
            UserTheme {
                theme,
                bytes: bytes.clone(),
            },
        );
        // A previously invalid file may have been removed before Save as new.
        // Its error must not survive our successful replacement-by-creation.
        self.issues.retain(|issue| issue.path != path);
        self.observed.insert(path, Ok(bytes));
        Ok(id)
    }
}
fn parse_bytes(bytes: &[u8]) -> Result<Theme, String> {
    Theme::from_json(std::str::from_utf8(bytes).map_err(|_| "theme must be UTF-8")?)
}
fn directory_exists(directory: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(directory) {
        Ok(meta) if meta.is_dir() => Ok(true),
        Ok(_) => Err("theme directory must be a directory, not a symlink".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("theme directory {}: {e}", directory.display())),
    }
}
fn scan(directory: &Path) -> Result<Observed, String> {
    let mut observed = BTreeMap::new();
    if !directory_exists(directory)? {
        return Ok(observed);
    }
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        if observed.len() >= MAX_USER_THEMES {
            return Err("user theme directory exceeds 128 documents".into());
        }
        observed.insert(path.clone(), read_document(&path));
    }
    Ok(observed)
}
fn read_document(path: &Path) -> Result<Vec<u8>, String> {
    let file = crate::platform::open_theme_document(path).map_err(|e| e.to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("theme must be a regular file".into());
    }
    if meta.len() > MAX_THEME_BYTES as u64 {
        return Err("theme exceeds 64 KiB".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_THEME_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_THEME_BYTES {
        return Err("theme exceeds 64 KiB".into());
    }
    Ok(bytes)
}
/// Atomic, no-clobber publication: hard-link a completed private sibling.
/// The destination appears complete or not at all, and never replaces another file.
fn create_document(path: &Path, bytes: &[u8]) -> Result<(), String> {
    parse_bytes(bytes)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    directory_exists(parent)?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temp = parent.join(format!(
        ".clanky-theme-{}-{}.tmp",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    crate::platform::private_file(&mut options);
    let mut file = options.open(&temp).map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::hard_link(&temp, path).map_err(|e| {
            format!(
                "cannot create theme {} (existing files are not replaced): {e}",
                path.display()
            )
        })
    })();
    drop(file);
    let cleanup =
        fs::remove_file(&temp).map_err(|e| format!("theme temporary-file cleanup failed: {e}"));
    result.and(cleanup)
}

#[cfg(test)]
#[path = "theme_store_tests.rs"]
mod tests;
