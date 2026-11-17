//! The pane's command blocks: what is drawn over the grid, and with the command input on, the space between blocks.
//! The space is drawing only: rows keep their places on the program's screen, and the mouse maps through the space
//! to the rows under it.

use std::{rc::Rc, sync::Arc};

use clanky_core::{MODE_ALT_SCREEN, TerminalSnapshot};
use clanky_state::blocks::{self, Breath, Spacing};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};

use super::TerminalPane;
use crate::{
    blocks_view::{self, Look},
    tokens as t,
};

/// What the pane draws: the snapshot as drawn, each row's top when blocks have space between them, and the blocks
/// over the grid.
pub(super) struct Drawn {
    pub snapshot: Option<Arc<TerminalSnapshot>>,
    pub tops: Option<Rc<[f32]>>,
    pub overlay: Option<AnyElement>,
}

/// The rulebook's space between blocks at `scale`, in whole device pixels; the divider is one device pixel.
fn spacing(scale: f32) -> Spacing {
    let device = |logical: f32| (logical * scale).round();
    Spacing {
        below: device(t::BLOCK_SPACE_BELOW),
        line: 1.0,
        above: device(t::BLOCK_SPACE_ABOVE),
        meta: device(t::BLOCK_META_ROW_HEIGHT),
        meta_gap: device(t::BLOCK_META_GAP),
    }
}

impl TerminalPane {
    pub(super) fn blocks_drawn(&mut self, window: &Window, cx: &mut Context<Self>) -> Drawn {
        let m = self.metrics.get();
        self.breath = None;
        let Some(snapshot) = self.snapshot.clone().filter(|_| m.scale > 0.0) else {
            return Drawn {
                snapshot: self.snapshot.clone(),
                tops: None,
                overlay: None,
            };
        };
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| {
                i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
            });
        let mut shown = blocks_view::shown(&snapshot, &self.blocks, now_ms, self.home.as_deref());
        if shown.counting && self.tick.is_none() {
            let wait =
                std::time::Duration::from_millis(1000 - now_ms.rem_euclid(1000).unsigned_abs() + 5);
            self.tick = Some(cx.spawn(async move |pane, cx| {
                cx.background_executor().timer(wait).await;
                let _ = pane.update(cx, |pane, cx| {
                    pane.tick = None;
                    cx.notify();
                });
            }));
        }
        // Space between blocks comes with the command input, on the normal screen only.
        if self.input.is_some() && snapshot.modes & MODE_ALT_SCREEN == 0 {
            let spacing = spacing(m.scale);
            let pad = (t::PANE_PADDING * m.scale).floor();
            let visible = m.pane_height - 2.0 * pad;
            match blocks::breathe_spaced(
                &shown.cards,
                &snapshot,
                m.line_height,
                spacing,
                visible,
                self.lift,
            ) {
                Some(breath) => {
                    self.lift = self.lift.min(breath.shift);
                    blocks_view::lifted_sticky(&mut shown, &breath, m.line_height);
                    self.breath = Some((breath, spacing));
                }
                None => self.lift = 0.0,
            }
        }
        if shown.blocks.is_empty() && shown.sticky.is_none() {
            return Drawn {
                snapshot: Some(snapshot),
                tops: None,
                overlay: None,
            };
        }
        // Meta text at the end of the command row needs its cells blanked; in a row of its own it needs none.
        let widths = self
            .breath
            .is_none()
            .then(|| blocks_view::meta_widths(&shown, window));
        let drawn = if shown.blocks.is_empty() {
            snapshot
        } else {
            Arc::new(blocks_view::decorate(
                &snapshot,
                &shown,
                widths.as_deref(),
                m,
            ))
        };
        let look = Look {
            colors: &self.colors,
            palette: &self.palette,
            text: &self.text,
            record: &self.record,
            landmarks: self.focus.is_focused(window),
            spaced: self
                .breath
                .as_ref()
                .map(|(breath, spacing)| (breath, *spacing)),
        };
        Drawn {
            snapshot: Some(drawn),
            tops: self
                .breath
                .as_ref()
                .map(|(breath, _)| Rc::from(breath.tops.as_slice())),
            overlay: Some(blocks_view::overlay(&shown, m, &look)),
        }
    }

    /// The focused pane's first text row, named `pane-text` in the control record for the gate's selection drag.
    pub(super) fn text_landmark(&self, window: &Window) -> Option<AnyElement> {
        let m = self.metrics.get();
        if !self.record.enabled() || !self.focus.is_focused(window) || m.scale <= 0.0 {
            return None;
        }
        let pad = (t::PANE_PADDING * m.scale).floor();
        Some(
            div()
                .absolute()
                .left(px(pad / m.scale))
                .top(px((pad + self.row_offset(0)) / m.scale))
                .w(px(m.cols as f32 * m.cell_width / m.scale))
                .h(px(m.line_height / m.scale))
                .child(self.record.landmark("pane-text", 0))
                .into_any_element(),
        )
    }

    /// The grid row under `dy` device pixels below the grid's origin: through the space between blocks when there
    /// is some, a point in the space belonging to the next block's command row.
    pub(super) fn row_at(&self, dy: f32) -> Option<usize> {
        let (breath, _) = self.breath.as_ref()?;
        let m = self.metrics.get();
        Some(
            breath
                .row_at(dy, m.line_height)
                .min(m.rows.saturating_sub(1)),
        )
    }

    /// A row's top in device pixels below the grid's origin.
    pub(super) fn row_offset(&self, row: usize) -> f32 {
        let m = self.metrics.get();
        self.breath
            .as_ref()
            .and_then(|(breath, _)| breath.tops.get(row).copied())
            .unwrap_or(row as f32 * m.line_height)
    }

    /// The wheel over rows lifted to keep the newest one in view: at the top of the history, up takes the lift
    /// back to show the rows above; down gives it back before the history scrolls. True when it moved the lift.
    pub(super) fn lift_by_wheel(&mut self, lines: f32) -> bool {
        let Some((breath, _)) = self.breath.as_ref() else {
            return false;
        };
        let shift = breath.shift;
        let line_height = self.metrics.get().line_height;
        let at_top = self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.display_offset >= s.history);
        let next = if lines > 0.0 && at_top && self.lift < shift {
            (self.lift + lines * line_height).min(shift)
        } else if lines < 0.0 && self.lift > 0.0 {
            (self.lift + lines * line_height).max(0.0)
        } else {
            return false;
        };
        self.lift = next;
        true
    }
}

/// The rows with space between command blocks, and that space.
pub(super) type Spaced = (Breath, Spacing);
