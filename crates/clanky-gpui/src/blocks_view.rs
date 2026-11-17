//! Command blocks over the terminal grid, in the quiet style (mockup section 1, `.blk` and `.meta`): a hairline above
//! every block after the first (in `err` when the block failed), the command row in bold, and the block's meta text
//! (folder, start time, then the running time, exit code and duration) at the right end of its command row, over
//! cells blanked for it. A block whose command row scrolled above the pane keeps its command in the sticky line at
//! the pane's top. All of it is drawing only: the grid, the screen the program sees and the server's rows do not
//! change, so selection and mouse reports read the real cells.

use clanky_core::{STYLE_BOLD, TerminalSnapshot};
use clanky_state::{blocks, themes::Colors};
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

/// One group of meta words, and whether it reports the failure.
#[derive(Clone, Debug, PartialEq)]
pub struct MetaText {
    pub text: String,
    pub failure: bool,
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
    pub sticky: Option<StickyLine>,
    pub counting: bool,
}

/// Digits of equal width, so a counting time or a column of durations does not shift (the rulebook's tabular meta).
pub fn tabular_digits() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

fn plain(text: String) -> Option<MetaText> {
    (!text.is_empty()).then_some(MetaText {
        text,
        failure: false,
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
    let shown_blocks = blocks::matched_blocks(snapshot, view)
        .into_iter()
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
                    .and_then(|cwd| plain(blocks::home_short(cwd, home))),
                block["started_unix_ms"]
                    .as_i64()
                    .and_then(|at| plain(blocks::local_hhmm(at))),
            ]
            .into_iter()
            .flatten()
            .collect();
            if card.running {
                counting = true;
                meta.extend(plain(running_label(block, now_ms)));
            }
            if failed {
                meta.push(MetaText {
                    text: format!("exit {}", card.exit_code.unwrap_or(1)),
                    failure: true,
                });
            }
            if let Some(ms) = card.duration_ms.filter(|_| !card.running) {
                meta.extend(plain(blocks::duration_label(ms)));
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
    shown
        .blocks
        .iter()
        .map(|block| {
            let words: f32 = block
                .meta
                .iter()
                .map(|meta| {
                    let runs = meta_runs(&meta.text, gpui::black());
                    f32::from(
                        system
                            .shape_line(
                                SharedString::from(meta.text.clone()),
                                px(t::TEXT_META),
                                &runs,
                            )
                            .width,
                    )
                })
                .sum();
            words + t::SPACE_8 * block.meta.len().saturating_sub(1) as f32
        })
        .collect()
}

/// The snapshot as drawn: each command row in bold, and the cells under its meta text blank (with a gap before it).
pub fn decorate(
    snapshot: &TerminalSnapshot,
    shown: &Shown,
    widths: &[f32],
    m: Metrics,
) -> TerminalSnapshot {
    let mut drawn = snapshot.clone();
    let cols = drawn.cols;
    let pad = (t::PANE_PADDING * m.scale).floor();
    for (block, width) in shown.blocks.iter().zip(widths) {
        if block.row >= drawn.rows {
            continue;
        }
        // Device pixels from the pane's left edge: where the meta text starts, less the gap before it.
        let clear_from = m.pane_width - pad - (width + t::SPACE_8) * m.scale;
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
}

/// The hairlines, meta text and sticky line, placed over the grid with the grid's own row geometry.
pub fn overlay(shown: &Shown, m: Metrics, look: &Look) -> AnyElement {
    let pad = (t::PANE_PADDING * m.scale).floor();
    let row_top = |row: usize| px((pad + row as f32 * m.line_height) / m.scale);
    let row_height = px(m.line_height / m.scale);
    let colors = look.colors;
    let mut layer = div().absolute().top_0().left_0().size_full();
    for (index, block) in shown.blocks.iter().enumerate() {
        if block.row > 0 {
            let line = if block.failed {
                paint(colors.err, t::BLOCK_DIVIDER_FAILED)
            } else {
                paint(colors.foreground, t::BLOCK_DIVIDER)
            };
            layer = layer.child(
                div()
                    .absolute()
                    .left_0()
                    .w_full()
                    .top(row_top(block.row))
                    .h(px(t::hairline(m.scale)))
                    .bg(line),
            );
        }
        if block.meta.is_empty() {
            continue;
        }
        let mut meta = div()
            .absolute()
            .right(px(t::PANE_PADDING))
            .top(row_top(block.row))
            .h(row_height)
            .flex()
            .items_center()
            .gap(px(t::SPACE_8))
            .font_family(INTERFACE_FONT_FAMILY)
            .font_features(tabular_digits())
            .text_size(px(t::TEXT_META))
            .whitespace_nowrap()
            .text_color(paint(colors.muted_foreground, 1.0))
            .children(block.meta.iter().map(|word| {
                let text = div().child(word.text.clone());
                if word.failure {
                    text.text_color(paint(colors.err, 1.0))
                } else {
                    text
                }
            }));
        if look.landmarks {
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
                }],
            }],
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
        let drawn = decorate(&screen(2, 4), &shown, &[30.0], m);
        let row =
            |r: usize| -> String { drawn.cells[r * 4..r * 4 + 4].iter().map(|c| c.ch).collect() };
        assert_eq!(row(0), "xxxx");
        assert_eq!(row(1), "xxx ");
        assert!(drawn.cells[4..8].iter().all(|c| c.style & STYLE_BOLD != 0));
        assert!(drawn.cells[..4].iter().all(|c| c.style & STYLE_BOLD == 0));
    }
}
