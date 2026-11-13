//! The mouse in a terminal pane, the way VTE does it: reports to the program
//! (xterm's "Mouse Tracking"), wheel handling, word and line selection, and pasting.
//! Window code supplies cells and pointer positions; nothing here touches the GUI.
use clanky_core::{
    MODE_ALTERNATE_SCROLL, MODE_ALT_SCREEN, MODE_MOUSE_DRAG, MODE_MOUSE_MOTION, MODE_MOUSE_REPORT,
    MODE_MOUSE_SGR, MODE_MOUSE_UTF8, TerminalSnapshot,
};

/// Modifier keys held during a mouse event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

/// Whether the program asked for mouse reports (1000, 1002 or 1003).
pub fn reporting(modes: u32) -> bool {
    modes & MODE_MOUSE_REPORT != 0
}

/// Whether motion to a new cell is reported: always in 1003, in 1002 only with a button held.
pub fn reports_motion(modes: u32, button_held: bool) -> bool {
    modes & MODE_MOUSE_MOTION != 0 || (modes & MODE_MOUSE_DRAG != 0 && button_held)
}

/// One report, as VTE's feed_mouse_event builds it. `button` is 0 for motion with no
/// button, 1 left, 2 middle, 3 right, 4/5 wheel up/down, 6/7 wheel left/right.
/// `col` and `row` are 0-based on the pane's screen. None when nothing can be sent.
pub fn report(modes: u32, button: u8, mods: Mods, col: usize, row: usize, drag: bool, release: bool) -> Option<Vec<u8>> {
    if !reporting(modes) {
        return None;
    }
    let mut cb: u32 = match button {
        0 => 3,
        1..=3 => u32::from(button) - 1,
        4..=7 => u32::from(button) - 4 + 64,
        _ => return None,
    };
    let sgr = modes & MODE_MOUSE_SGR != 0;
    if release && !sgr {
        cb = 3;
    }
    cb |= if mods.shift { 4 } else { 0 } | if mods.alt { 8 } else { 0 } | if mods.ctrl { 16 } else { 0 };
    if drag {
        cb |= 32;
    }
    let (x, y) = (col as u32 + 1, row as u32 + 1);
    if sgr {
        return Some(format!("\x1b[<{cb};{x};{y}{}", if release { 'm' } else { 'M' }).into_bytes());
    }
    let mut out = b"\x1b[M".to_vec();
    if modes & MODE_MOUSE_UTF8 != 0 {
        // xterm 1005: each value plus 32 as one UTF-8 character, up to 2015.
        for value in [cb, x, y] {
            let ch = char::from_u32(32 + value).filter(|_| 32 + value < 2048)?;
            let mut buf = [0; 4];
            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
        }
    } else {
        if cb > 223 || x > 223 || y > 223 {
            return None;
        }
        out.extend([32 + cb as u8, 32 + x as u8, 32 + y as u8]);
    }
    Some(out)
}

/// What one wheel notch does when the program did not ask for mouse reports:
/// on the alternate screen with 1007, n = max(1, ceil(rows / 10)) cursor keys
/// (VTE widget_mouse_scroll); otherwise None and the view scrolls its history.
pub fn wheel_keys(modes: u32, app_cursor: bool, rows: usize, up: bool) -> Option<Vec<u8>> {
    if modes & MODE_ALT_SCREEN == 0 || modes & MODE_ALTERNATE_SCROLL == 0 {
        return None;
    }
    let n = rows.div_ceil(10).max(1);
    let key: &[u8] = match (app_cursor, up) {
        (true, true) => b"\x1bOA",
        (true, false) => b"\x1bOB",
        (false, true) => b"\x1b[A",
        (false, false) => b"\x1b[B",
    };
    Some(key.repeat(n))
}

/// VTE's pastify_string: C0 controls other than NUL, HT, CR and LF, DEL and C1
/// controls become control pictures (U+2400.., U+2421, U+FFFD); LF and CRLF become
/// CR; brackets when the program turned on bracketed paste.
pub fn pastify(text: &str, bracketed: bool) -> Vec<u8> {
    let mut out = String::with_capacity(text.len() + 12);
    if bracketed {
        out.push_str("\x1b[200~");
    }
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                out.push('\r');
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            '\n' => out.push('\r'),
            '\0' | '\t' => out.push(c),
            '\u{1}'..='\u{1f}' => out.push(char::from_u32(0x2400 + c as u32).unwrap()),
            '\u{7f}' => out.push('\u{2421}'),
            '\u{80}'..='\u{9f}' => out.push('\u{fffd}'),
            _ => out.push(c),
        }
    }
    if bracketed {
        out.push_str("\x1b[201~");
    }
    out.into_bytes()
}

