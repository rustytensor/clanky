//! Terminal state, command transcripts, and Linux platform lifecycle.
mod engine;
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
}
pub fn text_fixture() -> TerminalSnapshot {
    terminal::fixture()
}
