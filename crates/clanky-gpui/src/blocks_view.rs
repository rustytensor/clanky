//! Command blocks over the terminal grid, in the quiet style (mockup section 1, `.blk` and `.meta`): a hairline above
//! every block after the first (in `err` when the block failed), the command row in bold, and the block's meta text
//! (folder, start time, then the running time, exit code and duration) at the right end of its command row, over
//! cells blanked for it. A block whose command row scrolled above the pane keeps its command in the sticky line at
//! the pane's top. All of it is drawing only: the grid, the screen the program sees and the server's rows do not
//! change, so selection and mouse reports read the real cells.

use clanky_core::{STYLE_BOLD, TerminalSnapshot};
use clanky_state::{
    blocks::{self, BlockCard, Breath, Spacing},
    themes::Colors,
};
use gpui::{
    AnyElement, FontFeatures, FontWeight, IntoElement, ParentElement, SharedString, Styled,
    TextRun, Window, div, font, px,
};
use serde_json::Value;
use std::sync::Arc;

use crate::{
    control_look::paint,
    control_record::ControlRecord,
    controls::INTERFACE_FONT_FAMILY,
    grid_element::{Metrics, TextSettings, colour},
    palette::Palette,
    tokens as t,
};

/// One group of meta words, whether it reports the failure, and whether it sits at the right (running time, exit
/// code, duration) rather than the left (folder, time) when the meta text has its own row.
#[derive(Clone, Debug, PartialEq)]
pub struct MetaText {
    pub text: String,
    pub failure: bool,
    pub right: bool,
}

/// A block as the pane draws it: its command row, whether it failed, and its meta text in reading order.
#[derive(Clone, Debug, PartialEq)]
pub struct ShownBlock {
    pub row: usize,
    pub failed: bool,
    pub meta: Vec<MetaText>,
}

/// The sticky line: the command, and its running time or duration on the right.
#[derive(Clone, Debug, PartialEq)]
pub struct StickyLine {
    pub command: String,
    pub right: String,
}

/// What the pane draws over its grid, and whether a running time is counting (one frame a second).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shown {
    pub blocks: Vec<ShownBlock>,
    /// The blocks as the server placed them on the screen's rows.
    pub cards: Vec<BlockCard>,
    pub sticky: Option<StickyLine>,
    pub counting: bool,
}

/// Digits of equal width, so a counting time or a column of durations does not shift (the rulebook's tabular meta).
pub fn tabular_digits() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

fn plain(text: String, right: bool) -> Option<MetaText> {
    (!text.is_empty()).then_some(MetaText {
        text,
        failure: false,
        right,
    })
}

fn running_label(block: &Value, now_ms: i64) -> String {
    let started = block["started_unix_ms"].as_i64().unwrap_or(now_ms);
    format!("running {} s", (now_ms - started).max(0) / 1000)
}

/// The blocks on `snapshot`, placed by the server's anchors in `view` (the reply that came with the snapshot).
pub fn shown(snapshot: &TerminalSnapshot, view: &Value, now_ms: i64, home: Option<&str>) -> Shown {
    let mut counting = false;
    let listed = view["blocks"].as_array();
    let cards = blocks::matched_blocks(snapshot, view);
    let shown_blocks = cards
        .iter()
        .map(|card| {
            let block = listed
                .and_then(|list| {
                    list.iter()
                        .find(|b| b["block_id"].as_str() == Some(card.id.as_str()))
                })
                .unwrap_or(&Value::Null);
            let failed = !card.running && card.exit_code.is_some_and(|code| code != 0);
            let mut meta: Vec<MetaText> = [
                block["cwd"]
                    .as_str()
                    .and_then(|cwd| plain(blocks::home_short(cwd, home), false)),
                block["started_unix_ms"]
                    .as_i64()
                    .and_then(|at| plain(blocks::local_hhmm(at), false)),
            ]
            .into_iter()
            .flatten()
            .collect();
            if card.running {
                counting = true;
                meta.extend(plain(running_label(block, now_ms), true));
            }
            if failed {
                meta.push(MetaText {
                    text: format!("exit {}", card.exit_code.unwrap_or(1)),
                    failure: true,
                    right: true,
                });
            }
            if let Some(ms) = card.duration_ms.filter(|_| !card.running) {
                meta.extend(plain(blocks::duration_label(ms), true));
            }
            ShownBlock {
                row: card.first_row,
                failed,
                meta,
            }
        })
        .collect();
    let line = &view["sticky"];
    let sticky = line["command"]
        .as_str()
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .map(|command| {
            let right = if line["complete"] == true {
                line["duration_ms"]
                    .as_u64()
                    .map(blocks::duration_label)
                    .unwrap_or_default()
            } else {
                counting = true;
                running_label(line, now_ms)
            };
            StickyLine {
                command: command.to_owned(),
                right,
            }
        });
    Shown {
        blocks: shown_blocks,
        cards,
        sticky,
        counting,
    }
}

