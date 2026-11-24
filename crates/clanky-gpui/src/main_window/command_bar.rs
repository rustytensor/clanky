//! The command input under each terminal pane (mockup sections 1 and 2, `.input`): a hairline, a row of chips (folder,
//! git branch, shell, agent access) with the Shell | Agent switch at its right, then the line being written in the
//! terminal font with bash colours, the caret and grey ghost text from history; after Tab the completion list opens
//! above it (`completion_list.rs`). While a command runs, a full-screen program has the keys or the shell sets no
//! marks, the line says where keys go and the chips dim.

use std::{cell::Cell, collections::BTreeMap, ops::Range, path::Path, rc::Rc};

use clanky_api::Client;
use clanky_state::{
    command_input::Tok,
    pane_input::{self, View},
};
use gpui::{
    AnyElement, App, Bounds, Context, Entity, HighlightStyle, Hsla, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Styled, StyledText, Window, canvas, div, px,
    svg,
};
use serde_json::Value;

use super::{MainWindow, SCREEN};
use crate::{
    control_look::{self, Kind as LookKind, State, paint},
    controls::{Control, Group, INTERFACE_FONT_FAMILY, Kind, Switcher},
    grid_element::colour,
    icons::Icon,
    pane::TerminalPane,
    tokens as t,
};

/// Control ids of the inputs: each pane's input takes ten from here, by the pane's place in the window.
const INPUT_IDS: u32 = 6000;
/// The input shows at most this many lines of text; more scroll.
const MAX_LINES: usize = 8;

/// What a pane's chips show, read from the server once a second.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Chips {
    folder: String,
    branch: Option<String>,
    shell: String,
    access: bool,
}

impl Chips {
    pub(super) fn set_access(&mut self, allowed: bool) {
        self.access = allowed;
    }
}

/// Each pane's chips: its shell's folder and git branch, the shell's name and the pane's agent access. `infos` holds
/// each pane's `terminal_info` reply from this look, shared with the side tabs so a pane is asked once.
pub(super) fn read_chips(
    client: &Client,
    ids: &[String],
    infos: &BTreeMap<String, Value>,
) -> BTreeMap<String, Chips> {
    let home = clanky_core::platform::home_dir().and_then(|home| home.to_str().map(str::to_owned));
    ids.iter()
        .map(|id| {
            let info = infos.get(id).cloned().unwrap_or_default();
            let cwd = info["cwd"].as_str().unwrap_or_default();
            let chips = Chips {
                folder: if cwd.is_empty() {
                    String::new()
                } else {
                    clanky_state::blocks::home_short(cwd, home.as_deref())
                },
                branch: (!cwd.is_empty())
                    .then(|| pane_input::git_branch(Path::new(cwd)))
                    .flatten(),
                shell: info["exe"]
                    .as_str()
                    .and_then(|exe| Path::new(exe).file_name())
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                access: {
                    crate::request_log::note("permission_get");
                    client.agent_permission(id).unwrap_or(false)
                },
            };
            (id.clone(), chips)
        })
        .collect()
}

