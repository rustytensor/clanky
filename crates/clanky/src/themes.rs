//! M12 theme documents. Loading is strict and independent of window/GPU state.
//! The old look::Theme stays separate until the app adopts these semantic tokens.
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub type Color = [u8; 4];
pub const MAX_THEME_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    Dark,
    Light,
}
impl Appearance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

macro_rules! colors {
    ($($field:ident),+ $(,)?) => {
        pub const COLOR_TOKENS: &[&str] = &[$(stringify!($field)),+];
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct Colors { $(pub $field: Color),+ }
        impl Colors {
            pub fn get(&self, token: &str) -> Option<Color> {
                match token { $(stringify!($field) => Some(self.$field),)+ _ => None }
            }
            pub fn set(&mut self, token: &str, value: Color) -> Result<(), String> {
                match token {
                    $(stringify!($field) => self.$field = value,)+
                    _ => return Err("unknown theme color token".into()),
                }
                Ok(())
            }
            fn from_value(value: &Value) -> Result<Self, String> {
                let object = value.as_object().ok_or("theme colors must be an object")?;
                if object.keys().any(|key| !COLOR_TOKENS.contains(&key.as_str())) {
                    return Err("unknown theme color token".into());
                }
                let get = |token: &str| -> Result<Color, String> {
                    let value = object.get(token)
                        .ok_or_else(|| format!("theme colors missing required token: {token}"))?;
                    parse_color(value.as_str().ok_or_else(|| format!("theme color {token} must be a string"))?)
                        .map_err(|_| format!("theme color {token} must be #rrggbb or #rrggbbaa"))
                };
                Ok(Self { $($field: get(stringify!($field))?),+ })
            }
            fn to_value(&self) -> Value {
                let mut object = Map::new();
                $(object.insert(stringify!($field).into(), json!(format_color(self.$field)));)+
                Value::Object(object)
            }
        }
    }
}
colors!(
    sidebar,
    background,
    card,
    muted,
    foreground,
    muted_foreground,
    accent,
    pressed,
    border,
    input,
    ring,
    primary,
    primary_foreground,
    sidebar_accent,
    ok,
    err,
    agent,
    robot_paint
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub name: String,
    pub appearance: Appearance,
    pub colors: Colors,
    /// Standard ANSI order: black, red, green, yellow, blue, magenta, cyan, white;
    /// then their bright variants. Values are straight RGBA, like all tokens.
    pub ansi: [Color; 16],
    /// Two to eight straight-RGBA stops, spread evenly from top to bottom.
    pub window_gradient: Option<Vec<Color>>,
    pub glow: bool,
    pub description: Option<String>,
}
impl Theme {
    pub fn from_json(source: &str) -> Result<Self, String> {
        if source.len() > MAX_THEME_BYTES {
            return Err("theme exceeds 64 KiB".into());
        }
        let value: Value = serde_json::from_str(source).map_err(|_| "theme must be valid JSON")?;
        let object = value.as_object().ok_or("theme must be an object")?;
        let allowed = [
            "name",
            "appearance",
            "colors",
            "ansi",
            "window_gradient",
            "glow",
            "description",
        ];
        if object.keys().any(|key| !allowed.contains(&key.as_str())) {
            return Err("unknown theme field".into());
        }
        let required = |key: &str| {
            object
                .get(key)
                .ok_or_else(|| format!("theme missing required field: {key}"))
        };
        let name = required("name")?
            .as_str()
            .ok_or("theme name must be a string")?
            .to_owned();
        let appearance = match required("appearance")?.as_str() {
            Some("dark") => Appearance::Dark,
            Some("light") => Appearance::Light,
            _ => return Err("theme appearance must be dark or light".into()),
        };
        let colors = Colors::from_value(required("colors")?)?;
        let ansi = color_array::<16>(required("ansi")?, "theme ansi must contain 16 colors")?;
        let window_gradient = match required("window_gradient")? {
            Value::Null => None,
            value => {
                let stops = value
                    .as_array()
                    .filter(|stops| (2..=8).contains(&stops.len()))
                    .ok_or("theme window_gradient must be null or 2 to 8 colors")?;
                Some(
                    stops
                        .iter()
                        .map(|value| {
                            parse_color(
                                value
                                    .as_str()
                                    .ok_or("theme gradient stops must be colors")?,
                            )
                        })
                        .collect::<Result<_, String>>()?,
                )
            }
        };
        let glow = required("glow")?
            .as_bool()
            .ok_or("theme glow must be boolean")?;
        let description = match object.get("description") {
            None | Some(Value::Null) => None,
            Some(Value::String(text)) => Some(text.clone()),
            _ => return Err("theme description must be a string".into()),
        };
        let theme = Self {
            name,
            appearance,
            colors,
            ansi,
            window_gradient,
            glow,
            description,
        };
        theme.validate()?;
        Ok(theme)
    }