fn meta_runs(text: &str, color: gpui::Hsla) -> Vec<TextRun> {
    let mut face = font(INTERFACE_FONT_FAMILY);
    face.features = tabular_digits();
    vec![TextRun {
        len: text.len(),
        font: face,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
        letter_spacing: None,
    }]
}

/// Each block's meta text width in logical pixels, words and the gaps between them.
pub fn meta_widths(shown: &Shown, window: &Window) -> Vec<f32> {
    let system = window.text_system();
    let width = |text: &str| {
        let runs = meta_runs(text, gpui::black());
        f32::from(
            system
                .shape_line(SharedString::from(text.to_owned()), px(t::TEXT_META), &runs)
                .width,
        )
    };
    shown
        .blocks
        .iter()
        .map(|block| {
            // Each group's words with their 4 px gaps, and 8 px between groups, as `overlay` lays them out.
            let groups: f32 = block
                .meta
                .iter()
                .map(|meta| {
                    let parts: Vec<&str> = meta.text.split(' ').collect();
                    parts.iter().map(|part| width(part)).sum::<f32>()
                        + t::SPACE_4 * parts.len().saturating_sub(1) as f32
                })
                .sum();
            groups + t::SPACE_8 * block.meta.len().saturating_sub(1) as f32
        })
        .collect()
}

/// The snapshot as drawn: each command row in bold, and the cells under its meta text blank (with a gap before it).
pub fn decorate(
    snapshot: &TerminalSnapshot,
    shown: &Shown,
    widths: Option<&[f32]>,
    m: Metrics,
) -> TerminalSnapshot {
    let mut drawn = snapshot.clone();
    let cols = drawn.cols;
    let pad = (t::PANE_PADDING * m.scale).floor();
    for (index, block) in shown.blocks.iter().enumerate() {
        if block.row >= drawn.rows {
            continue;
        }
        // Device pixels from the pane's left edge: where the meta text starts, less the gap before it. Meta text
        // in a row of its own (`widths` None) blanks nothing.
        let clear_from = widths
            .and_then(|widths| widths.get(index))
            .map_or(f32::INFINITY, |width| {
                m.pane_width - pad - (width + t::SPACE_8) * m.scale
            });
        for (col, cell) in drawn.cells[block.row * cols..(block.row + 1) * cols]
            .iter_mut()
            .enumerate()
        {
            cell.style |= STYLE_BOLD;
            if !block.meta.is_empty() && pad + (col + 1) as f32 * m.cell_width > clear_from {
                cell.ch = ' ';
            }
        }
    }
    drawn
}

/// What the overlay draws with.
pub struct Look<'a> {
    pub colors: &'a Colors,
    pub palette: &'a Palette,
    pub text: &'a TextSettings,
    pub record: &'a ControlRecord,
    /// Only the focused pane names its meta text in the control record, so two panes never share a landmark.
    pub landmarks: bool,
    /// The rows with space between blocks, and that space; None for the plain grid.
    pub spaced: Option<(&'a Breath, Spacing)>,
}

