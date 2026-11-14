//! Breathing blocks (mockup section 1, .blk and .meta): space between OSC 133 command blocks.
//!
//! Each block gets its own meta row (folder and time on the left, exit and duration on the
//! right) above its command, 9 px above that row and 11 px under the block before it, with
//! the 1 px hairline in that space. The space is drawing only: the grid, the screen the
//! program sees and the rows the owner API reports do not change. `tops` holds the pixel top
//! of every grid row relative to the pane's grid origin; drawing and the mouse both use it.
use crate::look::BlockCard;
use clanky_core::TerminalSnapshot;

/// The meta row's height at scale 1: 12 px text in a 21 px line (the cell's line height ratio).
pub const META_H: f32 = 21.0;
/// Under the last output row, before the hairline (.blk padding-bottom).
pub const BELOW: f32 = 11.0;
/// Over the meta row (.blk padding-top).
pub const ABOVE: f32 = 9.0;
/// Between the meta row and the command (.meta margin-bottom).
pub const META_GAP: f32 = 2.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Breath {
    /// Pixel top of each grid row, relative to the grid origin.
    pub tops: Vec<f32>,
    /// How far the rows were lifted so the newest row stays in view (before any wheel lift).
    pub shift: f32,
}

/// The space a block gets above its command row: the first block on the screen has no
/// block above it, so no hairline and no space under one.
pub fn gap_above(first: bool, scale: f32) -> f32 {
    let meta = ABOVE + META_H + META_GAP;
    if first { meta * scale } else { (BELOW + 1.0 + meta) * scale }
}

/// The rows of a pane drawn with space between its blocks, or None for the plain grid
/// (no block on screen, or the alternate screen). `clip_h` is the grid's visible height;
/// rows are lifted so the cursor and the last written row stay in it, less `lift` px the
/// wheel took back to show the rows above.
pub fn breathe(blocks: &[BlockCard], snapshot: &TerminalSnapshot, ch: f32, scale: f32, clip_h: f32, lift: f32) -> Option<Breath> {
    let rows = snapshot.rows;
    if rows == 0 || snapshot.cols == 0 || snapshot.modes & clanky_core::MODE_ALT_SCREEN != 0 { return None; }
    let mut extra = vec![0.0f32; rows];
    let mut any = false;
    for block in blocks.iter().filter(|b| b.first_row < rows) {
        extra[block.first_row] += gap_above(block.first_row == 0, scale);
        any = true;
    }
    if !any { return None; }
    let mut tops = Vec::with_capacity(rows);
    let mut acc = 0.0;
    for (row, add) in extra.iter().enumerate() { acc += add; tops.push(row as f32 * ch + acc); }
    let written = (0..rows).rev().find(|row| snapshot.cells[row * snapshot.cols..(row + 1) * snapshot.cols].iter().any(|c| c.ch != ' ')).unwrap_or(0);
    let anchor = snapshot.cursor.map_or(written, |(row, _)| row.min(rows - 1).max(written));
    let shift = (tops[anchor] + ch - clip_h).max(0.0).floor();
    let lifted = (shift - lift.max(0.0)).max(0.0);
    for top in &mut tops { *top -= lifted; }
    Some(Breath { tops, shift })
}

impl Breath {
    /// The grid row under `y` px below the grid origin. A point in the space above a block
    /// belongs to that block's command row.
    pub fn row_at(&self, y: f32, ch: f32) -> usize {
        let n = self.tops.partition_point(|top| *top <= y);
        if n == 0 { return 0; }
        let row = n - 1;
        if y < self.tops[row] + ch { row } else { (row + 1).min(self.tops.len() - 1) }
    }
    /// The hairline's y (relative to the grid origin) above a block, 11 px under the row before it.
    pub fn hairline(&self, first_row: usize, ch: f32, scale: f32) -> Option<f32> {
        (first_row > 0 && first_row < self.tops.len()).then(|| self.tops[first_row - 1] + ch + BELOW * scale)
    }
    /// The meta row's top (relative to the grid origin) above a block's command row.
    pub fn meta_top(&self, first_row: usize, scale: f32) -> f32 {
        self.tops[first_row] - (META_GAP + META_H) * scale
    }
    /// Consecutive rows that share one offset: (first row, rows, top of the first).
    pub fn runs(&self, ch: f32) -> Vec<(usize, usize, f32)> {
        let mut out: Vec<(usize, usize, f32)> = Vec::new();
        for (row, top) in self.tops.iter().enumerate() {
            match out.last_mut() {
                Some((_, n, start)) if (*start + *n as f32 * ch - top).abs() < 0.01 => *n += 1,
                _ => out.push((row, 1, *top)),
            }
        }
        out
    }
}
