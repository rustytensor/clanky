//! config.xml: one root `<clanky version="1">`, one element per setting (docs/config.md).
//!
//! Clanky keeps a setting document as an ordered JSON object in memory (the shape `Config::from_json`
//! validates); this module reads config.xml into that document, with the line of every setting, and
//! writes the document back as XML. It also moves an old config.json to config.xml.
use serde_json::{Map, Number, Value};
use std::{fs, io::Write, path::Path};

/// Settings whose element text is true or false.
const BOOL: [&str; 9] = ["post_ai", "startup_animation", "compose_send_enter", "osc52_write", "osc52_read",
    "restore_layout", "animations", "rails", "history"];
/// Settings written as whole numbers.
const INT: [&str; 8] = ["columns", "rows", "notify_after_seconds", "history_days", "history_output_kib",
    "cursor_blink_time_ms", "cursor_blink_timeout_s", "new_window_panes"];
/// Settings written as decimal numbers (stored as f32, written as the shortest decimal that reads back).
const NUM: [&str; 5] = ["font_size", "font_size_pt", "opacity", "cell_width_scale", "cell_height_scale"];

pub fn is_bool(key: &str) -> bool { BOOL.contains(&key) }
pub fn is_int(key: &str) -> bool { INT.contains(&key) }
pub fn is_number(key: &str) -> bool { NUM.contains(&key) }

/// One setting as read from the file, with its 1-based line.
#[derive(Clone, Debug)]
pub struct Entry { pub key: String, pub value: Value, pub line: u32 }

/// The settings of a config.xml text, in file order. Errors read "line N: ...".
pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let doc = roxmltree::Document::parse(text).map_err(|e| format!("line {}: {}", e.pos().row, plain(&e.to_string())))?;
    let line = |node: roxmltree::Node| doc.text_pos_at(node.range().start).row;
    let root = doc.root_element();
    if root.tag_name().name() != "clanky" {
        return Err(format!("line {}: the root element must be <clanky>, not <{}>", line(root), root.tag_name().name()));
    }
    if let Some(version) = root.attribute("version") {
        if version.trim() != "1" { return Err(format!("line {}: version {version} is newer than this Clanky reads (1)", line(root))); }
    }
    let mut out: Vec<Entry> = Vec::new();
    for node in root.children() {
        if node.is_text() {
            if node.text().is_some_and(|t| !t.trim().is_empty()) { return Err(format!("line {}: text outside a setting", line(node))); }
            continue;
        }
        if !node.is_element() { continue; }
        let key = node.tag_name().name().to_owned();
        if out.iter().any(|e| e.key == key) { return Err(format!("line {}: {key} is set twice", line(node))); }
        let value = element_value(&key, node).map_err(|e| format!("line {}: {e}", line(node)))?;
        out.push(Entry { key, value, line: line(node) });
    }
    Ok(out)
}

/// roxmltree ends its messages with " at ROW:COL"; the line is given in front instead.
fn plain(message: &str) -> String {
    match message.rsplit_once(" at ") {
        Some((head, tail)) if tail.split(':').all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())) => head.to_owned(),
        _ => message.to_owned(),
    }
}

fn text_of(node: roxmltree::Node) -> Result<String, String> {
    let mut text = String::new();
    for child in node.children() {
        if child.is_element() { return Err(format!("<{}> holds text, not <{}>", node.tag_name().name(), child.tag_name().name())); }
        if let Some(t) = child.text() { if child.is_text() { text.push_str(t); } }
    }
    Ok(text.trim().to_owned())
}

fn elements<'a, 'i>(node: roxmltree::Node<'a, 'i>) -> Result<Vec<roxmltree::Node<'a, 'i>>, String> {
    let mut out = Vec::new();
    for child in node.children() {
        if child.is_element() { out.push(child); }
        else if child.is_text() && child.text().is_some_and(|t| !t.trim().is_empty()) {
            return Err(format!("<{}> holds elements, not text", node.tag_name().name()));
        }
    }
    Ok(out)
}