/// The hairlines, meta text and sticky line, placed over the grid with the grid's own row geometry.
pub fn overlay(shown: &Shown, m: Metrics, look: &Look) -> AnyElement {
    let pad = (t::PANE_PADDING * m.scale).floor();
    // A row's top in device pixels below the pane's top edge.
    let top_of = |row: usize| {
        pad + look
            .spaced
            .map_or(row as f32 * m.line_height, |(breath, _)| {
                breath
                    .tops
                    .get(row)
                    .copied()
                    .unwrap_or(row as f32 * m.line_height)
            })
    };
    let logical = |device: f32| px(device / m.scale);
    let colors = look.colors;
    let mut layer = div().absolute().top_0().left_0().size_full();
    for (index, block) in shown.blocks.iter().enumerate() {
        let line = if block.failed {
            paint(colors.err, t::BLOCK_DIVIDER_FAILED)
        } else {
            paint(colors.foreground, t::BLOCK_DIVIDER)
        };
        let hairline = match look.spaced {
            Some((breath, spacing)) => spacing
                .hairline(breath, block.row, m.line_height)
                .map(|y| pad + y),
            None => (block.row > 0).then(|| top_of(block.row)),
        };
        if let Some(y) = hairline.filter(|y| *y >= pad) {
            layer = layer.child(
                div()
                    .absolute()
                    .left_0()
                    .w_full()
                    .top(logical(y))
                    .h(px(t::hairline(m.scale)))
                    .bg(line),
            );
        }
        if block.meta.is_empty() {
            continue;
        }
        // The meta text: a row of its own above the command (folder and time at the grid's first column, the rest
        // at the right), or with the classic input the right end of the command row.
        let (top, height) = match look.spaced {
            Some((breath, spacing)) => (pad + spacing.meta_top(breath, block.row), spacing.meta),
            None => (top_of(block.row), m.line_height),
        };
        // Meta text lifted above the grid with its rows is not drawn over the pane's top.
        if top < pad {
            continue;
        }
        let words = |right: bool| {
            div().flex().items_center().gap(px(t::SPACE_8)).children(
                block
                    .meta
                    .iter()
                    .filter(|word| look.spaced.is_none() || word.right == right)
                    .map(|word| {
                        // The words of one group keep a gap wider than a space at 10 px, so "2.3 s" never reads as
                        // one word (today's meta row does the same).
                        let text = div().flex().gap(px(t::SPACE_4)).children(
                            word.text
                                .split(' ')
                                .map(|part| div().child(part.to_owned())),
                        );
                        if word.failure {
                            text.text_color(paint(colors.err, 1.0))
                        } else {
                            text
                        }
                    }),
            )
        };
        let mut meta = div()
            .absolute()
            .top(logical(top))
            .h(logical(height))
            .flex()
            .items_center()
            .font_family(INTERFACE_FONT_FAMILY)
            .font_features(tabular_digits())
            .text_size(px(t::TEXT_META))
            .whitespace_nowrap()
            .text_color(paint(colors.muted_foreground, 1.0));
        meta = if look.spaced.is_some() {
            // A row of its own across the pane, inside the padding the grid has (the hairlines span it the same way).
            meta.left_0()
                .w_full()
                .px(px(t::PANE_PADDING))
                .child(words(false))
                .child(div().flex_1())
                .child(words(true))
        } else {
            meta.right(px(t::PANE_PADDING)).child(words(true))
        };
        if look.landmarks && look.record.enabled() {
            let index = u32::try_from(index).unwrap_or(u32::MAX);
            meta = meta.child(look.record.landmark("meta-text", index));
        }
        layer = layer.child(meta);
    }
    if let Some(sticky) = &shown.sticky {
        layer = layer.child(sticky_line(sticky, m, look));
    }
    layer.into_any_element()
}

/// With space between blocks, a block whose command row the rows' lift moved above the pane while its output still
/// shows keeps its command in the sticky line, unless the server already named one.
pub fn lifted_sticky(shown: &mut Shown, breath: &Breath, line_height: f32) {
    if shown.sticky.is_some() {
        return;
    }
    let lifted = shown.cards.iter().zip(&shown.blocks).find(|(card, _)| {
        let last = card.last_row.min(breath.tops.len().saturating_sub(1));
        breath
            .tops
            .get(card.first_row)
            .is_some_and(|top| *top < 0.0)
            && breath
                .tops
                .get(last)
                .is_some_and(|top| *top + line_height > line_height)
    });
    if let Some((card, block)) = lifted {
        let right = block
            .meta
            .iter()
            .rev()
            .find(|word| word.right && !word.failure)
            .map(|word| word.text.clone())
            .unwrap_or_default();
        shown.sticky = Some(StickyLine {
            command: card.command.trim().to_owned(),
            right,
        });
    }
}

