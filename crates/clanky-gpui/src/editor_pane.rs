//! An editor pane: a file open beside a terminal pane, edited with today's editor core (`clanky::editor`, which owns
//! the `clanky_edit::Document`). The core draws the buffer into a cell grid, as it does for today's window: line
//! numbers, token colours from the window theme, the caret's line and the selection. This pane draws that grid with
//! the terminal's `GridElement`, and under it a bar with the file's name, the pending-change marker, the core's hint
//! or error, and Save and Close.
//!
//! Closing with pending changes is refused, as today ("save or undo before closing"); nothing is lost.

use std::{cell::Cell as Shared, ops::Range, path::Path, rc::Rc, sync::Arc};

use clanky::editor::{EditorPane, NORMAL, SELECTED, SYNTAX_SOURCE, SyntaxColors};
use clanky_core::{
    Cell, CellColorSource, ColorSource, STYLE_ITALIC, STYLE_TERMINAL_ITALIC, TerminalSnapshot,
    layout::Rect,
};
use clanky_state::themes::{Color, Colors, Theme};
use gpui::{
    AnyElement, App, Bounds, Context, EntityInputHandler, EventEmitter, FocusHandle,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    ParentElement, Pixels, Render, ScrollDelta, ScrollWheelEvent, Styled, UTF16Selection, Window,
    div, px,
};
use serde_json::json;
use winit::keyboard::{Key, ModifiersState};

use crate::{
    control_look::paint,
    control_record::Faults,
    controls::{Control, Kind, Kit},
    grid_element::{GridElement, GridOwner, Metrics, TextSettings},
    icons::Icon,
    keys,
    palette::Palette,
    pane::SharedTrace,
    tokens as t,
};

/// The screen the bar's controls are recorded under: they are the main window's, like a terminal pane's chips.
const SCREEN: &str = "main-window";
/// The grid's size before the pane has been laid out.
const FIRST_SIZE: (usize, usize) = (24, 80);
/// The core's grid has a title row above the text and a status row below it; this window shows neither as cells.
const CORE_ROWS_AROUND_TEXT: usize = 2;
/// Lines the view scrolls per wheel notch, as the terminal's history does.
const WHEEL_LINES: f32 = 3.0;
/// gpui-ce reports one wheel notch on X11 and Wayland as this many lines.
const GPUI_LINES_PER_NOTCH: f32 = 3.0;
/// The core's own words when it has nothing else to say in its status row.
const CORE_IDLE_HINT: &str = "Ctrl+S save";
/// Control ids: each editor pane takes two (Save, Close) from here, by its slot.
const CONTROL_IDS: u32 = 8000;
const CONTROLS_EACH: u32 = 2;
/// Today's colour formulas (studio.rs `syntax_colors`): the share of the way from one colour to the other.
const NUMBER_MIX: f32 = 0.6;
const COMMENT_MIX: f32 = 0.51;
const GUTTER_MIX: f32 = 0.39;
const CARET_LINE_MIX: f32 = 0.035;

/// What an editor pane tells the window.
pub enum EditorEvent {
    /// The pane may close: it has no pending changes (or Close was forced by a planted fault).
    Closed,
}

/// The editor's token colours from the window theme (never the terminal's colours), as today's window has them:
/// keyword ANSI 5, function 4, type 3, string 2, number 40 % ANSI 1 with 60 % ANSI 3, constant and macro 6, comment
/// 51 % of the way from the background to `muted_foreground`, the gutter 39 % and the caret's line 3.5 % of that way.
pub fn syntax_colors(theme: &Theme) -> SyntaxColors {
    let rgb = |color: Color| [color[0], color[1], color[2]];
    let background = rgb(theme.colors.background);
    let muted = rgb(theme.colors.muted_foreground);
    SyntaxColors {
        keyword: rgb(theme.ansi[5]),
        function: rgb(theme.ansi[4]),
        type_: rgb(theme.ansi[3]),
        string: rgb(theme.ansi[2]),
        number: mix(rgb(theme.ansi[1]), rgb(theme.ansi[3]), NUMBER_MIX),
        constant: rgb(theme.ansi[6]),
        comment: mix(background, muted, COMMENT_MIX),
        gutter: mix_rounded(background, muted, GUTTER_MIX),
        current_line: mix(background, muted, CARET_LINE_MIX),
    }
}

