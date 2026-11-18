//! Built-in terminal colour schemes and import of other terminals' colour files.
//!
//! Every scheme and every import gives the terminal's 20 colours: background, foreground, cursor,
//! selection and the 16 ANSI colours. A file without a cursor colour draws the cursor in its
//! foreground; one without a selection colour gets the colour a quarter of the way from its
//! background to its foreground (as the built-in schemes whose sources give none).
use crate::config::TerminalColors;
use crate::themes::Color;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::OnceLock};

/// One built-in scheme: its config-safe id, the name the picker shows, and its colours.
#[derive(Clone, Debug)]
pub struct Scheme { pub id: String, pub name: String, pub colors: TerminalColors }

const BUILTIN_JSON: &str = include_str!("../../../assets/terminal-schemes/schemes.json");

/// The built-in schemes in picker order: the terminals' own defaults first, then themes by name.
pub fn builtin() -> &'static [Scheme] {
    static LIST: OnceLock<Vec<Scheme>> = OnceLock::new();
    LIST.get_or_init(|| {
        let list: Vec<Value> = serde_json::from_str(BUILTIN_JSON).expect("assets/terminal-schemes/schemes.json is a JSON list");
        list.iter().map(|entry| {
            let mut value = entry.clone();
            let object = value.as_object_mut().expect("scheme object");
            let id = object.shift_remove("id").and_then(|v| v.as_str().map(str::to_owned)).expect("scheme id");
            let name = object.shift_remove("name").and_then(|v| v.as_str().map(str::to_owned)).expect("scheme name");
            let colors = TerminalColors::from_value(&value).unwrap_or_else(|e| panic!("scheme {id}: {e}"));
            Scheme { id, name, colors }
        }).collect()
    })
}

fn hex(c: Color) -> String { format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]) }

/// terminal_colors as the config file stores it: only the colours that are set, lowercase #rrggbb.
pub fn to_value(colors: &TerminalColors) -> Value {
    let mut value = json!({});
    for (key, color) in [("background", colors.background), ("foreground", colors.foreground), ("cursor", colors.cursor), ("selection", colors.selection)] {
        if let Some(color) = color { value[key] = json!(hex(color)); }
    }
    if let Some(ansi) = colors.ansi { value["ansi"] = json!(ansi.iter().map(|c| hex(*c)).collect::<Vec<_>>()); }
    value
}

/// The selection colour Clanky uses when a scheme gives none.
pub fn default_selection(background: Color, foreground: Color) -> Color {
    std::array::from_fn(|i| if i == 3 { 255 } else { (background[i] as f32 + (foreground[i] as f32 - background[i] as f32) * 0.25).round() as u8 })
}

/// Colours read from a file before the missing ones are filled in.
#[derive(Default)]
struct Found { background: Option<Color>, foreground: Option<Color>, cursor: Option<Color>, selection: Option<Color>, ansi: [Option<Color>; 16] }
impl Found {
    fn finish(self, format: &str) -> Result<TerminalColors, String> {
        let missing: Vec<String> = self.ansi.iter().enumerate().filter(|(_, c)| c.is_none()).map(|(i, _)| format!("colour {i}")).collect();
        let (Some(background), Some(foreground)) = (self.background, self.foreground) else {
            return Err(format!("not a colour scheme: no background and foreground colours ({format})"));
        };
        if !missing.is_empty() { return Err(format!("not a colour scheme: {format} file without {}", missing.join(", "))); }
        Ok(TerminalColors { background: Some(background), foreground: Some(foreground), cursor: Some(self.cursor.unwrap_or(foreground)),
            selection: Some(self.selection.unwrap_or_else(|| default_selection(background, foreground))),
            ansi: Some(self.ansi.map(|c| c.unwrap_or([0, 0, 0, 255]))) })
    }
}

/// `#rrggbb`, `rrggbb` or `0xrrggbb` (quotes and spaces around it are ignored).
fn color(text: &str) -> Option<Color> {
    let text = text.trim().trim_matches(|c| c == '"' || c == '\'').trim();
    let digits = text.strip_prefix('#').or_else(|| text.strip_prefix("0x")).or_else(|| text.strip_prefix("0X")).unwrap_or(text);
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) { return None; }
    let n = u32::from_str_radix(digits, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8, 255])
}