    /// Canonical UTF-8 JSON, with one final newline. Editing a built-in produces
    /// an independent value; embedded documents are never changed.
    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        let mut value = json!({
            "name": self.name, "appearance": self.appearance.as_str(),
            "colors": self.colors.to_value(),
            "ansi": self.ansi.map(format_color),
            "window_gradient": self.window_gradient.as_ref().map(|colors| colors.iter().copied().map(format_color).collect::<Vec<_>>()),
            "glow": self.glow,
        });
        if let Some(description) = &self.description {
            value["description"] = json!(description);
        }
        let mut source = serde_json::to_string_pretty(&value).map_err(|_| "cannot encode theme")?;
        source.push('\n');
        if source.len() > MAX_THEME_BYTES {
            return Err("theme exceeds 64 KiB".into());
        }
        Ok(source)
    }

    /// Apply config overrides transactionally, leaving the original theme intact.
    pub fn with_overrides(&self, overrides: &BTreeMap<String, String>) -> Result<Self, String> {
        let mut theme = self.clone();
        for (token, value) in overrides {
            theme.colors.set(token, parse_color(value)?)?;
        }
        Ok(theme)
    }

    fn validate(&self) -> Result<(), String> {
        if self
            .window_gradient
            .as_ref()
            .is_some_and(|stops| !(2..=8).contains(&stops.len()))
        {
            return Err("theme window_gradient must be null or 2 to 8 colors".into());
        }
        if self.name.trim().is_empty()
            || self.name.len() > 128
            || self.name.chars().any(char::is_control)
        {
            return Err("theme name must be 1 to 128 bytes without control characters".into());
        }
        if self
            .description
            .as_ref()
            .is_some_and(|s| s.len() > 1024 || s.chars().any(char::is_control))
        {
            return Err(
                "theme description must be at most 1024 bytes without control characters".into(),
            );
        }
        Ok(())
    }
}

/// Strict ASCII hex, straight alpha; six digits imply opaque alpha.
pub fn parse_color(text: &str) -> Result<Color, String> {
    let bytes = text.as_bytes();
    if !matches!(bytes.len(), 7 | 9)
        || bytes[0] != b'#'
        || !bytes[1..].iter().all(u8::is_ascii_hexdigit)
    {
        return Err("color must be #rrggbb or #rrggbbaa".into());
    }
    let mut color = [255; 4];
    for (index, pair) in bytes[1..].chunks_exact(2).enumerate() {
        let digit = |b: u8| {
            if b.is_ascii_digit() {
                b - b'0'
            } else {
                b.to_ascii_lowercase() - b'a' + 10
            }
        };
        color[index] = digit(pair[0]) * 16 + digit(pair[1]);
    }
    Ok(color)
}
pub fn format_color(color: Color) -> String {
    if color[3] == 255 {
        format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            color[0], color[1], color[2], color[3]
        )
    }
}
fn color_array<const N: usize>(value: &Value, message: &str) -> Result<[Color; N], String> {
    let values = value
        .as_array()
        .filter(|values| values.len() == N)
        .ok_or(message)?;
    let mut colors = [[0; 4]; N];
    for (color, value) in colors.iter_mut().zip(values) {
        *color = parse_color(value.as_str().ok_or(message)?).map_err(|_| message)?;
    }
    Ok(colors)
}