/// `from` moved `share` of the way to `to`, each channel rounded half up (today's `mix`).
fn mix(from: [u8; 3], to: [u8; 3], share: f32) -> [u8; 3] {
    std::array::from_fn(|i| {
        let value = f32::from(from[i]) + share * (f32::from(to[i]) - f32::from(from[i]));
        // The value lies between two channel values, so it fits a channel.
        (value + 0.5).floor().clamp(0.0, 255.0) as u8
    })
}

/// The same with Rust's rounding (half away from zero), which today's gutter colour uses.
fn mix_rounded(from: [u8; 3], to: [u8; 3], share: f32) -> [u8; 3] {
    std::array::from_fn(|i| {
        let value = f32::from(from[i]) + share * (f32::from(to[i]) - f32::from(from[i]));
        value.round().clamp(0.0, 255.0) as u8
    })
}

/// A file's path as the bar shows it: under the home folder as `~/…`.
fn shown_path(path: &Path) -> String {
    let home = clanky_core::platform::home_dir();
    match home
        .as_deref()
        .and_then(|home| path.strip_prefix(home).ok())
    {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

pub struct EditorSetup {
    /// The file, opened by the editor core.
    pub editor: EditorPane,
    pub palette: Palette,
    pub text: TextSettings,
    pub syntax: SyntaxColors,
    pub colors: Colors,
    pub kit: Kit,
    pub trace: SharedTrace,
    /// The pane's place among the window's editor panes: its landmarks' index and its controls' ids.
    pub slot: u32,
}

pub struct EditorView {
    editor: EditorPane,
    palette: Palette,
    text: TextSettings,
    syntax: SyntaxColors,
    colors: Colors,
    kit: Kit,
    trace: SharedTrace,
    faults: Faults,
    slot: u32,
    focus: FocusHandle,
    metrics: Rc<Shared<Metrics>>,
    /// The grid's rows and columns of text, as last laid out.
    size: (usize, usize),
    /// The last failure (a save that could not write, a refused close), shown in the bar until the next key.
    notice: Option<String>,
    /// Whether the marker has shown since the pane opened; only the `marker-stays` fault reads it.
    marker_shown: bool,
}

impl EventEmitter<EditorEvent> for EditorView {}

impl EditorView {
    pub fn new(setup: EditorSetup, cx: &mut Context<Self>) -> Self {
        let faults = setup.kit.record().faults();
        let mut syntax = setup.syntax;
        if faults.syntax_off_by_one() {
            syntax.keyword = syntax.constant;
        }
        Self {
            editor: setup.editor,
            palette: setup.palette,
            text: setup.text,
            syntax,
            colors: setup.colors,
            kit: setup.kit,
            trace: setup.trace,
            faults,
            slot: setup.slot,
            focus: cx.focus_handle(),
            metrics: Rc::default(),
            size: FIRST_SIZE,
            notice: None,
            marker_shown: false,
        }
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// Whether the buffer differs from the file as last saved.
    fn pending(&self) -> bool {
        self.editor.dirty() || (self.faults.marker_stays() && self.marker_shown)
    }

    /// Closes the pane unless it has pending changes, which are kept and named in the bar instead.
    pub fn request_close(&mut self, cx: &mut Context<Self>) {
        if self.editor.dirty() && !self.faults.close_drops_pending() {
            self.notice = Some("Unsaved file: save or undo before closing".into());
            cx.notify();
            return;
        }
        cx.emit(EditorEvent::Closed);
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if !self.faults.save_not_written() {
            self.notice = self
                .editor
                .command("save", &json!({}))
                .err()
                .map(|e| format!("saving {}: {e}", self.editor.path().display()));
        }
        cx.notify();
    }

    /// The whole rectangle the core draws in: its title row, the text rows and its status row.
    fn core_rect(&self) -> Rect {
        Rect {
            col: 0,
            row: 0,
            cols: self.size.1,
            rows: self.size.0 + CORE_ROWS_AROUND_TEXT,
        }
    }

    /// The core's drawing of the buffer, as the grid shows it (text rows only), and its status row's words.
    fn draw_grid(&mut self, focused: bool) -> (TerminalSnapshot, String) {
        let rect = self.core_rect();
        let blank = Cell {
            ch: ' ',
            fg: self.palette.foreground,
            bg: NORMAL,
            color_source: None,
            style: 0,
        };
        let mut frame = TerminalSnapshot {
            rows: rect.rows,
            cols: rect.cols,
            cells: vec![blank; rect.rows * rect.cols],
            ..TerminalSnapshot::default()
        };
        self.editor
            .draw_with(&mut frame, &rect, focused, Some(&self.syntax));
        let cols = frame.cols;
        let status: String = frame.cells[(rect.rows - 1) * cols..]
            .iter()
            .map(|cell| cell.ch)
            .collect();
        let cells = frame.cells[cols..(rect.rows - 1) * cols]
            .iter()
            .map(|cell| self.recolour(cell))
            .collect();
        let grid = TerminalSnapshot {
            rows: self.size.0,
            cols,
            cells,
            cursor: frame
                .cursor
                .and_then(|(row, col)| row.checked_sub(1).map(|row| (row, col))),
            ..TerminalSnapshot::default()
        };
        (grid, status.trim().to_owned())
    }

    /// One of the core's cells in the window's colours: its plain ink and background become the pane's defaults, its
    /// selection the palette's selection, and token colours stay as drawn. Comments are drawn in the italic face.
    fn recolour(&self, cell: &Cell) -> Cell {
        let token = cell.color_source == Some(SYNTAX_SOURCE);
        let (bg, bg_source) = match cell.bg {
            NORMAL => (self.palette.background, ColorSource::Default),
            SELECTED => (self.palette.selection, ColorSource::Truecolor),
            other => (other, ColorSource::Truecolor),
        };
        let (fg, fg_source) = if token {
            (cell.fg, ColorSource::Truecolor)
        } else {
            (self.palette.foreground, ColorSource::Default)
        };
        let style = if cell.style & STYLE_ITALIC == 0 {
            cell.style
        } else {
            (cell.style & !STYLE_ITALIC) | STYLE_TERMINAL_ITALIC
        };
        Cell {
            ch: cell.ch,
            fg,
            bg,
            color_source: Some(CellColorSource {
                fg: fg_source,
                bg: bg_source,
                inverse: false,
            }),
            style,
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let m = &keystroke.modifiers;
        let key = match keys::named(&keystroke.key) {
            Some(named) => Key::Named(named),
            // Plain text comes through the input method (`replace_text_in_range`), composed characters included.
            None if !m.control && !m.alt => return,
            None => Key::Character(keystroke.key.as_str().into()),
        };
        let mut state = ModifiersState::empty();
        state.set(ModifiersState::SHIFT, m.shift);
        state.set(ModifiersState::ALT, m.alt);
        state.set(ModifiersState::CONTROL, m.control);
        self.trace.borrow_mut().key_event();
        self.notice = None;
        let save = m.control && !m.alt && !m.shift && keystroke.key == "s";
        if save {
            self.save(cx);
        } else if let Err(error) =
            self.editor
                .key(&key, keystroke.key_char.as_deref(), state, false)
        {
            self.notice = Some(error);
        }
        self.marker_shown |= self.editor.dirty();
        cx.stop_propagation();
        cx.notify();
    }

    /// The core's column and row (in its own grid, title row included) under a point in the window.
    fn core_cell(&self, position: gpui::Point<Pixels>) -> (usize, usize) {
        let (row, col, _) = self.metrics.get().cell_at(position.x, position.y);
        (col, row + 1)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        let (col, row) = self.core_cell(event.position);
        let rect = self.core_rect();
        if let Err(error) = self.editor.click(&rect, col, row, event.modifiers.shift) {
            self.notice = Some(error);
        }
        cx.notify();
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let (col, row) = self.core_cell(event.position);
        let rect = self.core_rect();
        if self.editor.drag(&rect, col, row).is_ok() {
            cx.notify();
        }
    }

    fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let metrics = self.metrics.get();
        let notches = match event.delta {
            ScrollDelta::Lines(delta) => delta.y / GPUI_LINES_PER_NOTCH,
            ScrollDelta::Pixels(delta) => {
                f32::from(delta.y) * metrics.scale / metrics.line_height.max(1.0) / WHEEL_LINES
            }
        };
        // The wheel scrolls whole lines; a touchpad's fraction of a line waits for the rest.
        let lines = (-notches * WHEEL_LINES).round() as i64;
        if lines != 0
            && self
                .editor
                .command("scroll", &json!({"lines": lines}))
                .is_ok()
        {
            cx.notify();
        }
    }

    /// The bar under the text: the file, the pending-change marker, the core's hint, then Save and Close.
    fn bar(&self, status: &str, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pending = self.pending();
        let hint = if let Some(notice) = &self.notice {
            notice.clone()
        } else if status.starts_with(CORE_IDLE_HINT) || status.is_empty() {
            "Ctrl+S saves · Ctrl+F finds".to_owned()
        } else {
            status.replace(" | ", " · ")
        };
        let marker = if pending {
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(t::SPACE_4))
                .text_size(px(t::TEXT_STATUS))
                .text_color(paint(self.colors.muted_foreground, 1.0))
                .child(
                    div()
                        .flex_none()
                        .size(px(t::SPACE_6))
                        .rounded_full()
                        .bg(paint(self.colors.foreground, 1.0)),
                )
                .child("unsaved")
        } else {
            div().flex_none().w(px(0.0)).overflow_hidden()
        };
        let ids = CONTROL_IDS + CONTROLS_EACH * self.slot;
        let saver = cx.entity();
        let save = Control::new(Kind::ButtonBar, ids, "Save")
            .on_click(move |_, cx| saver.update(cx, |this, cx| this.save(cx)));
        let closer = cx.entity();
        let name = shown_path(self.editor.path());
        let close = Control::new(Kind::IconButton, ids + 1, format!("Close {name}"))
            .icon(Icon::Close)
            .on_click(move |_, cx| closer.update(cx, Self::request_close));
        div()
            .relative()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(t::SPACE_8))
            .px(px(t::PANE_PADDING))
            .pt(px(t::SPACE_8))
            .pb(px(t::PANE_PADDING))
            .border_t(px(t::hairline(window.scale_factor())))
            .border_color(paint(self.colors.border, 1.0))
            .text_size(px(t::TEXT_UI))
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(name),
            )
            .child(
                div()
                    .relative()
                    .flex_none()
                    .child(marker)
                    .child(self.kit.record().landmark("editor-unsaved", self.slot)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(t::TEXT_STATUS))
                    .text_color(paint(
                        if self.notice.is_some() {
                            self.colors.err
                        } else {
                            self.colors.muted_foreground
                        },
                        1.0,
                    ))
                    .child(hint),
            )
            .child(self.kit.build(SCREEN, save, window, cx))
            .child(self.kit.build(SCREEN, close, window, cx))
            .into_any_element()
    }
}

