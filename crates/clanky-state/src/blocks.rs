//! Command blocks as the window shows them: the server's OSC 133 blocks placed on the screen's rows, and the
//! space between quiet blocks (mockup section 1, .blk and .meta).
//!
//! Each block gets its own meta row (folder and time on the left, exit and duration on the
//! right) above its command, 9 px above that row and 11 px under the block before it, with
//! the 1 px hairline in that space. The space is drawing only: the grid, the screen the
//! program sees and the rows the owner API reports do not change. `tops` holds the pixel top
//! of every grid row relative to the pane's grid origin; drawing and the mouse both use it.
use clanky_core::TerminalSnapshot;
use serde_json::Value;

/// One command block on the screen: its command row, its last output row, and how it ended.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockCard {
    pub id: String,
    pub first_row: usize,
    pub last_row: usize,
    pub command: String,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub running: bool,
}

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

/// Only server anchors from this snapshot's generation place cards; never text matching.
pub fn matched_blocks(snapshot: &TerminalSnapshot, view: &Value) -> Vec<BlockCard> {
    let mut cards = Vec::new();
    if !snapshot.images.is_empty() || snapshot.cols == 0 { return cards; }
    let Some(top) = view["viewport_line"].as_u64() else { return cards; };
    let Some(blocks) = view["blocks"].as_array() else { return cards; };
    for block in blocks {
        let (Some(id), Some(command), Some(line)) = (block["block_id"].as_str(), block["command"].as_str(), block["command_line"].as_u64()) else { continue; };
        let Some(prompt) = block["prompt_line"].as_u64() else { continue; };
        // Server lines (fish, measured): the command is typed on the line before output
        // starts (command_line), and the finish mark sits on the next prompt's line.
        let typed = if line > prompt { line - 1 } else { prompt };
        let Some(first) = typed.checked_sub(top).and_then(|n| usize::try_from(n).ok()).filter(|n| *n < snapshot.rows) else { continue; };
        let complete = block["complete"] == true;
        let end = match block["end_line"].as_u64() { Some(end) if complete => end.saturating_sub(1).max(typed), _ => top + snapshot.rows as u64 - 1 };
        let last = end.saturating_sub(top).min(snapshot.rows.saturating_sub(1) as u64) as usize;
        if last < first || cards.iter().any(|c: &BlockCard| first <= c.last_row && last >= c.first_row) { continue; }
        cards.push(BlockCard { id:id.into(), first_row:first, last_row:last, command:command.into(),
            exit_code:block["exit_code"].as_i64().and_then(|n|i32::try_from(n).ok()), duration_ms:block["duration_ms"].as_u64(), running:block["complete"] != true });
    }
    cards
}

/// Local wall-clock HH:MM for a Unix time in milliseconds.
pub fn local_hhmm(unix_ms: i64) -> String {
    let secs = (unix_ms / 1000) as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: localtime_r writes only into the tm we own; a null return leaves it zeroed.
    if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() { return String::new(); }
    format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
}

/// A finished command's duration as the meta row and the sticky line word it.
pub fn duration_label(ms:u64)->String{
    if ms<1000{format!("{ms} ms")}else if ms<60_000{format!("{:.1} s",ms as f64/1000.0)}
    else{format!("{} m {} s",ms/60_000,(ms%60_000)/1000)}
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn screen(rows: usize, cols: usize, written: &[usize]) -> TerminalSnapshot {
        let mut snapshot = TerminalSnapshot { rows, cols, cells: vec![clanky_core::Cell { ch: ' ', fg: [0; 3], bg: [0; 3], color_source: None, style: 0 }; rows * cols], ..Default::default() };
        for row in written { snapshot.cells[row * cols].ch = 'x'; }
        snapshot
    }

    /// Bash echoes the typed line on the prompt's own line, so a block whose output starts on the line after its
    /// prompt has its command row there; a finished block ends on the row before the next prompt.
    #[test]
    fn blocks_land_on_their_rows() {
        let view = json!({"viewport_line": 10, "blocks": [
            {"block_id": "a", "command": "mark m 0 0 one", "prompt_line": 10, "command_line": 11, "end_line": 12,
             "complete": true, "exit_code": 0, "duration_ms": 4},
            {"block_id": "b", "command": "mark c 3 0 two", "prompt_line": 12, "command_line": 13, "end_line": 14,
             "complete": true, "exit_code": 3, "duration_ms": 5},
            {"block_id": "c", "command": "sleep 9", "prompt_line": 14, "command_line": 15, "complete": false}
        ]});
        let cards = matched_blocks(&screen(6, 4, &[]), &view);
        let rows: Vec<_> = cards.iter().map(|c| (c.id.as_str(), c.first_row, c.last_row, c.exit_code, c.running)).collect();
        assert_eq!(rows, [("a", 0, 1, Some(0), false), ("b", 2, 3, Some(3), false), ("c", 4, 5, None, true)]);
    }

    /// The mockup's spacing at scale 1 with 26 px rows: the first block gets 9 + 21 + 2 = 32 px above it, a later
    /// block 11 + 1 + 32 = 44 px more.
    #[test]
    fn space_between_blocks() {
        let block = |id: &str, first_row, last_row| BlockCard { id: id.into(), first_row, last_row, command: String::new(), exit_code: Some(0), duration_ms: None, running: false };
        let breath = breathe(&[block("a", 0, 1), block("b", 2, 3)], &screen(4, 4, &[0, 1, 2, 3]), 26.0, 1.0, 1000.0, 0.0);
        assert_eq!(breath.map(|b| b.tops), Some(vec![32.0, 58.0, 128.0, 154.0]));
    }

    #[test]
    fn durations_read_as_words() {
        assert_eq!(duration_label(420), "420 ms");
        assert_eq!(duration_label(2_300), "2.3 s");
        assert_eq!(duration_label(125_000), "2 m 5 s");
    }
}
