//! Window-local URL hit testing and server-directed shell creation.
use clanky_api::Client;
use clanky_core::TerminalSnapshot;
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UrlHit {
    pub url: String,
    pub row: usize,
    pub start_col: usize,
    /// Exclusive end of the URL's visible cell span.
    pub end_col: usize,
}

pub fn valid_url(url: &str) -> bool {
    clanky_core::platform::valid_http_url(url)
}

pub fn url_at(snapshot: &TerminalSnapshot, row: usize, col: usize) -> Option<UrlHit> {
    if row >= snapshot.rows || col >= snapshot.cols { return None; }
    let offset = row.checked_mul(snapshot.cols)?;
    let cells = snapshot.cells.get(offset..offset.checked_add(snapshot.cols)?)?;
    let boundary = |c: char| c.is_whitespace() || c.is_control();
    if boundary(cells[col].ch) { return None; }
    let mut start = col;
    let mut end = col + 1;
    while start > 0 && !boundary(cells[start - 1].ch) { start -= 1; }
    while end < cells.len() && !boundary(cells[end].ch) { end += 1; }
    while start < end && matches!(cells[start].ch, '(' | '[' | '{' | '<' | '"' | '\'' | '`' | '“' | '‘') {
        start += 1;
    }
    // Remove prose punctuation, but keep balanced parentheses/brackets in the URL.
    loop {
        if start >= end { return None; }
        let last = cells[end - 1].ch;
        let excess_closer = match last {
            ')' => Some('('), ']' => Some('['), '}' => Some('{'), _ => None,
        }.is_some_and(|open| {
            cells[start..end].iter().filter(|c| c.ch == last).count()
                > cells[start..end].iter().filter(|c| c.ch == open).count()
        });
        if excess_closer || matches!(last, '.' | ',' | ';' | ':' | '!' | '?' | '>' | '"' | '\'' | '`' | '”' | '’') {
            end -= 1;
        } else { break; }
    }
    if col < start || col >= end { return None; }
    let url: String = cells[start..end].iter().map(|c| c.ch).collect();
    valid_url(&url).then_some(UrlHit { url, row, start_col: start, end_col: end })
}

pub fn url_click(
    hit: Option<&UrlHit>,
    control: bool,
    open: impl FnOnce(&str) -> Result<(), String>,
) -> Result<bool, String> {
    let Some(hit) = hit else { return Ok(false); };
    if !valid_url(&hit.url) { return Ok(false); }
    if control { open(&hit.url)?; }
    Ok(true)
}

pub fn new_terminal(
    client: &Client,
    focused: Option<&str>,
    rows: usize,
    cols: usize,
) -> Result<String, String> {
    // Resolve everything on the server before creating anything. Never consult GUI cwd/HOME.
    let defaults = client.exchange(3, "shell_defaults", &json!({})).map_err(|e| e.message)?;
    let reported = match focused {
        Some(tab) => client.exchange(3, "terminal_cwd", &json!({"tab_id":tab})).map_err(|e| e.message)?,
        None => json!({"cwd":null}),
    };
    let cwd = reported.get("cwd").ok_or("Missing terminal directory")?;
    let cwd = if cwd.is_null() {
        defaults.get("home").filter(|v| !v.is_null()).ok_or("Server home directory unavailable")?
    } else { cwd };
    // A nonnull reported directory is authoritative, including a directory run will reject.
    let cwd = cwd.as_str().ok_or("Invalid server directory")?;
    let argv = defaults.get("argv").and_then(Value::as_array).filter(|a| {
        !a.is_empty() && a.iter().all(|v| v.as_str().is_some())
    }).ok_or("Invalid server shell arguments")?;
    let tab = client.request("open_tab", &json!({"rows":rows,"cols":cols,"allow_agent":false}), true)
        .map_err(|e| e.message)?;
    let id = tab["tab_id"].as_str().ok_or("Missing tab ID")?.to_owned();
    if let Err(run) = client.request("run", &json!({"tab_id":id,"argv":argv,"cwd":cwd}), true) {
        if let Err(close) = client.request("close_tab", &json!({"tab_id":id}), true) {
            return Err(format!("{}; closing new tab failed: {}", run.message, close.message));
        }
        return Err(run.message);
    }
    Ok(id)
}