impl GridOwner for EditorView {
    fn fit(&mut self, rows: usize, cols: usize, cx: &mut Context<Self>) {
        if self.size != (rows, cols) {
            self.size = (rows, cols);
            cx.notify();
        }
    }
}

impl Render for EditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus.is_focused(window);
        let (grid, status) = self.draw_grid(focused);
        let element = GridElement {
            pane: cx.entity(),
            snapshot: Some(Arc::new(grid)),
            palette: self.palette.clone(),
            text: self.text.clone(),
            selection: Vec::new(),
            found: Vec::new(),
            focus: self.focus.clone(),
            metrics: self.metrics.clone(),
            show_cursor: true,
            tops: None,
        };
        let bar = self.bar(&status, window, cx);
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .key_context("Editor")
            .on_key_down(cx.listener(Self::on_key_down))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    .on_scroll_wheel(cx.listener(Self::on_scroll))
                    .child(element),
            )
            .child(bar)
    }
}

impl EntityInputHandler for EditorView {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Committed text from a key or the input method: the core inserts it (or adds it to its find prompt).
        self.trace.borrow_mut().key_event();
        self.notice = self.editor.insert_text(text).err();
        self.marker_shown |= self.editor.dirty();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        _new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

impl gpui::Focusable for EditorView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphite_dark_gives_todays_editor_colours() {
        // Worked out from assets/themes/graphite-dark.json (background #141414, muted #a3a3a3, ANSI 1 #f0716a,
        // ANSI 3 #e2b64f) with today's formulas, in Python: comment (93, 93, 93), number (232, 154, 90).
        let theme = clanky_state::themes::builtin("graphite-dark").expect("a bundled theme");
        let colors = syntax_colors(&theme);
        assert_eq!(colors.comment, [93, 93, 93]);
        assert_eq!(colors.number, [232, 154, 90]);
    }

    #[test]
    fn home_paths_are_shortened() {
        let home = clanky_core::platform::home_dir().expect("the test runs with a home folder");
        assert_eq!(shown_path(&home.join("x/y.rs")), "~/x/y.rs");
        assert_eq!(shown_path(Path::new("/etc/hosts")), "/etc/hosts");
    }
}