#[derive(Clone, Copy, Debug)]
pub struct Builtin {
    /// Stable picker/config identifier; never derive identifiers from labels.
    pub id: &'static str,
    pub name: &'static str,
    pub appearance: Appearance,
    source: &'static str,
}
impl Builtin {
    pub fn load(&self) -> Result<Theme, String> {
        Theme::from_json(self.source)
    }
}
pub const BUILTINS: &[Builtin] = &[
    Builtin {
        id: "stone-dark",
        name: "Stone",
        appearance: Appearance::Dark,
        source: include_str!("../../../assets/themes/stone-dark.json"),
    },
    Builtin {
        id: "stone-light",
        name: "Stone",
        appearance: Appearance::Light,
        source: include_str!("../../../assets/themes/stone-light.json"),
    },
];
pub fn builtin(id: &str) -> Result<Theme, String> {
    BUILTINS
        .iter()
        .find(|theme| theme.id == id)
        .ok_or("unknown built-in theme")?
        .load()
}
pub fn default_id(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::Dark => "stone-dark",
        Appearance::Light => "stone-light",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Literal expected channels are transcribed from the approved component
    // sheets, not produced by the loader or by a rendering result.
    #[test]
    fn stone_documents() {
        let mut source = BUILTINS[0].source.to_owned();
        if std::env::var_os("CLANKY_THEME_FAULT").is_some() {
            source = source.replace("#141210", "#000000");
        }
        let dark = Theme::from_json(&source).unwrap();
        let light = builtin("stone-light").unwrap();
        let expected_dark = [
            [20, 18, 16, 255],
            [26, 24, 22, 255],
            [32, 29, 26, 255],
            [38, 34, 31, 255],
            [235, 230, 223, 255],
            [160, 151, 141, 255],
            [43, 39, 36, 255],
            [51, 46, 42, 255],
            [255, 240, 225, 20],
            [255, 240, 225, 31],
            [138, 129, 120, 255],
            [235, 230, 223, 255],
            [26, 24, 22, 255],
            [34, 31, 28, 255],
            [147, 184, 154, 255],
            [217, 139, 126, 255],
            [210, 164, 118, 255],
            [107, 82, 54, 255], // robot6, Stone dark #6b5236
        ];
        let expected_light = [
            [243, 240, 235, 255],
            [251, 250, 248, 255],
            [255, 255, 255, 255],
            [243, 240, 235, 255],
            [42, 38, 34, 255],
            [119, 110, 100, 255],
            [239, 235, 229, 255],
            [230, 224, 216, 255],
            [231, 225, 217, 255],
            [221, 214, 204, 255],
            [163, 154, 143, 255],
            [42, 38, 34, 255],
            [251, 250, 248, 255],
            [233, 228, 220, 255],
            [77, 122, 86, 255],
            [176, 80, 61, 255],
            [154, 98, 41, 255],
            [227, 192, 142, 255], // robot6, Stone light #e3c08e
        ];
        for (theme, expected) in [(&dark, expected_dark), (&light, expected_light)] {
            for (token, rgba) in COLOR_TOKENS.iter().zip(expected) {
                assert_eq!(theme.colors.get(token), Some(rgba), "{token}");
            }
            assert!(!theme.glow);
            assert_eq!(theme.window_gradient, None);
        }
        assert_eq!(dark.appearance, Appearance::Dark);
        assert_eq!(light.appearance, Appearance::Light);
        assert_eq!(parse_color("#1a181680").unwrap(), [26, 24, 22, 128]);
        let mut broken: Value = serde_json::from_str(BUILTINS[0].source).unwrap();
        broken["colors"].as_object_mut().unwrap().remove("ring");
        assert_eq!(
            Theme::from_json(&broken.to_string()).unwrap_err(),
            "theme colors missing required token: ring"
        );
        let mut gradient = serde_json::from_str::<Value>(BUILTINS[0].source).unwrap();
        gradient["window_gradient"] = json!([
            "#ff0000", "#ff8000", "#ffff00", "#00ff00", "#0000ff", "#8000ff"
        ]);
        let gradient = Theme::from_json(&gradient.to_string()).unwrap();
        assert_eq!(
            gradient.window_gradient,
            Some(vec![
                [255, 0, 0, 255],
                [255, 128, 0, 255],
                [255, 255, 0, 255],
                [0, 255, 0, 255],
                [0, 0, 255, 255],
                [128, 0, 255, 255],
            ])
        );
        println!(
            "PASS: Stone dark/light match all 18 tokens; missing ring refused; six gradient stops preserved"
        );
    }
}
