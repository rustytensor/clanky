//! `clanky history`: read and edit the saved history file directly; no server needed.
use clanky_core::{history::{ROW_KEYS, Store}, platform};
use serde_json::Value;
use std::{io::Write, path::PathBuf};

const USAGE: &str = "clanky history path
clanky history [list] [--limit N] [--json]
clanky history search TEXT [--limit N] [--json]
clanky history show ID
clanky history export [--format json|csv|markdown] [--output FILE]
clanky history delete ID...
clanky history clear [--before YYYY-MM-DD]";

enum Failure {
    /// Exit code 2.
    Usage(String),
    /// Exit code 1.
    Error(String),
}
use Failure::{Error, Usage};

/// Runs `clanky history ARGS` and returns the process exit code: 0 done, 1 error, 2 bad arguments.
pub fn main(args: &[String]) -> i32 {
    match run(args) {
        Ok(()) => 0,
        Err(Usage(message)) => {
            eprintln!("clanky history: {message}\n{USAGE}");
            2
        }
        Err(Error(message)) => {
            eprintln!("clanky history: {message}");
            1
        }
    }
}

fn run(args: &[String]) -> Result<(), Failure> {
    let (command, rest) = match args.first().map(String::as_str) {
        None => ("list", args),
        Some(first) if first.starts_with("--") => ("list", args),
        Some(first) => (first, &args[1..]),
    };
    match command {
        "path" => {
            no_more(rest)?;
            println!("{}", path()?.display());
            Ok(())
        }
        "list" | "search" => {
            let mut text = None;
            let (mut limit, mut json) = (50u64, false);
            let mut items = rest.iter();
            while let Some(arg) = items.next() {
                match arg.as_str() {
                    "--json" => json = true,
                    "--limit" => {
                        let value = items.next().ok_or_else(|| Usage("--limit needs a number".into()))?;
                        limit = value.parse().ok().filter(|n| *n > 0).ok_or_else(|| Usage("--limit must be a positive integer".into()))?;
                    }
                    _ if command == "search" && text.is_none() && !arg.starts_with("--") => text = Some(arg.clone()),
                    _ => return Err(Usage(format!("unknown argument: {arg}"))),
                }
            }
            let store = store()?;
            let rows = if command == "search" {
                let text = text.ok_or_else(|| Usage("search needs TEXT".into()))?;
                store.search(&text, Some(limit)).map_err(Error)?
            } else {
                store.list(Some(limit)).map_err(Error)?
            };
            if json { print(&json_text(&rows)) } else { print(&table(&rows)) }
        }
        "show" => {
            let [id] = rest else { return Err(Usage("show needs one ID".into())) };
            let id = parse_id(id)?;
            let row = store()?.show(id).map_err(Error)?.ok_or_else(|| Error(format!("no command with ID {id}")))?;
            print(&detail(&row))
        }
        "export" => {
            let (mut format, mut output) = ("json".to_owned(), None);
            let mut items = rest.iter();
            while let Some(arg) = items.next() {
                match arg.as_str() {
                    "--format" => {
                        format = items.next().filter(|f| matches!(f.as_str(), "json" | "csv" | "markdown"))
                            .ok_or_else(|| Usage("--format must be json, csv or markdown".into()))?.clone();
                    }
                    "--output" => output = Some(PathBuf::from(items.next().ok_or_else(|| Usage("--output needs a file".into()))?)),
                    _ => return Err(Usage(format!("unknown argument: {arg}"))),
                }
            }
            let rows = store()?.list(None).map_err(Error)?;
            let text = match format.as_str() {
                "csv" => csv(&rows),
                "markdown" => markdown(&rows),
                _ => json_text(&rows),
            };
            match output {
                Some(file) => write_private(&file, &text),
                None => print(&text),
            }
        }
        "delete" => {
            if rest.is_empty() { return Err(Usage("delete needs at least one ID".into())); }
            let ids = rest.iter().map(|id| parse_id(id)).collect::<Result<Vec<_>, _>>()?;
            let removed = store()?.delete(&ids).map_err(Error)?;
            eprintln!("Deleted {removed} of {} commands.", ids.len());
            Ok(())
        }
        "clear" => {
            let before = match rest {
                [] => None,
                [flag, date] if flag == "--before" => Some(parse_date(date)?),
                _ => return Err(Usage("clear takes only --before YYYY-MM-DD".into())),
            };
            let removed = store()?.clear(before).map_err(Error)?;
            eprintln!("Deleted {removed} commands.");
            Ok(())
        }
        "--help" | "-h" | "help" => { println!("{USAGE}"); Ok(()) }
        other => Err(Usage(format!("unknown command: {other}"))),
    }
}

fn no_more(rest: &[String]) -> Result<(), Failure> {
    match rest.first() { Some(arg) => Err(Usage(format!("unknown argument: {arg}"))), None => Ok(()) }
}

fn path() -> Result<PathBuf, Failure> {
    platform::history_path().ok_or_else(|| Error("no data directory: set XDG_DATA_HOME or HOME".into()))
}

fn store() -> Result<Store, Failure> {
    Store::open(&path()?).map_err(Error)
}

fn parse_id(text: &str) -> Result<i64, Failure> {
    text.parse().ok().filter(|id| *id > 0).ok_or_else(|| Usage(format!("not a command ID: {text}")))
}