/// Import FILE as terminal_colors (all 20 colours), telling the format apart by content and extension.
pub fn import_file(path: &Path) -> Result<Value, String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
    let folder = path.parent().filter(|p| !p.as_os_str().is_empty()).map(|p| p.display().to_string()).unwrap_or_else(|| ".".into());
    let unreadable = |e: std::io::Error| match e.kind() {
        std::io::ErrorKind::NotFound => format!("There is no file named {name} in {folder}."),
        std::io::ErrorKind::PermissionDenied => format!("Clanky may not read {name}: permission denied."),
        _ if path.is_dir() => format!("{} is a folder. Add the colour file's name after it.", path.display()),
        _ => format!("Clanky could not read {name}: {e}."),
    };
    if path.as_os_str().is_empty() { return Err("Enter the path of a colour file.".into()); }
    if path.is_dir() { return Err(format!("{} is a folder. Add the colour file's name after it.", path.display())); }
    let meta = std::fs::metadata(path).map_err(unreadable)?;
    if meta.len() > 1024 * 1024 { return Err(format!("{name} is not a colour scheme (it is larger than 1 MiB).")); }
    let text = std::fs::read_to_string(path).map_err(|e| if e.kind() == std::io::ErrorKind::InvalidData { format!("{name} is not a colour scheme (it is not text).") } else { unreadable(e) })?;
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    import_text(&text, &ext).map(|colors| to_value(&colors)).map_err(|e| format!("{name}: {e}"))
}

/// The folder next to the config file that keeps imported schemes, so the picker lists them.
pub fn imported_dir(config: &Path) -> std::path::PathBuf {
    config.parent().unwrap_or(Path::new(".")).join("terminal-schemes")
}

/// Keep an imported scheme as `<folder>/<file name>.json` ({"name", "terminal_colors"}); returns the name
/// the picker shows: the source file's name without its extension.
pub fn keep_imported(config: &Path, source: &Path, colors: &Value) -> Result<String, String> {
    let name = source.file_stem().map(|n| n.to_string_lossy().trim().to_owned()).filter(|n| !n.is_empty()).unwrap_or_else(|| "Imported colours".into());
    let slug: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' { c } else { '-' }).collect();
    let dir = imported_dir(config);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let text = serde_json::to_string_pretty(&json!({"name": name, "terminal_colors": colors})).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{}.json", slug.trim_start_matches('.'))), text + "\n").map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(name)
}

/// The imported schemes kept next to the config file, by name. Files that do not read are left out.
pub fn imported(config: &Path) -> Vec<Scheme> {
    let Ok(entries) = std::fs::read_dir(imported_dir(config)) else { return Vec::new(); };
    let mut out: Vec<Scheme> = entries.filter_map(Result::ok).filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter(|e| e.metadata().is_ok_and(|m| m.is_file() && m.len() <= 64 * 1024))
        .filter_map(|e| {
            let value: Value = serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok()?;
            let name = value.get("name")?.as_str()?.chars().filter(|c| !c.is_control()).take(80).collect::<String>();
            let colors = TerminalColors::from_value(value.get("terminal_colors")?).ok()?;
            Some(Scheme { id: e.path().file_stem()?.to_string_lossy().into_owned(), name, colors })
        }).collect();
    out.sort_by_key(|s| s.name.to_lowercase());
    out
}

