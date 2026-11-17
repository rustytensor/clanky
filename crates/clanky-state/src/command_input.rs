//! The command input at the bottom of a terminal pane: the text being written, its undo, the
//! history it recalls and suggests from, Tab completion, and bash colours from tree-sitter.
//! Nothing here touches the window or the shell: the window sends what `submit_bytes` returns.
use std::path::Path;

/// The text and its caret: byte offsets on char boundaries, `anchor` != `cursor` is a selection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineEdit {
    text: String,
    cursor: usize,
    anchor: usize,
    undo: Vec<(String, usize, usize)>,
    redo: Vec<(String, usize, usize)>,
    /// The last change was typing: a run of typed characters undoes as one step.
    typing: bool,
}

impl LineEdit {
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn selection(&self) -> (usize, usize) {
        (self.anchor.min(self.cursor), self.anchor.max(self.cursor))
    }
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
    fn save(&mut self, typing: bool) {
        if !(typing && self.typing) {
            self.undo
                .push((self.text.clone(), self.cursor, self.anchor));
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.typing = typing;
    }
    /// Replace the selection with `text` (typing, a paste, a completion).
    pub fn insert(&mut self, text: &str) {
        let typing = text.chars().count() == 1 && self.anchor == self.cursor;
        self.save(typing);
        let (lo, hi) = self.selection();
        self.text.replace_range(lo..hi, text);
        self.cursor = lo + text.len();
        self.anchor = self.cursor;
    }
    /// Replace bytes `range` with `text`, the caret after it.
    pub fn replace(&mut self, range: std::ops::Range<usize>, text: &str) {
        self.save(false);
        self.text.replace_range(range.clone(), text);
        self.cursor = range.start + text.len();
        self.anchor = self.cursor;
    }
    /// The whole text, as a history recall sets it.
    pub fn set(&mut self, text: &str) {
        self.save(false);
        self.text = text.to_owned();
        self.cursor = self.text.len();
        self.anchor = self.cursor;
    }
    pub fn clear(&mut self) {
        if !self.text.is_empty() {
            self.set("");
        }
    }
    fn prev(&self, at: usize) -> usize {
        self.text[..at]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }
    fn next(&self, at: usize) -> usize {
        self.text[at..]
            .chars()
            .next()
            .map_or(at, |c| at + c.len_utf8())
    }
    pub fn backspace(&mut self) {
        let (lo, hi) = self.selection();
        let range = if lo != hi {
            lo..hi
        } else if lo > 0 {
            self.prev(lo)..lo
        } else {
            return;
        };
        self.replace(range, "");
    }
    pub fn delete(&mut self) {
        let (lo, hi) = self.selection();
        let range = if lo != hi {
            lo..hi
        } else if hi < self.text.len() {
            hi..self.next(hi)
        } else {
            return;
        };
        self.replace(range, "");
    }
    pub fn undo(&mut self) {
        if let Some((text, cursor, anchor)) = self.undo.pop() {
            self.redo.push((
                std::mem::replace(&mut self.text, text),
                self.cursor,
                self.anchor,
            ));
            self.cursor = cursor;
            self.anchor = anchor;
            self.typing = false;
        }
    }
    pub fn redo(&mut self) {
        if let Some((text, cursor, anchor)) = self.redo.pop() {
            self.undo.push((
                std::mem::replace(&mut self.text, text),
                self.cursor,
                self.anchor,
            ));
            self.cursor = cursor;
            self.anchor = anchor;
            self.typing = false;
        }
    }
    fn place(&mut self, at: usize, extend: bool) {
        self.cursor = at;
        if !extend {
            self.anchor = at;
        }
        self.typing = false;
    }
    pub fn left(&mut self, extend: bool) {
        let (lo, hi) = self.selection();
        if !extend && lo != hi {
            self.place(lo, false)
        } else {
            let at = self.prev(self.cursor);
            self.place(at, extend)
        }
    }
    pub fn right(&mut self, extend: bool) {
        let (lo, hi) = self.selection();
        if !extend && lo != hi {
            self.place(hi, false)
        } else {
            let at = self.next(self.cursor);
            self.place(at, extend)
        }
    }
    /// Start and end bytes of each line.
    pub fn lines(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut start = 0;
        for (i, b) in self.text.bytes().enumerate() {
            if b == b'\n' {
                out.push((start, i));
                start = i + 1;
            }
        }
        out.push((start, self.text.len()));
        out
    }
    /// The caret's line and column (in characters).
    pub fn caret(&self) -> (usize, usize) {
        self.line_col(self.cursor)
    }
    pub fn line_col(&self, at: usize) -> (usize, usize) {
        let lines = self.lines();
        let line = lines.iter().rposition(|(s, _)| *s <= at).unwrap_or(0);
        (line, self.text[lines[line].0..at].chars().count())
    }
    /// The byte at a line and column, clamped to the text.
    pub fn offset(&self, line: usize, col: usize) -> usize {
        let lines = self.lines();
        let (start, end) = lines[line.min(lines.len() - 1)];
        self.text[start..end]
            .char_indices()
            .nth(col)
            .map_or(end, |(i, _)| start + i)
    }
    pub fn home(&mut self, extend: bool) {
        let (line, _) = self.caret();
        let at = self.offset(line, 0);
        self.place(at, extend)
    }
    pub fn end(&mut self, extend: bool) {
        let (line, _) = self.caret();
        let at = self.offset(line, usize::MAX);
        self.place(at, extend)
    }
    /// Up or down a line; false at the first (or last) line.
    pub fn vertical(&mut self, down: bool, extend: bool) -> bool {
        let (line, col) = self.caret();
        let count = self.lines().len();
        if (!down && line == 0) || (down && line + 1 >= count) {
            return false;
        }
        let at = self.offset(if down { line + 1 } else { line - 1 }, col);
        self.place(at, extend);
        true
    }
    /// A click (or a drag with `extend`) at a line and column.
    pub fn click(&mut self, line: usize, col: usize, extend: bool) {
        let at = self.offset(line, col);
        self.place(at, extend)
    }
    pub fn at_end(&self) -> bool {
        self.cursor == self.text.len() && self.anchor == self.cursor
    }
}

/// Commands this window knows, newest last, each once: the ones sent from here, the ones the
/// panes' blocks ran, and the saved history (already masked by the server).
#[derive(Clone, Debug, Default)]
pub struct History {
    entries: Vec<String>,
}

impl History {
    pub fn entries(&self) -> &[String] {
        &self.entries
    }
    pub fn push(&mut self, command: &str) {
        let command = command.trim_end();
        if command.trim().is_empty() || command.len() > 64 * 1024 {
            return;
        }
        self.entries.retain(|e| e != command);
        self.entries.push(command.to_owned());
        if self.entries.len() > 5000 {
            self.entries.remove(0);
        }
    }
    /// Older commands, oldest first, that this window did not see itself.
    pub fn add_older(&mut self, older: Vec<String>) {
        let mut merged: Vec<String> = Vec::new();
        for command in older.into_iter().chain(std::mem::take(&mut self.entries)) {
            if command.trim().is_empty() {
                continue;
            }
            merged.retain(|e| *e != command);
            merged.push(command);
        }
        let skip = merged.len().saturating_sub(5000);
        self.entries = merged.split_off(skip);
    }
    /// The rest of the newest command that starts with `text` and is longer (grey ghost text).
    pub fn ghost(&self, text: &str) -> Option<&str> {
        if text.is_empty() || text.contains('\n') {
            return None;
        }
        self.entries
            .iter()
            .rev()
            .find(|e| e.len() > text.len() && e.starts_with(text) && !e.contains('\n'))
            .map(|e| &e[text.len()..])
    }
    /// The next older (or newer) entry starting with `prefix` from `from` (an index; None starts past the newest).
    pub fn step(&self, prefix: &str, from: Option<usize>, older: bool) -> Option<usize> {
        let matches = |i: &usize| self.entries[*i].starts_with(prefix);
        match (from, older) {
            (None, true) => (0..self.entries.len()).rev().find(matches),
            (Some(i), true) => (0..i).rev().find(matches),
            (Some(i), false) => (i + 1..self.entries.len()).find(matches),
            (None, false) => None,
        }
    }
}

/// The saved history's newest distinct commands typed by a person, oldest first. Rows whose
/// text the server masked (a run of stars) are left out: they would run as literal stars.
pub fn saved_commands(path: &Path, limit: usize) -> Vec<String> {
    clanky_core::history::recent_commands(path, limit)
        .unwrap_or_default()
        .into_iter()
        .filter(|c| !c.contains("****"))
        .collect()
}

/// What Enter sends: the text in a bracketed paste and a carriage return, so the shell takes
/// a multi-line text as one submission. Err names why the text cannot be sent.
pub fn submit_bytes(text: &str, bracketed: bool) -> Result<Vec<u8>, String> {
    if text.len() > 64 * 1024 {
        return Err("The command is longer than 64 KiB".into());
    }
    if text
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err("The command holds a control character".into());
    }
    let mut out = Vec::with_capacity(text.len() + 13);
    if bracketed {
        out.extend_from_slice(b"\x1b[200~");
        out.extend_from_slice(text.as_bytes());
        out.extend_from_slice(b"\x1b[201~");
    } else if text.contains('\n') {
        return Err(
            "The shell has bracketed paste off; a multi-line command would run line by line".into(),
        );
    } else {
        out.extend_from_slice(text.as_bytes());
    }
    out.push(b'\r');
    Ok(out)
}