fn element_value(key: &str, node: roxmltree::Node) -> Result<Value, String> {
    let named = |n: roxmltree::Node, want: &str| -> Result<String, String> {
        if n.tag_name().name() != want { return Err(format!("<{key}> holds <{want}> elements, not <{}>", n.tag_name().name())); }
        text_of(n)
    };
    Ok(match key {
        "tools" => Value::Array(elements(node)?.into_iter().map(|n| named(n, "tool").map(Value::String)).collect::<Result<_, _>>()?),
        "colors" => Value::Object(elements(node)?.into_iter().map(|n| Ok((n.tag_name().name().to_owned(), Value::String(text_of(n)?)))).collect::<Result<_, String>>()?),
        "terminal_colors" => {
            let mut object = Map::new();
            for n in elements(node)? {
                let name = n.tag_name().name();
                let value = if name == "ansi" {
                    Value::Array(elements(n)?.into_iter().map(|c| named(c, "color").map(Value::String)).collect::<Result<_, _>>()?)
                } else { Value::String(text_of(n)?) };
                object.insert(name.to_owned(), value);
            }
            Value::Object(object)
        }
        "agents" => Value::Array(elements(node)?.into_iter().map(|agent| {
            if agent.tag_name().name() != "agent" { return Err(format!("<agents> holds <agent> elements, not <{}>", agent.tag_name().name())); }
            let mut object = Map::new();
            for field in elements(agent)? { object.insert(field.tag_name().name().to_owned(), Value::String(text_of(field)?)); }
            Ok(Value::Object(object))
        }).collect::<Result<_, String>>()?),
        _ => scalar(key, &text_of(node)?),
    })
}

/// A scalar's JSON value: booleans and numbers by the setting's type; anything else stays text,
/// so Config's own checks name the setting and the kind of value it needs.
pub fn scalar(key: &str, text: &str) -> Value {
    if is_bool(key) {
        return match text { "true" => Value::Bool(true), "false" => Value::Bool(false), _ => Value::String(text.into()) };
    }
    if is_int(key) || is_number(key) {
        if let Ok(n) = text.parse::<u64>() { return Value::Number(n.into()); }
        if let Ok(n) = text.parse::<i64>() { return Value::Number(n.into()); }
        if let Some(n) = text.parse::<f64>().ok().and_then(Number::from_f64) { return Value::Number(n); }
    }
    Value::String(text.into())
}

pub fn document(entries: &[Entry]) -> Value {
    Value::Object(entries.iter().map(|e| (e.key.clone(), e.value.clone())).collect())
}

/// Validates a config.xml text: the window's Config, and the document it came from. A bad value's
/// error names its line ("line 3: opacity must be ...").
pub fn load(text: &str) -> Result<(crate::config::Config, Value), String> {
    load_entries(&parse(text)?)
}

/// Settings whose bad value falls back to its default on its own, the other settings kept.
const SOFT: [&str; 1] = ["new_window_panes"];

/// As `load`, except that a bad value of a SOFT setting (and only that) is left out: the Config and
/// document hold the other settings, and the third item is that value's error ("line 3: ...").
pub fn load_soft(text: &str) -> Result<(crate::config::Config, Value, Option<String>), String> {
    let entries = parse(text)?;
    let error = match load_entries(&entries) { Ok((config, doc)) => return Ok((config, doc, None)), Err(error) => error };
    let bad = entries.iter().find(|entry| SOFT.contains(&entry.key.as_str()) && {
        let single = Value::Object(std::iter::once((entry.key.clone(), entry.value.clone())).collect());
        crate::config::Config::from_json(&single.to_string()).is_err()
    });
    let Some(bad) = bad else { return Err(error) };
    let rest: Vec<Entry> = entries.iter().filter(|entry| entry.key != bad.key).cloned().collect();
    let soft = load_entries(&rest).map_err(|_| error.clone())?;
    let single = Value::Object(std::iter::once((bad.key.clone(), bad.value.clone())).collect());
    let own = crate::config::Config::from_json(&single.to_string()).err().unwrap_or_default();
    Ok((soft.0, soft.1, Some(format!("line {}: {own}", bad.line))))
}