pub fn import_text(text: &str, ext: &str) -> Result<TerminalColors, String> {
    let trimmed = text.trim_start();
    let has_line = |f: &dyn Fn(&str) -> bool| text.lines().map(str::trim).any(|l| f(l));
    if ext == "itermcolors" || trimmed.starts_with("<?xml") || trimmed.starts_with("<plist") || trimmed.starts_with("<!DOCTYPE plist") { return iterm(text); }
    if ext == "json" || trimmed.starts_with('{') { return windows_terminal(text); }
    if ext == "palette" || (has_line(&|l| l == "[Dark]") && has_line(&|l| l.starts_with("Color0="))) { return ptyxis(text); }
    if ext == "colorscheme" || (has_line(&|l| l == "[Background]") && has_line(&|l| l.starts_with("Color="))) { return konsole(text); }
    if ext == "toml" || has_line(&|l| l.starts_with("[colors")) { return toml_scheme(text); }
    if ext == "yml" || ext == "yaml" || has_line(&|l| l == "colors:") { return alacritty(&yaml_flat(text), "Alacritty YAML"); }
    if has_line(&|l| l.starts_with("palette") && l.contains('=')) { return ghostty(text); }
    if has_line(&|l| l.contains(':') && !l.starts_with('!') && (l.starts_with('*') || l.contains(".color") || l.contains("*color") || l.contains(".background") || l.contains("*background"))) { return xresources(text); }
    kitty(text)
}

// ---------------------------------------------------------------- iTerm2 .itermcolors (XML plist)

fn iterm(text: &str) -> Result<TerminalColors, String> {
    // Tokens: (tag, text that follows it). Only the outer dict's key -> inner dict pairs matter.
    let mut tokens = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        let Some(end) = rest[start..].find('>') else { break; };
        let tag = &rest[start + 1..start + end];
        let after = &rest[start + end + 1..];
        let body = &after[..after.find('<').unwrap_or(after.len())];
        if !tag.starts_with('?') && !tag.starts_with('!') { tokens.push((tag.trim().to_owned(), body.trim().to_owned())); }
        rest = after;
    }
    let mut depth = 0usize;
    let mut key = String::new();
    let mut entries: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut current: Option<(String, BTreeMap<String, String>)> = None;
    let mut inner_key = String::new();
    for (tag, body) in &tokens {
        match tag.as_str() {
            "dict" => { depth += 1; if depth == 2 { current = Some((key.clone(), BTreeMap::new())); } }
            "/dict" => { if depth == 2 { if let Some((k, v)) = current.take() { entries.insert(k, v); } } depth = depth.saturating_sub(1); }
            "key" if depth == 1 => key = body.clone(),
            "key" if depth == 2 => inner_key = body.clone(),
            "real" | "integer" | "string" if depth == 2 => { if let Some((_, map)) = current.as_mut() { map.insert(inner_key.clone(), body.clone()); } }
            _ => {}
        }
    }
    if entries.is_empty() { return Err("not a colour scheme: an XML file without iTerm2 colours".into()); }
    let get = |name: &str| -> Option<Color> {
        let map = entries.get(name)?;
        let c = |k: &str| map.get(k).and_then(|v| v.parse::<f64>().ok()).map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
        Some([c("Red Component")?, c("Green Component")?, c("Blue Component")?, 255])
    };
    let mut found = Found { background: get("Background Color"), foreground: get("Foreground Color"), cursor: get("Cursor Color"), selection: get("Selection Color"), ..Found::default() };
    for (i, slot) in found.ansi.iter_mut().enumerate() { *slot = get(&format!("Ansi {i} Color")); }
    found.finish("iTerm2")
}

// ---------------------------------------------------------------- Windows Terminal JSON scheme

fn windows_terminal(text: &str) -> Result<TerminalColors, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("not a colour scheme: invalid JSON ({e})"))?;
    // A settings.json with a "schemes" list: its first scheme.
    let scheme = value.get("schemes").and_then(|s| s.as_array()).and_then(|s| s.first()).unwrap_or(&value);
    let get = |k: &str| scheme.get(k).and_then(|v| v.as_str()).and_then(color);
    let names = ["black", "red", "green", "yellow", "blue", "purple", "cyan", "white"];
    let mut found = Found { background: get("background"), foreground: get("foreground"), cursor: get("cursorColor"), selection: get("selectionBackground"), ..Found::default() };
    for (i, name) in names.iter().enumerate() {
        found.ansi[i] = get(name);
        found.ansi[i + 8] = get(&format!("bright{}{}", name[..1].to_uppercase(), &name[1..]));
    }
    found.finish("Windows Terminal")
}

// ---------------------------------------------------------------- INI-style files: Ptyxis and Konsole