/// VTE's default word characters: letters and numbers, plus these
/// (WORD_CHAR_EXCEPTIONS_DEFAULT; Ptyxis leaves the setting unset).
const WORD_CHAR_EXCEPTIONS: &str = "-#%&+,./=?@\\_~\u{b7}";

pub fn is_word_char(c: char) -> bool {
    if c.is_alphanumeric() {
        return true;
    }
    if c.is_whitespace() || c.is_control() {
        return false;
    }
    WORD_CHAR_EXCEPTIONS.contains(c)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Char,
    Word,
    Line,
}

/// A pointer position on a line counted from the oldest scrollback line.
/// `right` is true over the right half of the cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pos {
    pub line: i64,
    pub col: usize,
    pub right: bool,
}

/// The view's first row as a line number counted from the oldest scrollback line.
pub fn top_line(snapshot: &TerminalSnapshot) -> i64 {
    snapshot.history as i64 - snapshot.display_offset as i64
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub kind: Kind,
    /// Ctrl+drag selects a rectangle.
    pub block: bool,
    pub anchor: Pos,
    pub head: Pos,
}

/// Selected cells on one line: columns start..end.
pub type Span = (i64, usize, usize);

impl Selection {
    pub fn new(kind: Kind, block: bool, at: Pos) -> Self {
        Self { kind, block, anchor: at, head: at }
    }

    /// Cells selected, per line, in order. Lines outside the view can only be
    /// chosen whole or by column (words need their cells and come out by column).
    pub fn spans(&self, snapshot: &TerminalSnapshot) -> Vec<Span> {
        let cols = snapshot.cols;
        if cols == 0 {
            return Vec::new();
        }
        let (a, b) = if (self.anchor.line, self.anchor.col, self.anchor.right) <= (self.head.line, self.head.col, self.head.right) {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        };
        let (start, end) = match self.kind {
            Kind::Char => {
                let edge = |p: Pos| (p.line, (p.col + usize::from(p.right)).min(cols));
                let (s, e) = (edge(a), edge(b));
                if self.block {
                    let (l, r) = (s.1.min(e.1), s.1.max(e.1));
                    if l == r {
                        return Vec::new();
                    }
                    return (s.0..=e.0).map(|line| (line, l, r)).collect();
                }
                if s == e {
                    return Vec::new();
                }
                (s, e)
            }
            Kind::Word => ((a.line, word_start(snapshot, a)), (b.line, word_end(snapshot, b))),
            Kind::Line => ((logical_start(snapshot, a.line), 0), (logical_end(snapshot, b.line), cols)),
        };
        (start.0..=end.0)
            .map(|line| (line, if line == start.0 { start.1 } else { 0 }, if line == end.0 { end.1 } else { cols }))
            .filter(|(_, s, e)| s < e || self.kind == Kind::Line)
            .collect()
    }

    /// The selected text from the cells in view; rows end with a newline unless they
    /// wrap onto the next row, and trailing blanks are dropped. A line selection
    /// ends with its newline, as in VTE.
    pub fn text(&self, snapshot: &TerminalSnapshot) -> String {
        let spans = self.spans(snapshot);
        let mut out = String::new();
        for (i, (line, s, e)) in spans.iter().enumerate() {
            let row = line - top_line(snapshot);
            let cells = (row >= 0 && (row as usize) < snapshot.rows).then(|| &snapshot.cells[row as usize * snapshot.cols..(row as usize + 1) * snapshot.cols]);
            let text: String = cells.map(|cells| cells[*s..*e].iter().map(|c| c.ch).collect()).unwrap_or_default();
            let wraps = !self.block && *e == snapshot.cols && row >= 0 && snapshot.wrapped.contains(&(row as usize));
            if wraps {
                out.push_str(&text);
            } else {
                out.push_str(text.trim_end_matches(' '));
                if i + 1 < spans.len() || self.kind == Kind::Line {
                    out.push('\n');
                }
            }
        }
        out
    }
}

fn row_of(snapshot: &TerminalSnapshot, line: i64) -> Option<usize> {
    let row = line - top_line(snapshot);
    (row >= 0 && (row as usize) < snapshot.rows).then_some(row as usize)
}

fn cell(snapshot: &TerminalSnapshot, row: usize, col: usize) -> char {
    snapshot.cells.get(row * snapshot.cols + col).map_or(' ', |c| c.ch)
}

/// Same class as VTE's is_same_class: runs of word characters group; others stand alone.
fn word_start(snapshot: &TerminalSnapshot, p: Pos) -> usize {
    let Some(row) = row_of(snapshot, p.line) else { return p.col };
    let mut col = p.col.min(snapshot.cols - 1);
    if !is_word_char(cell(snapshot, row, col)) {
        return col;
    }
    while col > 0 && is_word_char(cell(snapshot, row, col - 1)) {
        col -= 1;
    }
    col
}