fn load_entries(entries: &[Entry]) -> Result<(crate::config::Config, Value), String> {
    let doc = document(entries);
    match crate::config::Config::window_from_json(&doc.to_string()) {
        Ok(config) => Ok((config, doc)),
        Err(error) => {
            for entry in entries {
                let single = Value::Object(std::iter::once((entry.key.clone(), entry.value.clone())).collect());
                if let Err(own) = crate::config::Config::from_json(&single.to_string()) {
                    return Err(format!("line {}: {own}", entry.line));
                }
            }
            Err(error)
        }
    }
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c { '&' => out.push_str("&amp;"), '<' => out.push_str("&lt;"), '>' => out.push_str("&gt;"), _ => out.push(c) }
    }
    out
}

/// The text of a scalar: true | false, whole numbers in decimal, decimal settings as the shortest
/// decimal that reads back as the same f32 (0.85, 1.25, 18), strings escaped.
pub fn scalar_text(key: &str, value: &Value) -> String {
    match value {
        Value::Bool(b) => b.to_string(),
        Value::Number(n) if is_number(key) => match n.as_f64() { Some(f) if f.is_finite() => format!("{}", f as f32), _ => n.to_string() },
        Value::Number(n) => n.to_string(),
        Value::String(s) => escape(s),
        other => escape(&other.to_string()),
    }
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        && !name.to_ascii_lowercase().starts_with("xml")
}

/// The document as config.xml, in its own key order.
pub fn write(doc: &Value) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- Clanky settings. Settings (Ctrl+,) changes and saves them; docs/config.md describes each one. -->\n<clanky version=\"1\">\n");
    let empty = Map::new();
    for (key, value) in doc.as_object().unwrap_or(&empty) {
        if !valid_name(key) { continue; }
        match (key.as_str(), value) {
            ("tools", Value::Array(list)) => {
                out.push_str("  <tools>");
                for tool in list { out.push_str(&format!("<tool>{}</tool>", scalar_text("tool", tool))); }
                out.push_str("</tools>\n");
            }
            ("colors", Value::Object(colors)) => {
                if colors.is_empty() { continue; }
                out.push_str("  <colors>\n");
                for (token, color) in colors.iter().filter(|(t, _)| valid_name(t)) { out.push_str(&format!("    <{token}>{}</{token}>\n", scalar_text(token, color))); }
                out.push_str("  </colors>\n");
            }
            ("terminal_colors", Value::Object(colors)) => {
                out.push_str("  <terminal_colors>\n");
                for (name, color) in colors.iter().filter(|(n, _)| valid_name(n)) {
                    match color {
                        Value::Array(list) => {
                            out.push_str(&format!("    <{name}>\n"));
                            for c in list { out.push_str(&format!("      <color>{}</color>\n", scalar_text("color", c))); }
                            out.push_str(&format!("    </{name}>\n"));
                        }
                        c => out.push_str(&format!("    <{name}>{}</{name}>\n", scalar_text(name, c))),
                    }
                }
                out.push_str("  </terminal_colors>\n");
            }
            ("agents", Value::Array(list)) => {
                out.push_str("  <agents>\n");
                for agent in list {
                    out.push_str("    <agent>");
                    for (field, text) in agent.as_object().unwrap_or(&empty).iter().filter(|(f, _)| valid_name(f)) {
                        out.push_str(&format!("<{field}>{}</{field}>", scalar_text(field, text)));
                    }
                    out.push_str("</agent>\n");
                }
                out.push_str("  </agents>\n");
            }
            (_, Value::Array(_) | Value::Object(_) | Value::Null) => {}
            (_, scalar) => out.push_str(&format!("  <{key}>{}</{key}>\n", scalar_text(key, scalar))),
        }
    }
    out.push_str("</clanky>\n");
    out
}