fn ini(text: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut section = String::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') { section = line[1..line.len() - 1].trim().to_owned(); continue; }
        if line.starts_with('#') || line.starts_with(';') { continue; }
        if let Some((k, v)) = line.split_once('=') { out.entry(section.clone()).or_default().insert(k.trim().to_owned(), v.trim().to_owned()); }
    }
    out
}

fn ptyxis(text: &str) -> Result<TerminalColors, String> {
    let sections = ini(text);
    let dark = sections.get("Dark").ok_or("not a colour scheme: a Ptyxis palette without a [Dark] section")?;
    let get = |k: &str| dark.get(k).and_then(|v| color(v));
    let mut found = Found { background: get("Background"), foreground: get("Foreground"), cursor: get("Cursor"), ..Found::default() };
    for (i, slot) in found.ansi.iter_mut().enumerate() { *slot = get(&format!("Color{i}")); }
    found.finish("Ptyxis")
}

fn konsole(text: &str) -> Result<TerminalColors, String> {
    let sections = ini(text);
    let get = |s: &str| -> Option<Color> {
        let parts: Vec<u8> = sections.get(s)?.get("Color")?.split(',').take(3).map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
        (parts.len() == 3).then(|| [parts[0], parts[1], parts[2], 255])
    };
    let mut found = Found { background: get("Background"), foreground: get("Foreground"), ..Found::default() };
    for i in 0..8 { found.ansi[i] = get(&format!("Color{i}")); found.ansi[i + 8] = get(&format!("Color{i}Intense")); }
    found.finish("Konsole")
}

// ---------------------------------------------------------------- TOML (Alacritty and WezTerm)

/// A small TOML reader for colour files: `[section]` heads, `key = "string"` and arrays of strings
/// (over several lines too), as dotted keys. Anything else is skipped.
fn toml_flat(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    let mut section = String::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if line.starts_with('[') {
            section = line.trim_start_matches('[').split(']').next().unwrap_or("").trim().to_owned();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue; };
        let key = key.trim().trim_matches('"').to_owned();
        let mut value = value.trim().to_owned();
        if value.starts_with('[') {
            while !strip_comment(&value).trim_end().ends_with(']') { let Some(more) = lines.next() else { break; }; value.push(' '); value.push_str(more.trim()); }
        }
        let full = if section.is_empty() { key } else { format!("{section}.{key}") };
        out.insert(full, strings(&strip_comment(&value)));
    }
    out
}
/// The text before a `#` comment that starts outside quotes.
fn strip_comment(text: &str) -> String {
    let mut quote = None;
    for (i, c) in text.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '#') => return text[..i].to_owned(),
            _ => {}
        }
    }
    text.to_owned()
}
/// The quoted strings in a value (one for a string, several for an array).
fn strings(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = value;
    while let Some(start) = rest.find(['"', '\'']) {
        let quote = &rest[start..start + 1];
        let Some(end) = rest[start + 1..].find(quote) else { break; };
        out.push(rest[start + 1..start + 1 + end].to_owned());
        rest = &rest[start + 2 + end..];
    }
    out
}

fn toml_scheme(text: &str) -> Result<TerminalColors, String> {
    let flat = toml_flat(text);
    if flat.contains_key("colors.ansi") {
        let one = |k: &str| flat.get(k).and_then(|v| v.first()).and_then(|v| color(v));
        let list = |k: &str| flat.get(k).map(|v| v.iter().map(|c| color(c)).collect::<Vec<_>>()).unwrap_or_default();
        let mut found = Found { background: one("colors.background"), foreground: one("colors.foreground"), cursor: one("colors.cursor_bg"), selection: one("colors.selection_bg"), ..Found::default() };
        for (i, c) in list("colors.ansi").into_iter().chain(std::iter::repeat(None).take(8)).take(8).chain(list("colors.brights").into_iter().chain(std::iter::repeat(None).take(8)).take(8)).enumerate() { found.ansi[i] = c; }
        return found.finish("WezTerm");
    }
    let flat: BTreeMap<String, String> = flat.into_iter().filter_map(|(k, v)| v.into_iter().next().map(|v| (k, v))).collect();
    alacritty(&flat, "Alacritty TOML")
}