fn word_end(snapshot: &TerminalSnapshot, p: Pos) -> usize {
    let Some(row) = row_of(snapshot, p.line) else { return (p.col + 1).min(snapshot.cols) };
    let mut col = p.col.min(snapshot.cols - 1);
    if !is_word_char(cell(snapshot, row, col)) {
        return col + 1;
    }
    while col + 1 < snapshot.cols && is_word_char(cell(snapshot, row, col + 1)) {
        col += 1;
    }
    col + 1
}

/// The first and last rows of the soft-wrapped line holding `line`, within the view.
fn logical_start(snapshot: &TerminalSnapshot, mut line: i64) -> i64 {
    while let Some(row) = row_of(snapshot, line - 1) {
        if !snapshot.wrapped.contains(&row) {
            break;
        }
        line -= 1;
    }
    line
}

fn logical_end(snapshot: &TerminalSnapshot, mut line: i64) -> i64 {
    while let Some(row) = row_of(snapshot, line) {
        if !snapshot.wrapped.contains(&row) || row_of(snapshot, line + 1).is_none() {
            break;
        }
        line += 1;
    }
    line
}

/// Multi-click counting: presses within 400 ms (GTK's double-click time) on the
/// same cell count up 1, 2, 3, then start again at 1.
#[derive(Clone, Debug, Default)]
pub struct Clicks {
    last: Option<(std::time::Instant, i64, usize, u8)>,
}

impl Clicks {
    pub fn press(&mut self, now: std::time::Instant, line: i64, col: usize) -> u8 {
        let count = match self.last {
            Some((at, l, c, n)) if now.duration_since(at).as_millis() <= 400 && l == line && c == col => n % 3 + 1,
            _ => 1,
        };
        self.last = Some((now, line, col, count));
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clanky_core::{Cell, MODE_MOUSE_CLICK};

    fn view(lines: &[&str], cols: usize) -> TerminalSnapshot {
        let mut cells = Vec::new();
        for line in lines {
            let mut chars: Vec<char> = line.chars().collect();
            chars.resize(cols, ' ');
            cells.extend(chars.into_iter().map(|ch| Cell { ch, fg: [0; 3], bg: [0; 3], color_source: None, style: 0 }));
        }
        TerminalSnapshot { rows: lines.len(), cols, cells, ..Default::default() }
    }

    /// xterm ctlseqs, "Normal tracking mode": ESC [ M CbCxCy, each value plus 32,
    /// release as button 3; with 1006: ESC [ < Cb ; Cx ; Cy M or m.
    #[test]
    fn report_bytes() {
        let none = Mods::default();
        assert_eq!(report(MODE_MOUSE_CLICK, 1, none, 9, 4, false, false).unwrap(), b"\x1b[M\x20\x2a\x25");
        assert_eq!(report(MODE_MOUSE_CLICK, 1, none, 9, 4, false, true).unwrap(), b"\x1b[M\x23\x2a\x25");
        let ctrl = Mods { ctrl: true, ..none };
        assert_eq!(report(MODE_MOUSE_CLICK | MODE_MOUSE_SGR, 3, ctrl, 9, 4, false, true).unwrap(), b"\x1b[<18;10;5m");
        assert_eq!(report(MODE_MOUSE_MOTION | MODE_MOUSE_SGR, 0, none, 12, 6, true, false).unwrap(), b"\x1b[<35;13;7M");
    }

    /// Expected values from VTE's pastify-test.cc: controls become pictures, HT passes,
    /// a lone LF and CRLF become CR, brackets around the text.
    #[test]
    fn paste_like_vte() {
        assert_eq!(pastify("0", true), b"\x1b[200~0\x1b[201~");
        assert_eq!(String::from_utf8(pastify("abc\x1b\x1babc", false)).unwrap(), "abc\u{241b}\u{241b}abc");
        assert_eq!(pastify("a\tb\nc\r\nd", false), b"a\tb\rc\rd");
        assert_eq!(String::from_utf8(pastify("x\u{7f}\u{9b}", false)).unwrap(), "x\u{2421}\u{fffd}");
    }

    /// The gate's line: VTE's word characters keep the path together.
    #[test]
    fn word_and_line_selection() {
        let v = view(&["", "", "alpha beta gamma /srv/devops/x-y.txt"], 40);
        let at = |col| Pos { line: 2, col, right: false };
        assert_eq!(Selection::new(Kind::Word, false, at(7)).text(&v), "beta");
        assert_eq!(Selection::new(Kind::Word, false, at(23)).text(&v), "/srv/devops/x-y.txt");
        assert_eq!(Selection::new(Kind::Line, false, at(3)).text(&v), "alpha beta gamma /srv/devops/x-y.txt\n");
        let drag = Selection { kind: Kind::Char, block: false, anchor: Pos { line: 2, col: 0, right: false }, head: Pos { line: 2, col: 9, right: true } };
        assert_eq!(drag.text(&v), "alpha beta");
    }
}
