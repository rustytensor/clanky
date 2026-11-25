//! The command palette: a search line, the commands it finds, and a footer, centred near the top of the window. It
//! opens with Ctrl+Shift+P or a click on the title bar's search box and lists what clanky-state's palette model finds
//! for what is typed (`clanky_state::command_palette`, which holds the matching rule). The arrows, Home and End move
//! the selection, Enter or a click runs the command, and Esc closes it; either way the keyboard goes back to the pane
//! that had it, which is also where a typed command runs.

use clanky_core::layout::Axis;
use clanky_state::{
    command_palette::{Command, Context as PaletteContext, Entry, Query, entries, scroll_to_show},
    keys::{Action, Key, action_for, chords_for},
    list_selection::ListKey,
};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Styled, Window, div, px, svg,
};

use super::{MainWindow, SCREEN as MAIN_SCREEN, chord_of};
use crate::{
    control_look::paint,
    controls::{Control, Group, Kind},
    icons::Icon,
    popover::{Area, Opener, Placement},
    tokens as t,
};

/// The palette's name in the control record.
const SCREEN: &str = "palette";
/// The title bar's search box, a control of the main window (3 is the Settings button beside it).
pub(super) const SEARCH_BOX_ID: u32 = 4;
/// The rows' control ids: this plus the row's place in the list.
const FIRST_ROW_ID: u32 = 100;
const ROWS: Group = Group::vertical("palette-rows");
const PLACEHOLDER: &str = "Type a command or search\u{2026}";
const SEARCH_BOX_LABEL: &str = "Search or run a command";
const FOOTER: &str = "Enter runs \u{b7} Esc closes";
const NOTHING_FOUND: &str = "No matching commands.";
/// The pane the palette opened over, as a "Run" row names it.
const PANE_NAME: &str = "this pane";

/// What the open palette keeps: what was typed, the pane it opened over, and the first row in view.
#[derive(Default)]
pub(super) struct PaletteState {
    query: Query,
    pane: Option<String>,
    first: usize,
}

/// A command's icon, by what it does.
fn icon_of(command: &Command) -> Icon {
    match command {
        Command::NewTab => Icon::Plus,
        Command::SplitRight => Icon::SplitRight,
        Command::SplitDown => Icon::SplitDown,
        Command::ClosePane => Icon::Close,
        Command::ToggleToolsPanel | Command::ToggleSideTabs => Icon::Sidebar,
        Command::OpenSettings => Icon::Settings,
        Command::GoToTab(_) | Command::Run(_) => Icon::Terminal,
    }
}

impl MainWindow {
    /// Ctrl+Shift+P opens the palette, or closes it when it is open. True when the key was that one.
    pub(super) fn palette_key(
        &mut self,
        chord: clanky_state::keys::Chord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if action_for(chord) != Some(Action::ToggleCommandPalette) {
            return false;
        }
        if self.popover.screen() == Some(SCREEN) {
            self.close_popover(window, cx);
        } else {
            self.open_palette(window, cx);
        }
        true
    }

    /// The title bar's search box: a field that opens the palette when clicked.
    pub(super) fn search_box(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity();
        let mut field = Control::new(Kind::Field, SEARCH_BOX_ID, SEARCH_BOX_LABEL)
            .icon(Icon::Search)
            .width(t::SEARCH_BOX_WIDTH)
            .on_click(move |window, cx| {
                this.update(cx, |this, cx| this.open_palette_from_box(window, cx));
            });
        if let Some(chord) = chords_for(Action::ToggleCommandPalette).next() {
            field = field.keys(chord.label());
        }
        self.kit.build(MAIN_SCREEN, field, window, cx)
    }

    /// A click on the search box: the press gave the box the keyboard, but the box only stands for the palette, so
    /// the pane gets it back first and the palette returns it there when it closes.
    fn open_palette_from_box(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pane = self
            .focused_pane(window, cx)
            .and_then(|id| self.panes.get(&id))
            .map(|pane| pane.0.read(cx).focus_handle().clone());
        if let Some(handle) = pane {
            window.focus(&handle, cx);
        }
        self.open_palette(window, cx);
    }

    fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pane = self.focused_pane(window, cx);
        let area = self
            .kit
            .bounds_of(MAIN_SCREEN, SEARCH_BOX_ID)
            .map_or_else(Area::default, Area::of);
        let opener = Opener {
            area,
            control: None,
            pane: pane.clone(),
        };
        self.palette_state = PaletteState {
            pane,
            ..PaletteState::default()
        };
        self.popover.open(
            SCREEN,
            opener,
            Placement::Centred {
                top: t::PALETTE_TOP,
            },
            window,
            cx,
        );
        self.popover.restart_list(self.palette_entries().len());
        cx.notify();
    }

    fn palette_entries(&self) -> Vec<Entry> {
        let context = PaletteContext {
            tabs: (0..self.workspace.groups.len())
                .map(|index| (self.tab_title(index), index == self.workspace.active))
                .collect(),
            pane: self
                .palette_state
                .pane
                .as_ref()
                .map(|_| PANE_NAME.to_owned()),
            side_tabs: self.vertical(),
        };
        let mut found = entries(&context, self.palette_state.query.text());
        if self.kit.record().faults().palette_no_run() {
            found.retain(|entry| !matches!(entry.command, Command::Run(_)));
        }
        found
    }

    /// A key while the palette's own line has the keyboard: the list's keys, then typing and Backspace, which filter
    /// the list and select its first row again. True when the key was taken.
    fn palette_typed(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.popover.holds_keyboard(window) {
            return false;
        }
        let stroke = &event.keystroke;
        if let Some(chord) = chord_of(stroke) {
            match self.popover.list_key(chord) {
                ListKey::Moved => {
                    cx.notify();
                    return true;
                }
                ListKey::Pick(index) => {
                    self.pick_palette(index, window, cx);
                    return true;
                }
                ListKey::Ignored => {}
            }
        }
        let modifiers = &stroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform {
            return false;
        }
        let changed = if chord_of(stroke).is_some_and(|chord| chord.key == Key::Backspace) {
            self.palette_state.query.backspace()
        } else {
            stroke
                .key_char
                .as_deref()
                .is_some_and(|typed| self.palette_state.query.insert(typed))
        };
        if changed {
            self.palette_state.first = 0;
            self.popover.restart_list(self.palette_entries().len());
            cx.notify();
        }
        changed
    }

    /// Runs the row at `index` of the list, after closing the palette, which gives the pane back its keyboard.
    fn pick_palette(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.palette_entries().into_iter().nth(index) else {
            return;
        };
        let pane = self.palette_state.pane.take();
        self.close_popover(window, cx);
        match entry.command {
            Command::NewTab => self.act(window, cx, Self::new_tab),
            Command::SplitRight => self.act(window, cx, |this, window, cx| {
                this.split(Axis::Columns, window, cx)
            }),
            Command::SplitDown => self.act(window, cx, |this, window, cx| {
                this.split(Axis::Rows, window, cx)
            }),
            Command::ClosePane => self.act(window, cx, Self::close_pane),
            Command::ToggleToolsPanel => self.toggle_tools(cx),
            Command::ToggleSideTabs => self.toggle_side_tabs(cx),
            Command::OpenSettings => self.open_settings(window, cx),
            Command::GoToTab(tab) => self.act(window, cx, move |this, _, _| this.select_tab(tab)),
            Command::Run(text) => self.act(window, cx, move |this, _, _| {
                let pane = pane.ok_or("no pane had the keyboard to run the command in")?;
                this.client
                    .input(&pane, format!("{text}\r").as_bytes())
                    .map_err(|e| format!("running the command in the pane: {}", e.message))
            }),
        }
    }

    /// How many rows the palette shows: up to `PALETTE_VISIBLE_ROWS`, fewer when the window has less room below the
    /// palette's top for its line, footer and padding.
    fn palette_room(window: &Window) -> usize {
        let height = f32::from(window.viewport_size().height);
        let fixed = t::PALETTE_TOP
            + t::POPOVER_WINDOW_MARGIN
            + t::PALETTE_INPUT_HEIGHT
            + t::STATUS_HEIGHT
            + 2.0 * t::POPOVER_PADDING;
        let fits = ((height - fixed) / t::PALETTE_ROW_HEIGHT).floor().max(1.0) as usize;
        fits.min(t::PALETTE_VISIBLE_ROWS)
    }

    /// The open palette, drawn in the window's popover.
    pub(super) fn command_palette(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let found = self.palette_entries();
        let holds = self.popover.holds_keyboard(window);
        let room = Self::palette_room(window);
        let first = scroll_to_show(
            self.palette_state.first,
            self.popover.selected_row(),
            found.len(),
            room,
        );
        self.palette_state.first = first;
        let selected = self.popover.selected_row().filter(|_| holds);
        let mut rows = div().flex().flex_col().p(px(t::POPOVER_PADDING));
        for (index, entry) in found.iter().enumerate().skip(first).take(room) {
            let this = cx.entity();
            let mut row = Control::new(
                Kind::PaletteRow,
                FIRST_ROW_ID + index as u32,
                entry.label.clone(),
            )
            .icon(icon_of(&entry.command))
            .group(ROWS)
            .checked(selected == Some(index))
            .on_click(move |window, cx| {
                this.update(cx, |this, cx| this.pick_palette(index, window, cx));
            });
            if let Some(keys) = &entry.keys {
                row = row.keys(keys.clone());
            }
            // The pointer selects the row it moves over, so one row is lit at a time, as in today's palette.
            let pointer = cx.entity();
            rows = rows.child(
                div()
                    .w_full()
                    .on_mouse_move(move |_, _, cx| {
                        pointer.update(cx, |this, cx| {
                            if this.popover.selected_row() != Some(index) {
                                this.popover.select_row(index);
                                cx.notify();
                            }
                        });
                    })
                    .child(self.kit.build(SCREEN, row, window, cx)),
            );
        }
        if found.is_empty() {
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(t::PALETTE_ROW_HEIGHT))
                    .text_size(px(t::TEXT_TITLE))
                    .text_color(paint(self.colors.muted_foreground, 1.0))
                    .child(NOTHING_FOUND),
            );
        }
        let hair = t::hairline(window.scale_factor());
        let content = div()
            .flex()
            .flex_col()
            .w(px(t::PALETTE_WIDTH))
            .max_w(window.viewport_size().width - px(2.0 * t::POPOVER_WINDOW_MARGIN))
            .child(self.palette_line(holds, hair))
            .child(rows)
            .child(self.palette_footer(hair));
        let keys = cx.entity();
        let closer = cx.entity();
        self.popover.element(
            &self.kit,
            &self.colors,
            content,
            move |event, window, cx| {
                keys.update(cx, |this, cx| {
                    if this.palette_typed(event, window, cx) {
                        cx.stop_propagation();
                    }
                });
            },
            move |window, cx| closer.update(cx, |this, cx| this.close_popover(window, cx)),
            window,
        )
    }

    /// The search line: the icon, what was typed (or the placeholder) and the caret while the line has the keyboard.
    /// Its bottom edge is the line between it and the rows, inside its `PALETTE_INPUT_HEIGHT`.
    fn palette_line(&self, holds: bool, hair: f32) -> AnyElement {
        let colors = &self.colors;
        let typed = self.palette_state.query.text();
        let line = t::PALETTE_INPUT_HEIGHT - hair;
        let (text, ink) = if typed.is_empty() {
            (PLACEHOLDER.to_owned(), colors.muted_foreground)
        } else {
            (typed.to_owned(), colors.foreground)
        };
        // The text fills the line's height, so the landmark the design gate measures it in is the whole line.
        let text = div()
            .relative()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .line_height(px(line))
            .text_size(px(t::TEXT_TITLE))
            .font_weight(FontWeight(f32::from(t::WEIGHT_REGULAR)))
            .text_color(paint(ink, 1.0))
            .child(text)
            .child(self.kit.record().landmark("popover-input", 0));
        let caret = holds.then(|| {
            div()
                .flex_none()
                .w(px(t::CARET_WIDTH))
                .h(px(t::PALETTE_CARET_HEIGHT))
                .bg(paint(colors.foreground, 1.0))
        });
        let mut row = div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(line))
            .px(px(t::SPACE_16))
            .gap(px(t::SPACE_12))
            .border_b(px(hair))
            .border_color(paint(colors.border, 1.0))
            .child(
                svg()
                    .flex_none()
                    .data(Icon::Search.svg())
                    .size(px(t::ICON_SIZE_ROW))
                    .text_color(paint(colors.muted_foreground, 1.0)),
            );
        // The caret stands where the next letter goes: before the placeholder, after typed text.
        let words = div()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(px(t::SPACE_2));
        row = if typed.is_empty() {
            row.child(words.children(caret).child(text))
        } else {
            row.child(words.child(text).children(caret))
        };
        row.into_any_element()
    }

    /// The footer: how to run and close, in the status text. Its top edge is the line above it.
    fn palette_footer(&self, hair: f32) -> AnyElement {
        div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(px(t::STATUS_HEIGHT - hair))
            .px(px(t::SPACE_12))
            .border_t(px(hair))
            .border_color(paint(self.colors.border, 1.0))
            .text_size(px(t::TEXT_STATUS))
            .text_color(paint(self.colors.muted_foreground, 1.0))
            .child(FOOTER)
            .child(self.kit.record().landmark("popover-footer", 0))
            .into_any_element()
    }
}
