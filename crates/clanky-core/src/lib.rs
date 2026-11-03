//! Terminal state, command transcripts, and Linux platform lifecycle.
mod engine;
mod kitty;
pub mod layout;
pub mod platform;
mod terminal;
pub(crate) use engine::{Command, Tab};
pub use engine::{CoreError, Engine};
#[derive(Clone, Debug)]
pub struct Cell {
    pub ch: char,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
}
#[derive(Clone, Debug)]
pub struct TerminalSnapshot {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<Cell>,
    pub cursor: Option<(usize, usize)>,
    pub images: Vec<ImagePlacement>,
}
/// An immutable image and its terminal-local placement and viewport clip, in cells.
#[derive(Clone, Debug)]
pub struct ImagePlacement {
    pub image_id: u32,
    pub placement_id: u32,
    pub x: usize,
    pub y: usize,
    pub cols: usize,
    pub rows: usize,
    pub width: u32,
    pub height: u32,
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