/// The old config.json's document in its modern form: font_size_pt P becomes font_size P x 4/3 and
/// rails becomes tools_panel "on" | "off", unless the modern key is there too (it wins).
pub fn modernize(doc: &Value) -> Value {
    let Some(object) = doc.as_object() else { return doc.clone(); };
    let mut out = Map::new();
    for (key, value) in object {
        match key.as_str() {
            "font_size_pt" if value.is_number() => {
                if !object.contains_key("font_size") {
                    let px = value.as_f64().unwrap_or(12.0) * 4.0 / 3.0;
                    let px = (px * 1e6).round() / 1e6;
                    out.insert("font_size".into(), Number::from_f64(px).map(Value::Number).unwrap_or(Value::Null));
                }
            }
            "rails" if value.is_boolean() => {
                if !object.contains_key("tools_panel") {
                    out.insert("tools_panel".into(), Value::String(if value.as_bool() == Some(true) { "on" } else { "off" }.into()));
                }
            }
            _ => { out.insert(key.clone(), value.clone()); }
        }
    }
    Value::Object(out)
}

/// On a start with config.json and no config.xml beside it: writes config.xml with the same values
/// (modernized) and renames config.json to config.json.migrated. Returns whether it migrated.
pub fn migrate(xml_path: &Path) -> Result<bool, String> {
    let json_path = xml_path.with_file_name("config.json");
    if fs::symlink_metadata(xml_path).is_ok() || !json_path.is_file() { return Ok(false); }
    let bytes = fs::read(&json_path).map_err(|e| format!("{}: {e}", json_path.display()))?;
    let doc: Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("{}: invalid JSON, not migrated: {e}", json_path.display()))?;
    if !doc.is_object() { return Err(format!("{}: not a JSON object, not migrated", json_path.display())); }
    let text = write(&modernize(&doc));
    let parent = xml_path.parent().unwrap_or(Path::new("."));
    let temp = parent.join(format!(".clanky-migrate-{}.tmp", std::process::id()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    crate::platform::private_file(&mut options);
    let result = (|| {
        let mut file = options.open(&temp).map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        // A link never replaces a config.xml another process wrote meanwhile.
        match fs::hard_link(&temp, xml_path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    })();
    let _ = fs::remove_file(&temp);
    let migrated = result.map_err(|e| format!("{}: {e}", xml_path.display()))?;
    if migrated {
        fs::rename(&json_path, json_path.with_file_name("config.json.migrated")).map_err(|e| format!("{}: {e}", json_path.display()))?;
    }
    Ok(migrated)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lines_and_round_trip() {
        let bad = "<?xml version=\"1.0\"?>\n<clanky version=\"1\">\n  <tab_style>vertical</tab_style>\n  <font_size>16</font_sizee>\n</clanky>\n";
        assert!(parse(bad).unwrap_err().starts_with("line 4:"), "{}", parse(bad).unwrap_err());
        let value = "<?xml version=\"1.0\"?>\n<clanky version=\"1\">\n  <opacity>0.3</opacity>\n</clanky>\n";
        let error = load(value).unwrap_err();
        assert!(error.starts_with("line 3: opacity"), "{error}");
        let doc = serde_json::json!({"opacity":0.85,"font_size":18,"cell_height_scale":1.25,"post_ai":true,"columns":100,
            "font_family":"A & <B>","tools":["files","agents"],"colors":{"accent":"#3a7bd5"},
            "terminal_colors":{"background":"#000000","ansi":vec!["#010101";16]},"agents":[{"name":"Pi","command":"pi --x"}]});
        let text = write(&doc);
        assert!(text.contains("<opacity>0.85</opacity>") && text.contains("<font_size>18</font_size>") && text.contains("<cell_height_scale>1.25</cell_height_scale>"), "{text}");
        let (_, back) = load(&text).unwrap();
        assert_eq!(back["font_family"], "A & <B>");
        assert_eq!(back["agents"][0]["command"], "pi --x");
        assert_eq!(back["terminal_colors"]["ansi"].as_array().unwrap().len(), 16);
        assert_eq!(write(&back), text);
        let old = modernize(&serde_json::json!({"font_size_pt":12,"rails":false,"columns":90}));
        assert_eq!(old, serde_json::json!({"font_size":16.0,"tools_panel":"off","columns":90}));
    }
}