const ALACRITTY: [&str; 8] = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
fn alacritty(flat: &BTreeMap<String, String>, format: &str) -> Result<TerminalColors, String> {
    let get = |k: &str| flat.get(k).and_then(|v| color(v));
    let mut found = Found { background: get("colors.primary.background"), foreground: get("colors.primary.foreground"),
        cursor: get("colors.cursor.cursor"), selection: get("colors.selection.background"), ..Found::default() };
    for (i, name) in ALACRITTY.iter().enumerate() {
        found.ansi[i] = get(&format!("colors.normal.{name}"));
        found.ansi[i + 8] = get(&format!("colors.bright.{name}"));
    }
    found.finish(format)
}

// ---------------------------------------------------------------- YAML (Alacritty before 0.13)

/// Nested `key: value` maps by indentation, as dotted keys; list items are skipped.
fn yaml_flat(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    for raw in text.lines() {
        let line = strip_comment(raw);
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('-') || trimmed == "---" { continue; }
        let indent = line.len() - line.trim_start().len();
        let Some((key, value)) = trimmed.split_once(':') else { continue; };
        while stack.last().is_some_and(|(i, _)| *i >= indent) { stack.pop(); }
        let key = key.trim().trim_matches(|c| c == '"' || c == '\'').to_owned();
        let value = value.trim();
        if value.is_empty() { stack.push((indent, key)); continue; }
        let path: Vec<&str> = stack.iter().map(|(_, k)| k.as_str()).chain(std::iter::once(key.as_str())).collect();
        out.insert(path.join("."), value.trim_matches(|c| c == '"' || c == '\'').to_owned());
    }
    out
}

// ---------------------------------------------------------------- key-value files: Ghostty, Kitty, Xresources

fn ghostty(text: &str) -> Result<TerminalColors, String> {
    let mut found = Found::default();
    for line in text.lines().map(str::trim) {
        if line.starts_with('#') { continue; }
        let Some((key, value)) = line.split_once('=') else { continue; };
        let value = value.trim();
        match key.trim() {
            "palette" => if let Some((n, c)) = value.split_once('=') { if let Ok(n) = n.trim().parse::<usize>() { if n < 16 { found.ansi[n] = color(c); } } },
            "background" => found.background = color(value),
            "foreground" => found.foreground = color(value),
            "cursor-color" => found.cursor = color(value),
            "selection-background" => found.selection = color(value),
            _ => {}
        }
    }
    found.finish("Ghostty")
}

fn kitty(text: &str) -> Result<TerminalColors, String> {
    let mut found = Found::default();
    for line in text.lines().map(str::trim) {
        if line.starts_with('#') { continue; }
        let mut words = line.split_whitespace();
        let (Some(key), Some(value)) = (words.next(), words.next()) else { continue; };
        let c = color(value);
        match key {
            "background" => found.background = c,
            "foreground" => found.foreground = c,
            "cursor" => found.cursor = c,
            "selection_background" => found.selection = c,
            _ => if let Some(n) = key.strip_prefix("color").and_then(|n| n.parse::<usize>().ok()) { if n < 16 { found.ansi[n] = c; } },
        }
    }
    found.finish("Kitty")
}

fn xresources(text: &str) -> Result<TerminalColors, String> {
    let mut defines = BTreeMap::new();
    let mut found = Found::default();
    for line in text.lines().map(str::trim) {
        if line.starts_with('!') { continue; }
        if let Some(rest) = line.strip_prefix("#define") {
            let mut words = rest.split_whitespace();
            if let (Some(name), Some(value)) = (words.next(), words.next()) { defines.insert(name.to_owned(), value.to_owned()); }
            continue;
        }
        let Some((key, value)) = line.split_once(':') else { continue; };
        let name = key.trim().rsplit(['.', '*']).next().unwrap_or("").trim();
        let value = value.trim();
        let c = color(defines.get(value).map(String::as_str).unwrap_or(value));
        match name {
            "background" => found.background = c,
            "foreground" => found.foreground = c,
            "cursorColor" => found.cursor = c,
            _ => if let Some(n) = name.strip_prefix("color").and_then(|n| n.parse::<usize>().ok()) { if n < 16 { found.ansi[n] = c; } },
        }
    }
    found.finish("Xresources")
}
