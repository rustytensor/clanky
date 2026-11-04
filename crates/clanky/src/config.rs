//! Startup configuration and session-only font sizing.
use std::{collections::BTreeMap, fs, io::ErrorKind, path::{Path, PathBuf}};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemePreference { Dark, Light, #[default] System }

#[derive(Clone, Debug)]
pub struct Config {
    pub font_size_pt: f32,
    pub font_size: f32,
    pub theme: ThemePreference,
    pub colors: BTreeMap<String, crate::look::Color>,
    pub columns: usize,
    pub rows: usize,
    pub compose_dictionary: String,
    pub compose_dictionary_dir: PathBuf,
    pub compose_send_enter: bool,
    pub compose_words: Option<PathBuf>,
    pub osc52_write: bool,
    pub osc52_read: bool,
    pub notify_after_seconds: u64,
    pub restore_layout: bool,
    pub opacity: f32,
    pub animations: bool,
    pub rails: bool,
    pub font_family: String,
    pub ui_font_family: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font_size_pt: 12.0,
            font_size: 16.0,
            theme: ThemePreference::System,
            colors: BTreeMap::new(),
            columns: 80,
            rows: 24,
            compose_dictionary: "en_US".into(),
            compose_dictionary_dir: PathBuf::from("/usr/share/hunspell"),
            compose_send_enter: false,
            compose_words: None,
            osc52_write: true,
            osc52_read: false,
            notify_after_seconds: 10,
            restore_layout: true,
            opacity: 0.94,
            animations: true,
            rails: true,
            font_family: "JetBrains Mono".into(),
            ui_font_family: "Inter".into(),
        }
    }
}

impl Config {
    pub fn resolved_theme(&self, system_light: bool) -> crate::look::Theme {
        let light = match self.theme {
            ThemePreference::Light => true, ThemePreference::Dark => false,
            ThemePreference::System => system_light,
        };
        let mut theme = if light { crate::look::Theme::light() } else { crate::look::Theme::dark() };
        for (token, color) in &self.colors {
            // from_json has already validated each token.
            let _ = theme.override_color(token, *color);
        }
        theme
    }
    /// Straight alpha byte for scene clear; renderer reports opaque-only surfaces.
    pub fn opacity_alpha(&self) -> u8 {
        let opacity = if self.opacity.is_finite() { self.opacity.clamp(0.5, 1.0) } else { 0.94 };
        (opacity * 255.0).round() as u8
    }

