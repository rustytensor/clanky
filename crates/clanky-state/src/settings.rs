//! The settings panel edits the same validated, ordered settings file as Config: config.xml, or
//! the old JSON format for an explicit path ending in .json.
use crate::config::Config;
use serde_json::Value;
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_BYTES: u64 = 1024 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub struct Settings {
    path: PathBuf,
    config: Config,
    /// The last good file's bytes and its settings document (what a change starts from while the
    /// file on disk is broken).
    bytes: Vec<u8>,
    document: Value,
    observed: Option<Vec<u8>>,
    last_error: Option<String>,
}
impl Settings {
    /// Missing files represent defaults and are created only on the first edit. A file that does
    /// not parse or holds a bad value opens with the defaults and its error (see `error`); it is
    /// not rewritten until a change is made.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = path.into();
        let (observed, read_error) = match read_optional(&path) {
            Ok(bytes) => (bytes, None),
            Err(error) => (None, Some(error)),
        };
        let mut settings = Self {
            config: defaults(&path),
            bytes: empty(&path),
            document: Value::Object(Default::default()),
            observed: observed.clone(),
            last_error: read_error,
            path,
        };
        if let Some(bytes) = observed {
            match validate_soft(&settings.path, &bytes) {
                Ok((config, document, soft)) => {
                    settings.config = config;
                    settings.document = document;
                    settings.bytes = bytes;
                    settings.last_error = soft;
                }
                Err(error) => settings.last_error = Some(error),
            }
        }
        Ok(settings)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn config(&self) -> &Config {
        &self.config
    }
    /// The file's current problem (with its line), while the last good settings stay in use.
    pub fn error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
    /// Returns true only for a new valid file. Invalid edits keep the last good
    /// config and report once per distinct content/error; no file is rewritten.
    pub fn reload(&mut self) -> Result<bool, String> {
        let observed = match read_optional(&self.path) {
            Ok(bytes) => bytes,
            Err(error) => {
                if self.last_error.as_ref() == Some(&error) {
                    return Ok(false);
                }
                self.last_error = Some(error.clone());
                return Err(error);
            }
        };
        if observed == self.observed {
            return Ok(false);
        }
        self.observed = observed.clone();
        let Some(bytes) = observed else {
            let error = "settings file was removed; keeping last good settings".to_owned();
            self.last_error = Some(error.clone());
            return Err(error);
        };
        match validate_soft(&self.path, &bytes) {
            Ok((config, document, soft)) => {
                self.config = config;
                self.document = document;
                self.bytes = bytes;
                self.last_error = soft;
                Ok(true)
            }
            Err(error) => {
                self.last_error = Some(error.clone());
                Err(error)
            }
        }
    }
    pub fn set(&mut self, key: &str, value: Value) -> Result<&Config, String> {
        self.change(|document| {
            document[key] = value;
            Ok(())
        })
    }
    /// Several keys in one atomic write: Some sets a key, None removes it (other keys keep their order).
    pub fn set_many(&mut self, changes: &[(&str, Option<Value>)]) -> Result<&Config, String> {
        self.change(|document| {
            for (key, value) in changes {
                match value {
                    Some(value) => document[*key] = value.clone(),
                    None => {
                        document.as_object_mut().unwrap().shift_remove(*key);
                    }
                }
            }
            Ok(())
        })
    }
    pub fn reset(&mut self, key: &str) -> Result<&Config, String> {
        self.change(|document| {
            // shift_remove preserves every other key's original order.
            document.as_object_mut().unwrap().shift_remove(key);
            Ok(())
        })
    }
    /// None resets just this colour to the selected palette's default.
    pub fn set_color(&mut self, token: &str, color: Option<&str>) -> Result<&Config, String> {
        if !crate::look::COLOR_TOKENS.contains(&token)
            && !crate::themes::COLOR_TOKENS.contains(&token)
        {
            return Err(format!("unknown color token: {token}"));
        }
        self.change(|document| {
            if document.get("colors").is_none() {
                document["colors"] = serde_json::json!({});
            }
            let colors = document["colors"]
                .as_object_mut()
                .ok_or("colors must be an object")?;
            match color {
                Some(color) => {
                    colors.insert(token.into(), Value::String(color.into()));
                }
                None => {
                    colors.shift_remove(token);
                }
            }
            if colors.is_empty() {
                document.as_object_mut().unwrap().shift_remove("colors");
            }
            Ok(())
        })
    }
    fn change(
        &mut self,
        change: impl FnOnce(&mut Value) -> Result<(), String>,
    ) -> Result<&Config, String> {
        // Merge against the latest valid disk content, never a stale UI copy; while the file on
        // disk is broken, against the last good settings (a change replaces the broken file).
        let expected = read_optional(&self.path)?;
        let mut document = match expected
            .as_deref()
            .map(|bytes| validate_soft(&self.path, bytes))
        {
            Some(Ok((_, document, _))) => document,
            _ => self.document.clone(),
        };
        if !document.is_object() {
            document = Value::Object(Default::default());
        }
        change(&mut document)?;
        Config::window_from_json(&document.to_string())?;
        let bytes = if crate::config::is_json_path(&self.path) {
            let mut bytes = serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?;
            bytes.push(b'\n');
            bytes
        } else {
            crate::config_xml::write(&document).into_bytes()
        };
        self.apply(bytes, expected)
    }
    pub fn import(&mut self, source: &Path) -> Result<&Config, String> {
        let bytes = read_optional(source)?.ok_or("settings import file does not exist")?;
        let expected = read_optional(&self.path)?;
        self.apply(bytes, expected)
    }
    /// Export the last-good file byte-for-byte. The UI must confirm replacement.
    pub fn export(&self, destination: &Path) -> Result<(), String> {
        atomic_write(
            destination,
            &self.bytes,
            read_optional(destination)?.as_deref(),
        )
    }
    fn apply(&mut self, bytes: Vec<u8>, expected: Option<Vec<u8>>) -> Result<&Config, String> {
        let (config, document) = validate(&self.path, &bytes)?;
        atomic_write(&self.path, &bytes, expected.as_deref())?;
        self.config = config;
        self.document = document;
        self.bytes = bytes.clone();
        self.observed = Some(bytes);
        self.last_error = None;
        Ok(&self.config)
    }
}
fn defaults(path: &Path) -> Config {
    let mut config = Config::window_defaults();
    config.compose_words = path.parent().map(|parent| parent.join("compose-words.txt"));
    config
}
fn empty(path: &Path) -> Vec<u8> {
    if crate::config::is_json_path(path) {
        b"{}\n".to_vec()
    } else {
        crate::config_xml::write(&serde_json::json!({})).into_bytes()
    }
}
/// The file's Config and settings document; errors start with the file name ("config.xml line 4: ...").
fn validate(path: &Path, bytes: &[u8]) -> Result<(Config, Value), String> {
    let (config, document, soft) = validate_soft(path, bytes)?;
    match soft {
        Some(error) => Err(error),
        None => Ok((config, document)),
    }
}
/// As `validate`, except that a bad value of a setting that falls back on its own (new_window_panes)
/// gives the other settings and that value's error.
fn validate_soft(path: &Path, bytes: &[u8]) -> Result<(Config, Value, Option<String>), String> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let text = std::str::from_utf8(bytes).map_err(|_| format!("{name}: the file must be UTF-8"))?;
    let named = |e: String| {
        if e.starts_with("line ") {
            format!("{name} {e}")
        } else {
            format!("{name}: {e}")
        }
    };
    let (mut config, document, soft) = if crate::config::is_json_path(path) {
        let document: Value =
            serde_json::from_str(text).map_err(|e| format!("{name}: invalid JSON: {e}"))?;
        (
            Config::window_from_json(text).map_err(|e| format!("{name}: {e}"))?,
            document,
            None,
        )
    } else {
        crate::config_xml::load_soft(text).map_err(named)?
    };
    config.compose_words = path.parent().map(|parent| parent.join("compose-words.txt"));
    Ok((config, document, soft.map(named)))
}
fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.is_file() {
        return Err("settings path must be a regular file (not a symlink)".into());
    }
    if metadata.len() > MAX_BYTES {
        return Err("settings file exceeds 1 MiB".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("settings file exceeds 1 MiB".into());
    }
    Ok(Some(bytes))
}
fn atomic_write(path: &Path, bytes: &[u8], expected: Option<&[u8]>) -> Result<(), String> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err("settings file exceeds 1 MiB".into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temp = parent.join(format!(
        ".clanky-settings-{}-{}.tmp",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    crate::platform::private_file(&mut options);
    let mut file = options.open(&temp).map_err(|e| e.to_string())?;
    let result = (|| {
        if let Ok(metadata) = fs::metadata(path) {
            file.set_permissions(metadata.permissions())
                .map_err(|e| e.to_string())?;
        }
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        if read_optional(path)?.as_deref() != expected {
            return Err("settings changed during write; try again".into());
        }
        fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removed_settings_keeps_last_good() {
        let path =
            PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").expect("shared build directory"))
                .join(format!("settings-removed-{}.json", std::process::id()));
        fs::write(&path, b"{\"rails\":false,\"opacity\":0.8}").unwrap();
        let mut settings = Settings::open(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(settings.reload().is_err());
        assert_eq!(settings.config().opacity_alpha(), 204);
        assert!(!settings.reload().unwrap());
        settings.set("font_size", serde_json::json!(20)).unwrap();
        let config = Config::load(Some(&path)).unwrap();
        assert_eq!(config.opacity_alpha(), 204);
        assert!(!config.tools_panel);
        assert_eq!(config.font_size, 20.0);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn ordered_settings_round_trip() {
        let root = PathBuf::from(
            std::env::var_os("CARGO_TARGET_DIR").expect("use the shared build directory"),
        )
        .join(format!("settings-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("config.json");
        fs::write(
            &path,
            b"{\"rails\":false,\"opacity\":0.94,\"theme\":\"light\"}\n",
        )
        .unwrap();
        let mut settings = Settings::open(&path).unwrap();
        settings.set("opacity", serde_json::json!(0.8)).unwrap();
        if std::env::var_os("CLANKY_SETTINGS_FAULT").is_some() {
            fs::write(&path, b"{\"opacity\":1}").unwrap();
        }
        assert_eq!(Config::load(Some(&path)).unwrap().opacity_alpha(), 204);
        let bytes = fs::read(&path).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(
            text.find("rails").unwrap() < text.find("opacity").unwrap()
                && text.find("opacity").unwrap() < text.find("theme").unwrap()
        );
        let error = settings
            .set("opacity", serde_json::json!(0.3))
            .err()
            .unwrap();
        assert_eq!(error, Config::from_json(r#"{"opacity":0.3}"#).unwrap_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        settings.set_color("panel", Some("#202020")).unwrap();
        assert_eq!(
            settings.config().resolved_theme(false).panel,
            [32, 32, 32, 255]
        );
        let export = root.join("export.json");
        settings.export(&export).unwrap();
        settings.reset("rails").unwrap();
        settings.import(&export).unwrap();
        assert_eq!(fs::read(&path).unwrap(), fs::read(&export).unwrap());
        fs::write(&path, b"{\"opacity\":0.3}").unwrap();
        assert!(settings.reload().is_err());
        assert_eq!(settings.config().opacity, 0.8);
        assert!(!settings.reload().unwrap());
        fs::write(&path, b"{\"opacity\":1}").unwrap();
        assert!(settings.reload().unwrap());
        assert_eq!(settings.config().opacity_alpha(), 255);
        fs::remove_dir_all(root).unwrap();
    }
}
