//! Terminal state, command transcripts, and Linux platform lifecycle.
mod engine;
pub mod history;
mod kitty;
pub mod layout;
pub mod platform;
mod session;
mod terminal;
pub(crate) use engine::{Command, Tab};
pub use engine::{CoreError, Engine, set_request_connection};
pub const POST_AI_REASON: &str = "agents are off (Post-AI mode)";
use serde::{Deserialize, Serialize};
pub use session::{Workspace, WorkspaceGroup};
/// Original SGR colour selection, before inverse-video swaps foreground/background.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorSource { Default, Indexed(u8), Truecolor }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellColorSource {
    pub fg: ColorSource,
    pub bg: ColorSource,
    #[serde(default)]
    pub inverse: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cell {
    pub ch: char,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    /// Optional presentation provenance; legacy readers continue using fg/bg.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_source: Option<CellColorSource>,
    /// SGR attributes the renderer draws: STYLE_BOLD, STYLE_FAINT.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub style: u8,
}
/// SGR 1: drawn with the bold face (colour unchanged; bold is not bright).
pub const STYLE_BOLD: u8 = 1;
/// SGR 2: drawn with reduced contrast against the background.
pub const STYLE_FAINT: u8 = 2;
fn is_zero(value: &u8) -> bool { *value == 0 }
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TerminalSnapshot {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<Cell>,
    pub cursor: Option<(usize, usize)>,
    pub images: Vec<ImagePlacement>,
    /// DECCKM (ESC[?1h): cursor keys send ESC O x instead of ESC [ x.
    #[serde(default)]
    pub app_cursor: bool,
    /// Mouse, focus and paste modes the program turned on (MODE_* bits).
    #[serde(default, skip_serializing_if = "is_zero32")]
    pub modes: u32,
    /// Scrollback lines above the screen, and how many of them the view is scrolled back.
    /// Row r of the view is line `history - display_offset + r` counted from the oldest line.
    #[serde(default)]
    pub history: usize,
    #[serde(default)]
    pub display_offset: usize,
    /// View rows whose line goes on in the next row (soft wrap).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wrapped: Vec<usize>,
}
fn is_zero32(value: &u32) -> bool { *value == 0 }
/// ESC[?1000h: report button presses and releases.
pub const MODE_MOUSE_CLICK: u32 = 1;
/// ESC[?1002h: also report motion while a button is held.
pub const MODE_MOUSE_DRAG: u32 = 1 << 1;
/// ESC[?1003h: report all motion.
pub const MODE_MOUSE_MOTION: u32 = 1 << 2;
/// ESC[?1006h: SGR report encoding.
pub const MODE_MOUSE_SGR: u32 = 1 << 3;
/// ESC[?1005h: UTF-8 report encoding.
pub const MODE_MOUSE_UTF8: u32 = 1 << 4;
/// ESC[?1007h (on by default): the wheel sends cursor keys on the alternate screen.
pub const MODE_ALTERNATE_SCROLL: u32 = 1 << 5;
/// ESC[?1004h: report window focus in and out.
pub const MODE_FOCUS: u32 = 1 << 6;
/// ESC[?1049h and friends: the alternate screen is shown.
pub const MODE_ALT_SCREEN: u32 = 1 << 7;
/// ESC[?2004h: bracketed paste.
pub const MODE_BRACKETED_PASTE: u32 = 1 << 8;
/// Any of the three mouse reporting modes.
pub const MODE_MOUSE_REPORT: u32 = MODE_MOUSE_CLICK | MODE_MOUSE_DRAG | MODE_MOUSE_MOTION;
/// Stable FNV-1a hash of one row of cells, shared by the session server and its
/// windows so a window can name the rows it already holds.
pub fn row_hash(cells: &[Cell]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut eat = |byte: u8| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    };
    for cell in cells {
        for byte in (cell.ch as u32).to_le_bytes() { eat(byte); }
        for byte in cell.fg.iter().chain(&cell.bg) { eat(*byte); }
        match cell.color_source {
            None => eat(0),
            Some(source) => {
                eat(1);
                for side in [source.fg, source.bg] {
                    match side {
                        ColorSource::Default => eat(0),
                        ColorSource::Indexed(n) => { eat(1); eat(n); }
                        ColorSource::Truecolor => eat(2),
                    }
                }
                eat(u8::from(source.inverse));
            }
        }
        eat(cell.style);
    }
    hash
}
/// An immutable image and its terminal-local placement and viewport clip, in cells.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImagePlacement {
    pub image_id: u32,
    pub placement_id: u32,
    pub x: usize,
    pub y: usize,
    pub cols: usize,
    pub rows: usize,
    pub width: u32,
    pub height: u32,
    pub image_key: u64,
    #[serde(skip)]
    pub rgba: std::sync::Arc<Vec<u8>>,
    pub clip_x: usize,
    pub clip_y: usize,
    pub clip_cols: usize,
    pub clip_rows: usize,
}

/// Feed real terminal/parser state without a PTY, preserving chunk boundaries.
pub fn snapshot_from_chunks(
    rows: usize,
    cols: usize,
    chunks: &[&[u8]],
) -> Result<TerminalSnapshot, CoreError> {
    if rows == 0 || cols == 0 || rows > 1000 || cols > 1000 || rows * cols > 250000 {
        return Err(CoreError {
            code: "invalid_size".into(),
            message: "invalid terminal dimensions".into(),
        });
    }
    let mut terminal = terminal::Terminal::new(rows, cols);
    for chunk in chunks {
        terminal.feed("", chunk);
    }
    Ok(terminal.snapshot())
}
pub fn text_fixture() -> TerminalSnapshot {
    terminal::fixture()
}