    pub fn load(explicit: Option<&Path>) -> Result<Self, String> {
        let default_path;
        let path = match explicit {
            Some(path) => path,
            None => {
                let Some(path) = crate::platform::settings_path() else {
                    return Ok(Self::default());
                };
                default_path = path;
                &default_path
            }
        };
        let mut config = match fs::read_to_string(path) {
            Ok(json) => Self::from_json(&json)
                .map_err(|error| format!("config {}: {error}", path.display())),
            Err(error) if explicit.is_none() && error.kind() == ErrorKind::NotFound => {
                Ok(Self::default())
            }
            Err(error) => Err(format!("config {}: {error}", path.display())),
        }?;
        config.compose_words = path.parent().map(|parent| parent.join("compose-words.txt"));
        Ok(config)
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|error| format!("invalid JSON: {error}"))?;
        let object = value.as_object().ok_or("config must be a JSON object")?;
        let mut config = Self::default();
        for (key, value) in object {
            match key.as_str() {
                "theme" => {
                    config.theme = match value.as_str() {
                        Some("dark") => ThemePreference::Dark,
                        Some("light") => ThemePreference::Light,
                        Some("system") => ThemePreference::System,
                        _ => return Err("theme must be dark, light or system".into()),
                    };
                }
                "colors" => {
                    let colors = value.as_object().ok_or("colors must be an object")?;
                    for (token, value) in colors {
                        if !crate::look::COLOR_TOKENS.contains(&token.as_str()) {
                            return Err(format!("unknown color token: {token}"));
                        }
                        let text = value.as_str().filter(|text| text.len() == 7 && text.starts_with('#')
                            && text.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit))
                            .ok_or_else(|| format!("colors.{token} must be #rrggbb"))?;
                        let channel = |start| u8::from_str_radix(&text[start..start+2], 16).unwrap();
                        config.colors.insert(token.clone(), [channel(1),channel(3),channel(5),255]);
                    }
                }
                "font_size" => {
                    let pixels = value.as_f64().ok_or("font_size must be a number")?;
                    if !pixels.is_finite() || !(8.0..=32.0).contains(&pixels) {
                        return Err("font_size must be finite and between 8 and 32".into());
                    }
                    config.font_size = pixels as f32;
                }
                "font_size_pt" => {
                    let points = value.as_f64().ok_or("font_size_pt must be a number")?;
                    if !points.is_finite() || !(6.0..=48.0).contains(&points) {
                        return Err("font_size_pt must be finite and between 6 and 48".into());
                    }
                    config.font_size_pt = points as f32;
                }
                "columns" | "rows" => {
                    let dimension = value
                        .as_u64()
                        .filter(|n| (1..=1000).contains(n))
                        .ok_or_else(|| format!("{key} must be an integer between 1 and 1000"))?
                        as usize;
                    if key == "columns" {
                        config.columns = dimension;
                    } else {
                        config.rows = dimension;
                    }
                }
                "compose_dictionary" => {
                    let language = value.as_str().ok_or("compose_dictionary must be a string")?;
                    if language.is_empty() || language.len() > 64
                        || !language.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                    {
                        return Err("compose_dictionary must be a safe locale name (letters, digits, _ or -)".into());
                    }
                    config.compose_dictionary = language.into();
                }
                "compose_dictionary_dir" => {
                    let dir = PathBuf::from(value.as_str().ok_or("compose_dictionary_dir must be a string")?);
                    if !dir.is_absolute() { return Err("compose_dictionary_dir must be absolute".into()); }
                    config.compose_dictionary_dir = dir;
                }
                "osc52_write" => {
                    config.osc52_write = value.as_bool().ok_or("osc52_write must be a boolean")?;
                }
                "osc52_read" => {
                    config.osc52_read = value.as_bool().ok_or("osc52_read must be a boolean")?;
                }
                "compose_send_enter" => {
                    config.compose_send_enter = value.as_bool().ok_or("compose_send_enter must be a boolean")?;
                }
                "notify_after_seconds" => {
                    config.notify_after_seconds = value.as_u64().ok_or("notify_after_seconds must be a nonnegative integer")?;
                }
                "restore_layout" => {
                    config.restore_layout = value.as_bool().ok_or("restore_layout must be a boolean")?;
                }
                "opacity" => {
                    let opacity = value.as_f64().ok_or("opacity must be a number")?;
                    if !opacity.is_finite() || !(0.5..=1.0).contains(&opacity) {
                        return Err("opacity must be finite and between 0.5 and 1.0".into());
                    }
                    config.opacity = opacity as f32;
                }
                "animations" | "rails" => {
                    let enabled = value.as_bool().ok_or_else(|| format!("{key} must be a boolean"))?;
                    if key == "animations" { config.animations = enabled; } else { config.rails = enabled; }
                }
                "font_family" | "ui_font_family" => {
                    let family = value.as_str().ok_or_else(|| format!("{key} must be a string"))?;
                    if family.trim().is_empty() || family.len() > 256 || family.chars().any(char::is_control) {
                        return Err(format!("{key} must be a nonempty font family of at most 256 bytes without controls"));
                    }
                    if key == "font_family" { config.font_family = family.into(); } else { config.ui_font_family = family.into(); }
                }
                _ => return Err(format!("unknown config field: {key}")),
            }
        }
        // The new logical-pixel key wins independent of JSON key order.
        if object.contains_key("font_size") { config.font_size_pt = config.font_size * 0.75; }
        else { config.font_size = config.font_size_pt * 4.0 / 3.0; }
        // Match the core's per-axis and total-cell limits.
        if config.columns * config.rows > 250_000 {
            return Err("terminal dimensions exceed 250000 cells".into());
        }
        Ok(config)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub font_px: f32,
    pub cell_width: u32,
    pub cell_height: u32,
}

#[derive(Clone, Debug)]
pub struct Zoom {
    configured_pt: f32,
    points: f32,
}

impl Zoom {
    pub fn new(configured_pt: f32) -> Self {
        let configured_pt = if configured_pt.is_finite() {
            configured_pt.clamp(6.0, 48.0)
        } else {
            12.0
        };
        Self {
            configured_pt,
            points: configured_pt,
        }
    }

    pub fn adjust(&mut self, delta: f32) {
        if !delta.is_nan() {
            self.points = (self.points + delta).clamp(6.0, 48.0);
        }
    }

    pub fn reset(&mut self) {
        self.points = self.configured_pt;
    }

    pub fn points(&self) -> f32 {
        self.points
    }

    pub fn metrics(&self, scale: f64) -> Result<Metrics, String> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err("window scale must be finite and positive".into());
        }
        // Do not round to f32 before the ceiling: e.g. 12pt at 1.5 is exactly 24px.
        let pixels = f64::from(self.points) * scale * 4.0 / 3.0;
        let cell = |factor: f64| -> Result<u32, String> {
            let size = pixels * factor;
            // Remove only floating-point arithmetic noise at integer boundaries.
            let nearest = size.round();
            let size = if (size - nearest).abs() <= 4.0 * f64::EPSILON * size.abs() {
                nearest
            } else {
                size.ceil()
            };
            if !size.is_finite() || size < 1.0 || size > f64::from(u32::MAX) {
                return Err("physical cell size is out of range".into());
            }
            Ok(size as u32)
        };
        let font_px = pixels as f32;
        if !font_px.is_finite() || font_px <= 0.0 {
            return Err("physical font size is out of range".into());
        }
        Ok(Metrics {
            font_px,
            cell_width: cell(0.625)?,
            cell_height: cell(1.25)?,
        })
    }
}
