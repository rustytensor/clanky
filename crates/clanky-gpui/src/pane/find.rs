//! What the pane shows of its find bar: the matches marked on the grid (the current one in the selection colour, as
//! VTE selects it, the others in `accent`), and the current match's box in the control record. The bar itself and
//! its search are the window's.

use clanky_state::pane_find::Run;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};

use super::TerminalPane;
use crate::tokens as t;

/// The colour of a theme colour's red, green and blue, as the grid paints cells.
fn rgb(colour: [u8; 4]) -> [u8; 3] {
    [colour[0], colour[1], colour[2]]
}

impl TerminalPane {
    /// Marks the find bar's matches on the pane's grid; an empty list takes the marks away.
    pub fn show_found(&mut self, runs: Vec<Run>, cx: &mut Context<Self>) {
        if self.found != runs {
            self.found = runs;
            cx.notify();
        }
    }

    /// The grid's area in the window, in logical pixels `[x, y, w, h]`, once the pane has been laid out.
    pub fn grid_area(&self) -> Option<[f32; 4]> {
        let m = self.metrics.get();
        if m.scale <= 0.0 {
            return None;
        }
        let pad = (t::PANE_PADDING * m.scale).floor();
        Some([
            (m.origin_x - pad) / m.scale,
            (m.origin_y - pad) / m.scale,
            m.pane_width / m.scale,
            m.pane_height / m.scale,
        ])
    }

    /// The marked runs with their fills, for the grid.
    pub(super) fn found_fills(&self) -> Vec<(usize, usize, usize, [u8; 3])> {
        self.found
            .iter()
            .map(|run| {
                let fill = if run.current {
                    self.palette.selection
                } else {
                    rgb(self.colors.accent)
                };
                (run.row, run.start, run.end, fill)
            })
            .collect()
    }

    /// The current match's first run, measured as the record's `find-match` landmark.
    pub(super) fn found_landmark(&self) -> Option<AnyElement> {
        let m = self.metrics.get();
        let run = self.found.iter().find(|run| run.current)?;
        if !self.record.enabled() || m.scale <= 0.0 {
            return None;
        }
        let pad = (t::PANE_PADDING * m.scale).floor();
        Some(
            div()
                .absolute()
                .left(px((pad + run.start as f32 * m.cell_width) / m.scale))
                .top(px((pad + self.row_offset(run.row)) / m.scale))
                .w(px((run.end - run.start) as f32 * m.cell_width / m.scale))
                .h(px(m.line_height / m.scale))
                .child(self.record.landmark("find-match", 0))
                .into_any_element(),
        )
    }
}