/// Midnight UTC of YYYY-MM-DD, in Unix ms.
fn parse_date(text: &str) -> Result<i64, Failure> {
    let bad = || Usage(format!("not a date (YYYY-MM-DD): {text}"));
    let parts: Vec<_> = text.split('-').collect();
    let [y, m, d] = parts.as_slice() else { return Err(bad()) };
    if y.len() != 4 || m.len() != 2 || d.len() != 2 { return Err(bad()); }
    let (y, m, d): (i64, u32, u32) = (y.parse().map_err(|_| bad())?, m.parse().map_err(|_| bad())?, d.parse().map_err(|_| bad())?);
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days_in = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if !(1..=12).contains(&m) || d == 0 || d > days_in[m as usize - 1] { return Err(bad()); }
    Ok(days_from_civil(y, m, d) * 86_400_000)
}

/// Days since 1970-01-01 (proleptic Gregorian; H. Hinnant's algorithm).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { yoe + era * 400 + 1 } else { yoe + era * 400 }, m, d)
}

/// "2026-11-05 22:30:01 UTC"
fn utc(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let t = secs.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC", t / 3600, t / 60 % 60, t % 60)
}

fn print(text: &str) -> Result<(), Failure> {
    let mut out = std::io::stdout().lock();
    out.write_all(text.as_bytes()).and_then(|()| out.flush()).map_err(|e| Error(e.to_string()))
}

/// History text is private: an export file is owner-only.
fn write_private(path: &std::path::Path, text: &str) -> Result<(), Failure> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)
        .map_err(|e| Error(format!("cannot write {}: {e}", path.display())))?;
    file.write_all(text.as_bytes()).map_err(|e| Error(format!("cannot write {}: {e}", path.display())))
}

fn json_text(rows: &[Value]) -> String {
    let mut text = serde_json::to_string_pretty(rows).unwrap_or_else(|_| "[]".into());
    text.push('\n');
    text
}

fn first_line(command: &Value) -> String {
    let line = command.as_str().and_then(|c| c.lines().next()).unwrap_or("");
    let line: String = line.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    if line.chars().count() > 80 { line.chars().take(79).chain(['…']).collect() } else { line }
}

fn exit_label(row: &Value) -> String {
    if row["complete"] != true { return "interrupted".into(); }
    row["exit_code"].as_i64().map_or_else(|| "-".into(), |code| code.to_string())
}

fn table(rows: &[Value]) -> String {
    if rows.is_empty() { return "No saved commands.\n".into(); }
    let mut text = format!("{:>6}  {:<23}  {:>11}  {:<10}  {}\n", "ID", "STARTED", "EXIT", "BY", "COMMAND");
    for row in rows {
        text += &format!("{:>6}  {:<23}  {:>11}  {:<10}  {}\n", row["id"], utc(row["started_at"].as_i64().unwrap_or(0)),
            exit_label(row), row["actor"].as_str().unwrap_or(""), first_line(&row["command"]));
    }
    text
}

fn detail(row: &Value) -> String {
    let mut text = format!("Command {} · session {} · {}\n", row["id"], row["session_id"], utc(row["started_at"].as_i64().unwrap_or(0)));
    let duration = row["duration_ms"].as_u64().map_or_else(String::new, |ms| format!(" · {:.1} s", ms as f64 / 1000.0));
    text += &format!("By {} · exit {}{duration}\n", row["actor"].as_str().unwrap_or(""), exit_label(row));
    if let Some(cwd) = row["cwd"].as_str() { text += &format!("In {cwd}\n"); }
    text += &format!("$ {}\n", row["command"].as_str().unwrap_or("(no command line)"));
    let output = row["output"].as_str().unwrap_or("");
    if !output.is_empty() {
        text += output;
        text.push('\n');
    }
    if row["output_truncated"] == true { text += "(output truncated)\n"; }
    text
}

fn csv(rows: &[Value]) -> String {
    fn field(value: &Value) -> String {
        let text = match value {
            Value::Null => String::new(),
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        if text.contains([',', '"', '\n', '\r']) { format!("\"{}\"", text.replace('"', "\"\"")) } else { text }
    }
    let mut text = ROW_KEYS.join(",") + "\r\n";
    for row in rows {
        text += &ROW_KEYS.iter().map(|key| field(&row[*key])).collect::<Vec<_>>().join(",");
        text += "\r\n";
    }
    text
}

fn markdown(rows: &[Value]) -> String {
    fn fence(body: &str, info: &str) -> String {
        let mut longest = 0;
        let mut run = 0;
        for c in body.chars() {
            run = if c == '`' { run + 1 } else { 0 };
            longest = longest.max(run);
        }
        let marks = "`".repeat((longest + 1).max(3));
        format!("{marks}{info}\n{body}\n{marks}\n")
    }
    let mut text = String::from("# Clanky saved history\n");
    for row in rows {
        text += &format!("\n## {} · {} · exit {} · {}\n\n", row["id"], utc(row["started_at"].as_i64().unwrap_or(0)),
            exit_label(row), row["actor"].as_str().unwrap_or(""));
        if let Some(cwd) = row["cwd"].as_str() { text += &format!("In `{}`\n\n", cwd.replace('`', "'")); }
        text += &fence(row["command"].as_str().unwrap_or(""), "sh");
        let output = row["output"].as_str().unwrap_or("");
        if !output.is_empty() { text += "\n"; text += &fence(output, "text"); }
        if row["output_truncated"] == true { text += "\n(output truncated)\n"; }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dates_match_known_unix_days() {
        // 2000-01-01 is day 10957; 2026-11-05 is day 20762 (date -u -d 2026-11-05 +%s / 86400).
        assert!(matches!(parse_date("2000-01-01"), Ok(946_684_800_000)));
        assert_eq!(days_from_civil(2026, 11, 5), 20_762);
        assert_eq!(civil_from_days(20_762), (2026, 11, 5));
        assert_eq!(utc(1_762_381_801_000), "2025-11-05 22:30:01 UTC");
        assert!(matches!(parse_date("2026-02-29"), Err(Usage(_))));
    }
}