impl MainWindow {
    /// The command input under pane `id`, the `place`-th pane of the window; None for a pane without one.
    pub(super) fn command_bar(
        &self,
        id: &str,
        place: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let pane = self.panes.get(id)?.0.clone();
        let (shown, focused) = {
            let read = pane.read(cx);
            (read.input_shown()?, read.focus_handle().is_focused(window))
        };
        let chips = self.status.chips.get(id).cloned().unwrap_or_default();
        let dim = if shown.aside.is_some() {
            t::DISABLED_OPACITY
        } else {
            1.0
        };
        let line = match shown.aside {
            Some(aside) => div()
                .h(px(self.line_height()))
                .flex()
                .items_center()
                .font_family(INTERFACE_FONT_FAMILY)
                .text_size(px(t::TEXT_UI))
                .text_color(paint(self.colors.muted_foreground, dim))
                .child(aside.text())
                .into_any_element(),
            None => self.text_lines(&shown.view, focused && shown.caret, &pane, cx),
        };
        let focus_pane = pane.clone();
        let measured = super::completion_list::Measured::default();
        let bar = div()
            .relative()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(t::SPACE_8))
            .px(px(t::PANE_PADDING))
            .pt(px(t::SPACE_8))
            .pb(px(t::PANE_PADDING))
            .border_t(px(t::hairline(window.scale_factor())))
            .border_color(paint(self.colors.border, 1.0))
            // A click in the input gives the pane the keyboard.
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                let handle = focus_pane.read(cx).focus_handle().clone();
                window.focus(&handle, cx);
            })
            .child(self.chips_row(id, place, &chips, &shown.view, dim, window, cx))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(t::SPACE_8))
                    .child(
                        // The prompt mark (mockup ❯) as the chevron icon, so it never depends on the terminal
                        // font having the glyph; in the terminal's blue.
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .h(px(self.line_height()))
                            .child(
                                svg()
                                    .data(Icon::ChevronRight.svg())
                                    .size(px(t::ICON_SIZE_BAR))
                                    .text_color(paint_rgb(self.palette.ansi[4], dim)),
                            ),
                    )
                    .child(div().flex_1().min_w_0().overflow_hidden().child(line)),
            )
            .child(Self::measure_bar(&measured));
        let listed = shown.view.popup.is_some() && shown.aside.is_none();
        self.follow_completion_list(id, listed, focused, &measured, window, cx);
        Some(bar.into_any_element())
    }

    /// One terminal row's height in logical pixels, as the grid draws it.
    fn line_height(&self) -> f32 {
        self.text.size * self.text.line_height
    }

    #[allow(clippy::too_many_arguments)]
    fn chips_row(
        &self,
        id: &str,
        place: u32,
        chips: &Chips,
        view: &View,
        dim: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let base = INPUT_IDS + place * 10;
        let mut row = div()
            .flex()
            .items_center()
            .gap(px(t::SPACE_6))
            .h(px(t::CHIP_HEIGHT))
            .font_family(INTERFACE_FONT_FAMILY)
            .text_size(px(t::TEXT_UI));
        let mut labels: Vec<String> = Vec::new();
        if !chips.folder.is_empty() {
            labels.push(chips.folder.clone());
        }
        if let Some(branch) = &chips.branch {
            labels.push(format!("⎇ {branch}"));
        }
        if !chips.shell.is_empty() {
            labels.push(chips.shell.clone());
        }
        for label in labels {
            row = row.child(self.chip(dim).child(label));
        }
        if !self.post_ai {
            let this = cx.entity();
            let (pane, allowed) = (id.to_owned(), chips.access);
            let switch = Control::new(Kind::Switch, base, "Agent access")
                .checked(allowed)
                .on_click(move |_, cx| {
                    let pane = pane.clone();
                    this.update(cx, |this, cx| this.set_access(&pane, !allowed, cx));
                });
            row = row.child(
                self.chip(dim)
                    .gap(px(t::SPACE_8))
                    .child("Agent access")
                    .child(self.kit.build(SCREEN, switch, window, cx)),
            );
        }
        if let Some(notice) = &view.notice {
            row = row.child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(t::TEXT_STATUS))
                    .text_color(paint(self.colors.err, 1.0))
                    .child(notice.clone()),
            );
        }
        row = row.child(div().flex_1());
        if !self.post_ai {
            // Shell | Agent: the pane is a shell; Agent opens the agent picker to start one beside it.
            let segments = [(base + 1, "Shell"), (base + 2, "Agent")];
            let (this, pane) = (cx.entity(), id.to_owned());
            let switcher = Switcher {
                group: Group::horizontal("command-mode"),
                segments: &segments,
                selected: 0,
                choose: Rc::new(move |index, window, cx: &mut App| {
                    if index == 1 {
                        let pane = pane.clone();
                        this.update(cx, |this, cx| {
                            this.agent_segment_chosen(base + 2, pane, window, cx);
                        });
                    }
                }),
            };
            row = row.child(self.kit.segmented(SCREEN, &switcher, window, cx));
        }
        row.into_any_element()
    }

    /// A chip that shows a fact (folder, branch, shell): the chip's rest look, never a control.
    fn chip(&self, dim: f32) -> gpui::Div {
        let colors = &self.colors;
        let mut chip = div()
            .flex_none()
            .flex()
            .items_center()
            .h(px(LookKind::Chip.height()))
            .px(px(t::SPACE_8))
            .rounded(px(LookKind::Chip.radius()))
            .whitespace_nowrap()
            .text_color(paint(colors.muted_foreground, dim))
            .border(px(t::BORDER_WIDTH))
            .border_color(gpui::transparent_black());
        if let Some(edge) = control_look::border(LookKind::Chip, State::Rest, None, colors) {
            chip = chip.border_color(paint(edge, dim));
        }
        if let Some(fill) = control_look::fill(LookKind::Chip, State::Rest, false, None, colors) {
            chip = chip.bg(paint(fill, dim));
        }
        chip
    }

    /// The text's lines in the terminal font, coloured by bash role, with the selection, the caret (in the focused
    /// pane) and the ghost text after the last line.
    fn text_lines(
        &self,
        view: &View,
        focused: bool,
        pane: &Entity<TerminalPane>,
        cx: &App,
    ) -> AnyElement {
        let first = view.caret.0.saturating_sub(MAX_LINES - 1);
        let last_line = view.lines.len().saturating_sub(1);
        // A click places the caret at the cell under it (Shift extends the selection); the text's box is measured
        // as it is drawn.
        let cell = pane.read(cx).cell_size();
        // A line longer than the input scrolls sideways so the caret stays in view: every line starts at the same
        // column, the one that puts the caret in the last visible cell.
        let columns =
            pane.read(cx)
                .width()
                .zip(cell)
                .map_or(usize::MAX, |(width, (cell_width, _))| {
                    let text = width - 2.0 * t::PANE_PADDING - t::SPACE_8 - cell_width;
                    ((text / cell_width).floor() as usize).max(1)
                });
        let left = view.caret.1.saturating_sub(columns - 1);
        let measured: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::default();
        let keep = Rc::clone(&measured);
        // The text's line and column under a point, from the box measured as the text was drawn.
        let place = Rc::new(move |position: gpui::Point<Pixels>| {
            let (bounds, (width, height)) = (measured.get()?, cell?);
            let at = position - bounds.origin;
            let line = first + (f32::from(at.y) / height).max(0.0) as usize;
            let col = left + (f32::from(at.x) / width).round().max(0.0) as usize;
            Some((line, col))
        });
        let (press_pane, drag_pane) = (pane.clone(), pane.clone());
        let drag_place = Rc::clone(&place);
        let mut lines = div()
            .relative()
            .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                if let Some((line, col)) = place(event.position) {
                    let extend = event.modifiers.shift;
                    press_pane.update(cx, |pane, cx| pane.input_press(line, col, extend, cx));
                }
            })
            // Dragging with the button held selects, as in today's input.
            .on_mouse_move(move |event, _, cx| {
                let held = event.pressed_button == Some(MouseButton::Left);
                match drag_place(event.position) {
                    Some((line, col)) if held => {
                        drag_pane.update(cx, |pane, cx| pane.input_drag(line, col, cx));
                    }
                    _ => {}
                }
            })
            .child(
                canvas(move |bounds, _, _| keep.set(Some(bounds)), |_, (), _, _| {})
                    .absolute()
                    .size_full(),
            )
            .flex()
            .flex_col()
            .font_family(self.text.family.clone())
            .text_size(px(self.text.size))
            .whitespace_nowrap();
        for (index, line) in view.lines.iter().enumerate().skip(first).take(MAX_LINES) {
            let caret = (focused && view.caret.0 == index).then_some(view.caret.1 - left);
            let ghost = (index == last_line).then_some(view.ghost.as_str());
            lines = lines.child(self.text_line(line.get(left..).unwrap_or_default(), caret, ghost));
        }
        lines.into_any_element()
    }

    /// One line of the input as a single shaped text, so every character sits on the terminal font's cell grid:
    /// bash colours, the selection, the caret as a block over its character, and the ghost text after the line,
    /// whose first character the caret covers when it rests at the end (as in today's input).
    fn text_line(
        &self,
        line: &[(char, Tok, bool)],
        caret: Option<usize>,
        ghost: Option<&str>,
    ) -> AnyElement {
        let caret_look = HighlightStyle {
            color: Some(colour(self.palette.background)),
            background_color: Some(colour(self.palette.foreground)),
            ..HighlightStyle::default()
        };
        let mut text = String::new();
        let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
        let mut push = |text: &mut String, ch: char, look: HighlightStyle| {
            let at = text.len();
            text.push(ch);
            match highlights.last_mut() {
                Some((range, last)) if range.end == at && *last == look => range.end = text.len(),
                _ => highlights.push((at..text.len(), look)),
            }
        };
        for (col, (ch, tok, selected)) in line.iter().enumerate() {
            let shown = if ch.is_control() { ' ' } else { *ch };
            let look = if caret == Some(col) {
                caret_look
            } else {
                HighlightStyle {
                    color: Some(self.token_colour(*tok)),
                    background_color: selected.then(|| colour(self.palette.selection)),
                    ..HighlightStyle::default()
                }
            };
            push(&mut text, shown, look);
        }
        let ghost_look = HighlightStyle {
            color: Some(paint(self.colors.muted_foreground, 1.0)),
            ..HighlightStyle::default()
        };
        let mut ghost = ghost.unwrap_or_default().chars();
        if caret.is_some_and(|col| col >= line.len()) {
            push(&mut text, ghost.next().unwrap_or(' '), caret_look);
        }
        for ch in ghost {
            push(&mut text, ch, ghost_look);
        }
        div()
            .flex()
            .h(px(self.line_height()))
            .items_center()
            .child(StyledText::new(text).with_highlights(highlights))
            .into_any_element()
    }

    fn token_colour(&self, tok: Tok) -> Hsla {
        let ansi = &self.palette.ansi;
        match tok {
            Tok::Plain => colour(self.palette.foreground),
            Tok::Function => colour(ansi[4]),
            Tok::Str => colour(ansi[2]),
            Tok::Property => colour(ansi[6]),
            Tok::Constant | Tok::Keyword => colour(ansi[5]),
            Tok::Number => colour(ansi[1]),
            Tok::Comment | Tok::Operator => paint(self.colors.muted_foreground, 1.0),
        }
    }
}

/// A terminal colour at `alpha` opacity.
fn paint_rgb(rgb: [u8; 3], alpha: f32) -> Hsla {
    let mut colour = colour(rgb);
    colour.alpha *= alpha;
    colour
}