// ------------------------------------------------------------------ completion

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    File,
    Folder,
    Branch,
    Command,
}
impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::File => "file",
            Kind::Folder => "folder",
            Kind::Branch => "branch",
            Kind::Command => "command",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// What the list shows (the word as it will read).
    pub shown: String,
    /// What replaces the word, escaped for the shell, with its trailing slash or space.
    pub insert: String,
    pub kind: Kind,
}
/// The word being completed: its bytes in the text, and where it sits in the command.
#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub place: Place,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Place {
    Command,
    Branch,
    Argument,
}

const SEPARATORS: &[char] = &['|', ';', '&', '(', ')', '<', '>', '`'];
/// The word that ends at `at`, read as bash splits words (quotes and backslashes kept together).
pub fn word_at(text: &str, at: usize) -> Word {
    let head = &text[..at];
    // Split the head into words of the current simple command.
    let mut words: Vec<(usize, String)> = Vec::new();
    let mut current: Option<(usize, String)> = None;
    let mut quote: Option<char> = None;
    let mut chars = head.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => {
                quote = None;
            }
            (Some(_), c) => {
                current.get_or_insert((i, String::new())).1.push(c);
            }
            (None, '\\') => {
                let start = current.get_or_insert((i, String::new()));
                if let Some((_, next)) = chars.next() {
                    start.1.push(next);
                }
            }
            (None, '\'' | '"') => {
                quote = Some(c);
                current.get_or_insert((i, String::new()));
            }
            (None, c) if c.is_whitespace() => {
                if let Some(w) = current.take() {
                    words.push(w);
                }
            }
            (None, c) if SEPARATORS.contains(&c) => {
                if let Some(w) = current.take() {
                    words.push(w);
                }
                words.clear();
            }
            (None, c) => {
                current.get_or_insert((i, String::new())).1.push(c);
            }
        }
    }
    let (start, word) = current.unwrap_or((at, String::new()));
    let before: Vec<&str> = words
        .iter()
        .map(|(_, w)| w.as_str())
        .skip_while(|w| w.contains('=') && !w.starts_with('=') && !w.starts_with('-'))
        .collect();
    let place = if before.is_empty() {
        Place::Command
    } else if before.first() == Some(&"git")
        && before
            .iter()
            .skip(1)
            .find(|w| !w.starts_with('-'))
            .is_some_and(|sub| *sub == "checkout" || *sub == "switch")
        && before.len() <= 3
        && !before
            .iter()
            .any(|w| matches!(*w, "-b" | "-B" | "-c" | "-C" | "--"))
    {
        Place::Branch
    } else {
        Place::Argument
    };
    Word {
        start,
        end: at,
        text: word,
        place,
    }
}
/// Backslash-escape what bash would split or expand.
pub fn escape(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_whitespace() || "\\'\"`$&|;()<>*?[]{}!#~=,^".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
fn expand_home(dir: &str, home: Option<&Path>) -> Option<std::path::PathBuf> {
    match (dir.strip_prefix('~'), home) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
            Some(home.join(rest.trim_start_matches('/')))
        }
        _ => None,
    }
}
/// Files and folders for `word` relative to `cwd`, in name order as readline lists them.
pub fn paths(word: &str, cwd: &Path, home: Option<&Path>) -> Vec<Candidate> {
    let (dir, prefix) = match word.rfind('/') {
        Some(i) => (&word[..=i], &word[i + 1..]),
        None => ("", word),
    };
    let base = if dir.is_empty() {
        cwd.to_owned()
    } else if let Some(path) = expand_home(dir, home) {
        path
    } else if dir.starts_with('/') {
        std::path::PathBuf::from(dir)
    } else {
        cwd.join(dir)
    };
    let Ok(entries) = std::fs::read_dir(&base) else {
        return Vec::new();
    };
    let mut out: Vec<Candidate> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !name.starts_with(prefix) || (name.starts_with('.') && !prefix.starts_with('.')) {
                return None;
            }
            let folder = entry.path().is_dir();
            let shown = format!("{dir}{name}{}", if folder { "/" } else { "" });
            let insert = format!(
                "{}{}{}",
                escape_dir(dir),
                escape(&name),
                if folder { "/" } else { " " }
            );
            Some(Candidate {
                shown,
                insert,
                kind: if folder { Kind::Folder } else { Kind::File },
            })
        })
        .take(2000)
        .collect();
    out.sort_by(|a, b| a.shown.cmp(&b.shown));
    out.truncate(200);
    out
}
fn escape_dir(dir: &str) -> String {
    match dir.strip_prefix('~') {
        Some(rest) => format!("~{}", escape(rest)),
        None => escape(dir),
    }
}
/// Programs on `path` (the window's PATH) and bash builtins starting with `word`.
pub fn commands(word: &str, path: Option<&std::ffi::OsStr>) -> Vec<Candidate> {
    use std::os::unix::fs::PermissionsExt;
    const BUILTINS: &[&str] = &[
        "alias",
        "bg",
        "bind",
        "break",
        "builtin",
        "caller",
        "cd",
        "command",
        "compgen",
        "complete",
        "compopt",
        "continue",
        "declare",
        "dirs",
        "disown",
        "echo",
        "enable",
        "eval",
        "exec",
        "exit",
        "export",
        "false",
        "fc",
        "fg",
        "getopts",
        "hash",
        "help",
        "history",
        "jobs",
        "kill",
        "let",
        "local",
        "logout",
        "mapfile",
        "popd",
        "printf",
        "pushd",
        "pwd",
        "read",
        "readarray",
        "readonly",
        "return",
        "set",
        "shift",
        "shopt",
        "source",
        "suspend",
        "test",
        "times",
        "trap",
        "true",
        "type",
        "typeset",
        "ulimit",
        "umask",
        "unalias",
        "unset",
        "wait",
    ];
    let mut names: std::collections::BTreeSet<String> = BUILTINS
        .iter()
        .filter(|b| b.starts_with(word))
        .map(|b| (*b).to_owned())
        .collect();
    for dir in path.map(std::env::split_paths).into_iter().flatten() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if !name.starts_with(word) || names.contains(&name) {
                continue;
            }
            if entry
                .metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            {
                names.insert(name);
            }
        }
    }
    names
        .into_iter()
        .take(200)
        .map(|name| Candidate {
            insert: format!("{} ", escape(&name)),
            shown: name,
            kind: Kind::Command,
        })
        .collect()
}
/// Local and remote branches of the repository at `cwd` starting with `word` (git for-each-ref).
pub fn branches(word: &str, cwd: &Path) -> Vec<Candidate> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args([
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes",
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    let Ok(out) = out else {
        return Vec::new();
    };
    let mut names: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|n| n.starts_with(word) && !n.ends_with("/HEAD") && !n.is_empty())
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .take(200)
        .map(|name| Candidate {
            insert: format!("{} ", escape(&name)),
            shown: name,
            kind: Kind::Branch,
        })
        .collect()
}
/// Everything Tab can offer for the word at the caret.
pub fn complete(
    text: &str,
    at: usize,
    cwd: &Path,
    home: Option<&Path>,
    path: Option<&std::ffi::OsStr>,
) -> (Word, Vec<Candidate>) {
    let word = word_at(text, at);
    let list = match word.place {
        Place::Branch => branches(&word.text, cwd),
        Place::Command if !word.text.contains('/') && !word.text.is_empty() => {
            commands(&word.text, path)
        }
        _ => paths(&word.text, cwd, home),
    };
    (word, list)
}