/// The sticky line covers the pane's top padding and first row, in `card`, with the command in the terminal's bold.
fn sticky_line(sticky: &StickyLine, m: Metrics, look: &Look) -> AnyElement {
    let pad = (t::PANE_PADDING * m.scale).floor();
    div()
        .absolute()
        .top_0()
        .left_0()
        .w_full()
        .h(px((pad + m.line_height) / m.scale))
        .bg(paint(look.colors.card, 1.0))
        .child(
            div()
                .absolute()
                .top(px(pad / m.scale))
                .left(px(pad / m.scale))
                .right(px(t::PANE_PADDING))
                .h(px(m.line_height / m.scale))
                .flex()
                .items_center()
                .gap(px(t::SPACE_8))
                .whitespace_nowrap()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .font_family(look.text.family.clone())
                        .text_size(px(look.text.size))
                        .font_weight(FontWeight::BOLD)
                        .text_color(colour(look.palette.foreground))
                        .child(sticky.command.clone()),
                )
                .child(
                    div()
                        .flex_none()
                        .font_family(INTERFACE_FONT_FAMILY)
                        .font_features(tabular_digits())
                        .text_size(px(t::TEXT_META))
                        .text_color(paint(look.colors.muted_foreground, 1.0))
                        .child(sticky.right.clone()),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn screen(rows: usize, cols: usize) -> TerminalSnapshot {
        TerminalSnapshot {
            rows,
            cols,
            cells: vec![
                clanky_core::Cell {
                    ch: 'x',
                    fg: [0; 3],
                    bg: [0; 3],
                    color_source: None,
                    style: 0
                };
                rows * cols
            ],
            ..Default::default()
        }
    }

    /// The q4 gate's three blocks: exit 0, exit 3, and one still running for 2 s; the folder is ~/alpha.
    #[test]
    fn meta_text_in_reading_order() {
        let view = json!({"viewport_line": 0, "blocks": [
            {"block_id": "a", "command": "mark m 0 0 one", "prompt_line": 0, "command_line": 1, "end_line": 2,
             "complete": true, "exit_code": 0, "duration_ms": 40, "cwd": "/home/u/alpha"},
            {"block_id": "b", "command": "mark c 3 0 two", "prompt_line": 2, "command_line": 3, "end_line": 4,
             "complete": true, "exit_code": 3, "duration_ms": 2_300, "cwd": "/home/u/alpha"},
            {"block_id": "c", "command": "sleep 9", "prompt_line": 4, "command_line": 5, "complete": false,
             "started_unix_ms": 10_000, "cwd": "/home/u/alpha"}
        ], "sticky": null});
        let shown = shown(&screen(6, 4), &view, 12_500, Some("/home/u"));
        let words = |i: usize| -> Vec<(String, bool)> {
            shown.blocks[i]
                .meta
                .iter()
                .map(|m| (m.text.clone(), m.failure))
                .collect()
        };
        let label = |text: &str, failure| (text.to_owned(), failure);
        assert_eq!(words(0), [label("~/alpha", false), label("40 ms", false)]);
        assert_eq!(
            words(1),
            [
                label("~/alpha", false),
                label("exit 3", true),
                label("2.3 s", false)
            ]
        );
        assert_eq!(words(2)[0], label("~/alpha", false));
        assert_eq!(words(2).last(), Some(&label("running 2 s", false)));
        assert_eq!(
            shown
                .blocks
                .iter()
                .map(|b| (b.row, b.failed))
                .collect::<Vec<_>>(),
            [(0, false), (2, true), (4, false)]
        );
        assert!(shown.counting);
    }

    #[test]
    fn sticky_line_reads_the_running_time() {
        let view = json!({"viewport_line": 40, "blocks": [], "sticky":
            {"block_id": "s", "command": " : stickyjob; seq 1 150 ", "complete": false, "started_unix_ms": 1_000}});
        let shown = shown(&screen(2, 2), &view, 4_999, None);
        assert_eq!(
            shown.sticky,
            Some(StickyLine {
                command: ": stickyjob; seq 1 150".into(),
                right: "running 3 s".into()
            })
        );
    }

    /// 4 columns of 10 device px after 12 px of padding in a 100 px pane: meta text 30 px wide starts at
    /// 100 - 12 - 30 = 58, so with the 8 px gap cells ending after 50 px are blanked: columns 3 (ends at 52) and on.
    #[test]
    fn cells_under_the_meta_text_are_blank_and_the_command_row_bold() {
        let shown = Shown {
            blocks: vec![ShownBlock {
                row: 1,
                failed: false,
                meta: vec![MetaText {
                    text: "1 s".into(),
                    failure: false,
                    right: true,
                }],
            }],
            cards: Vec::new(),
            sticky: None,
            counting: false,
        };
        let m = Metrics {
            scale: 1.0,
            cell_width: 10.0,
            line_height: 20.0,
            pane_width: 100.0,
            rows: 2,
            cols: 4,
            ..Metrics::default()
        };
        let drawn = decorate(&screen(2, 4), &shown, Some(&[30.0]), m);
        let row =
            |r: usize| -> String { drawn.cells[r * 4..r * 4 + 4].iter().map(|c| c.ch).collect() };
        assert_eq!(row(0), "xxxx");
        assert_eq!(row(1), "xxx ");
        assert!(drawn.cells[4..8].iter().all(|c| c.style & STYLE_BOLD != 0));
        assert!(drawn.cells[..4].iter().all(|c| c.style & STYLE_BOLD == 0));
    }
}