// ------------------------------------------------------------------ colours

/// What a character of the text is, for its colour (tree-sitter-bash's own highlight captures).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok {
    Plain,
    Function,
    Str,
    Property,
    Constant,
    Keyword,
    Comment,
    Operator,
    Number,
}

/// bash colours from tree-sitter-bash's parse of `text` and its upstream highlights query.
/// Where captures overlap, the innermost node wins, then the earlier pattern.
pub struct Highlighter {
    parser: tree_sitter::Parser,
    query: tree_sitter::Query,
    toks: Vec<Option<Tok>>,
}
impl Highlighter {
    pub fn new() -> Option<Self> {
        let language: tree_sitter::Language = tree_sitter_bash::LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).ok()?;
        let query = tree_sitter::Query::new(&language, tree_sitter_bash::HIGHLIGHT_QUERY).ok()?;
        let toks = query
            .capture_names()
            .iter()
            .map(|name| {
                Some(match name.split('.').next().unwrap_or("") {
                    "function" => Tok::Function,
                    "string" => Tok::Str,
                    "property" => Tok::Property,
                    "constant" => Tok::Constant,
                    "keyword" => Tok::Keyword,
                    "comment" => Tok::Comment,
                    "operator" => Tok::Operator,
                    "number" => Tok::Number,
                    _ => return None,
                })
            })
            .collect();
        Some(Self {
            parser,
            query,
            toks,
        })
    }
    /// One token per byte of `text`.
    pub fn tokens(&mut self, text: &str) -> Vec<Tok> {
        use tree_sitter::StreamingIterator;
        let mut out = vec![Tok::Plain; text.len()];
        let Some(tree) = self.parser.parse(text, None) else {
            return out;
        };
        let mut best: Vec<Option<(usize, usize)>> = vec![None; text.len()];
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&self.query, tree.root_node(), text.as_bytes());
        while let Some(m) = matches.next() {
            for c in m.captures() {
                let Some(tok) = self.toks.get(c.index as usize).copied().flatten() else {
                    continue;
                };
                let range = c.node.byte_range();
                let rank = (range.len(), m.pattern_index);
                for i in range.start..range.end.min(text.len()) {
                    if best[i].is_none_or(|b| rank < b) {
                        best[i] = Some(rank);
                        out[i] = tok;
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_sitter_bash_colours_the_gate_line() {
        // tree-sitter-highlight 0.27.2 on this line (the Q2 gate's TS_CAPS), one entry per character.
        let line = r#"NAME=val grep -n "two words" $HOME | wc -l # note"#;
        use Tok::*;
        let want: Vec<Tok> = [
            vec![Property; 4],
            vec![Plain; 5],
            vec![Function; 4],
            vec![Plain],
            vec![Constant; 2],
            vec![Plain],
            vec![Str; 11],
            vec![Plain, Operator],
            vec![Property; 4],
            vec![Plain, Operator, Plain],
            vec![Function; 2],
            vec![Plain],
            vec![Constant; 2],
            vec![Plain],
            vec![Comment; 6],
        ]
        .concat();
        assert_eq!(Highlighter::new().unwrap().tokens(line), want);
    }
}
