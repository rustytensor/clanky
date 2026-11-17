//! One terminal pane: the latest snapshot from the session server, and every input that goes back to it
//! (keys, input-method text, paste, mouse reports, wheel, selection). The grid itself is drawn by `GridElement`.

use std::{cell::Cell as Shared, ops::Range, rc::Rc, sync::Arc, time::Instant};

use clanky::mouse::{self, Clicks, Kind, Mods, Pos, Selection};
use clanky_api::Client;
use clanky_core::{MODE_BRACKETED_PASTE, TerminalSnapshot};
use gpui::{
    App, Bounds, ClipboardItem, Context, EntityInputHandler, FocusHandle, InteractiveElement,
    IntoElement, KeyDownEvent, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Pixels, Render, ScrollDelta, ScrollWheelEvent, Styled, Task,
    UTF16Selection, Window, div, point, px, size,
};

use crate::{
    grid_element::{GridElement, Metrics, TextSettings, colour},
    keys,
    link::{Link, Update},
    palette::Palette,
};

/// Lines the history scrolls per wheel notch, as VTE.
const WHEEL_LINES: f32 = 3.0;

pub struct TerminalPane {
    client: Client,
    tab: String,
    snapshot: Option<Arc<TerminalSnapshot>>,
    palette: Palette,
    text: TextSettings,
    focus: FocusHandle,
    metrics: Rc<Shared<Metrics>>,
    /// The size last sent to the server, so a resize is sent once per change.
    size_sent: (usize, usize),
    selection: Option<Selection>,
    selecting: bool,
    clicks: Clicks,
    /// The button held while the program receives mouse reports, and the cell last reported.
    reporting_button: Option<(u8, (usize, usize))>,
    wheel_rest: f32,
    /// Text the input method is composing; it is shown nowhere yet and sent only when committed.
    marked: Option<String>,
    notice: Option<String>,
    _link: Link,
    _updates: Task<()>,
}

pub struct PaneSetup {
    pub client: Client,
    pub tab: String,
    pub palette: Palette,
    pub text: TextSettings,
    pub size_sent: (usize, usize),
    pub link: Link,
    pub updates: async_channel::Receiver<Update>,
}

impl TerminalPane {
    pub fn new(setup: PaneSetup, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let updates = setup.updates;
        let task = cx.spawn_in(window, async move |pane, cx| {
            while let Ok(update) = updates.recv().await {
                let finished = matches!(update, Update::Finished);
                let applied = pane.update(cx, |pane, cx| {
                    match update {
                        Update::Snapshot(snapshot) => pane.snapshot = Some(snapshot),
                        Update::Lost(message) => pane.notice = Some(message),
                        Update::Finished => cx.quit(),
                    }
                    cx.notify();
                });
                if applied.is_err() || finished {
                    break;
                }
            }
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            client: setup.client,
            tab: setup.tab,
            snapshot: None,
            palette: setup.palette,
            text: setup.text,
            focus,
            metrics: Rc::new(Shared::new(Metrics::default())),
            size_sent: setup.size_sent,
            selection: None,
            selecting: false,
            clicks: Clicks::default(),
            reporting_button: None,
            wheel_rest: 0.0,
            marked: None,
            notice: None,
            _link: setup.link,
            _updates: task,
        }
    }

    /// Called while laying out: tells the server the grid's size when it changed.
    pub fn fit(&mut self, rows: usize, cols: usize) {
        if self.size_sent == (rows, cols) {
            return;
        }
        self.size_sent = (rows, cols);
        if let Err(error) = self.client.resize(&self.tab, rows, cols) {
            self.notice = Some(format!("resizing the pane: {}", error.message));
        }
    }

    fn send(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        // Typing returns the view to the bottom and ends a selection, as in VTE.
        self.selection = None;
        for chunk in bytes.chunks(64 * 1024) {
            if let Err(error) = self.client.input(&self.tab, chunk) {
                self.notice = Some(format!("sending input: {}", error.message));
                return;
            }
        }
    }

    fn modes(&self) -> (u32, bool) {
        self.snapshot
            .as_ref()
            .map_or((0, false), |s| (s.modes, s.app_cursor))
    }

    fn paste(&mut self, text: &str) {
        let bracketed = self.modes().0 & MODE_BRACKETED_PASTE != 0;
        self.send(&mouse::pastify(text, bracketed));
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let m = &keystroke.modifiers;
        if m.control && m.shift && !m.alt {
            match keystroke.key.as_str() {
                "c" => {
                    if let (Some(selection), Some(snapshot)) = (&self.selection, &self.snapshot) {
                        cx.write_to_clipboard(ClipboardItem::new_string(selection.text(snapshot)));
                    }
                    cx.stop_propagation();
                    return;
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        self.paste(&text);
                    }
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                _ => {}
            }
        }
        if let Some(bytes) = keys::bytes_for(keystroke, self.modes().1) {
            self.send(&bytes);
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn top_line(&self) -> i64 {
        self.snapshot
            .as_ref()
            .map_or(0, |s| s.history as i64 - s.display_offset as i64)
    }

    fn pos_at(&self, x: Pixels, y: Pixels) -> (Pos, usize, usize) {
        let (row, col, right) = self.metrics.get().cell_at(x, y);
        (
            Pos {
                line: self.top_line() + row as i64,
                col,
                right,
            },
            row,
            col,
        )
    }

    fn mods(modifiers: Modifiers) -> Mods {
        Mods {
            shift: modifiers.shift,
            alt: modifiers.alt,
            ctrl: modifiers.control,
        }
    }

    /// Whether the program gets this mouse event (Shift keeps the mouse for selecting, as in VTE).
    fn reports(&self, modifiers: Modifiers) -> bool {
        mouse::reporting(self.modes().0) && !modifiers.shift
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        let (pos, row, col) = self.pos_at(event.position.x, event.position.y);
        let button = match event.button {
            MouseButton::Left => 1,
            MouseButton::Middle => 2,
            MouseButton::Right => 3,
            MouseButton::Navigate(_) => return,
        };
        if self.reports(event.modifiers) {
            if let Some(bytes) = mouse::report(
                self.modes().0,
                button,
                Self::mods(event.modifiers),
                col,
                row,
                false,
                false,
            ) {
                self.send(&bytes);
            }
            self.reporting_button = Some((button, (row, col)));
            return;
        }
        match button {
            1 => {
                let kind = match self.clicks.press(Instant::now(), pos.line, pos.col) {
                    2 => Kind::Word,
                    3 => Kind::Line,
                    _ => Kind::Char,
                };
                self.selection = Some(Selection::new(kind, event.modifiers.control, pos));
                self.selecting = true;
                cx.notify();
            }
            2 => {
                if let Some(text) = cx.read_from_primary().and_then(|item| item.text()) {
                    self.paste(&text);
                }
            }
            _ => {}
        }
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (pos, row, col) = self.pos_at(event.position.x, event.position.y);
        if let Some((button, last)) = self.reporting_button {
            let modes = self.modes().0;
            if last != (row, col) && mouse::reports_motion(modes, true) {
                if let Some(bytes) = mouse::report(
                    modes,
                    button,
                    Self::mods(event.modifiers),
                    col,
                    row,
                    true,
                    false,
                ) {
                    self.send(&bytes);
                }
                self.reporting_button = Some((button, (row, col)));
            }
            return;
        }
        if self.selecting
            && event.pressed_button == Some(MouseButton::Left)
            && let Some(selection) = &mut self.selection
        {
            selection.head = pos;
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let (_, row, col) = self.pos_at(event.position.x, event.position.y);
        if let Some((button, _)) = self.reporting_button.take() {
            if let Some(bytes) = mouse::report(
                self.modes().0,
                button,
                Self::mods(event.modifiers),
                col,
                row,
                false,
                true,
            ) {
                self.send(&bytes);
            }
            return;
        }
        if self.selecting {
            self.selecting = false;
            // A finished selection becomes the primary selection, as on every X11 terminal.
            let text = self
                .selection
                .as_ref()
                .zip(self.snapshot.as_ref())
                .map(|(s, snapshot)| s.text(snapshot));
            match text {
                Some(text) if !text.is_empty() => {
                    cx.write_to_primary(ClipboardItem::new_string(text));
                }
                _ => self.selection = None,
            }
            cx.notify();
        }
    }

    fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let m = self.metrics.get();
        let lines = match event.delta {
            ScrollDelta::Lines(delta) => delta.y,
            ScrollDelta::Pixels(delta) => {
                f32::from(delta.y) * m.scale / m.line_height.max(1.0) / WHEEL_LINES
            }
        };
        self.wheel_rest += lines;
        let notches = self.wheel_rest.trunc();
        self.wheel_rest -= notches;
        if notches == 0.0 {
            return;
        }
        let up = notches > 0.0;
        let (modes, app_cursor) = self.modes();
        let (_, row, col) = self.pos_at(event.position.x, event.position.y);
        for _ in 0..notches.abs() as usize {
            if self.reports(event.modifiers) {
                let button = if up { 4 } else { 5 };
                if let Some(bytes) = mouse::report(
                    modes,
                    button,
                    Self::mods(event.modifiers),
                    col,
                    row,
                    false,
                    false,
                ) {
                    self.send(&bytes);
                }
            } else if let Some(bytes) = mouse::wheel_keys(modes, app_cursor, m.rows, up) {
                self.send(&bytes);
            } else {
                let delta = if up {
                    WHEEL_LINES as i32
                } else {
                    -(WHEEL_LINES as i32)
                };
                if let Err(error) = self.client.scroll(&self.tab, delta) {
                    self.notice = Some(format!("scrolling: {}", error.message));
                }
            }
        }
        cx.notify();
    }

    fn cursor_bounds(&self) -> Option<Bounds<Pixels>> {
        let snapshot = self.snapshot.as_ref()?;
        let (row, col) = snapshot.cursor?;
        let m = self.metrics.get();
        let x = (m.origin_x + col as f32 * m.cell_width) / m.scale;
        let y = (m.origin_y + row as f32 * m.line_height) / m.scale;
        Some(Bounds::new(
            point(px(x), px(y)),
            size(px(m.cell_width / m.scale), px(m.line_height / m.scale)),
        ))
    }
}

impl Render for TerminalPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selection = match (&self.selection, &self.snapshot) {
            (Some(selection), Some(snapshot)) => {
                let top = self.top_line();
                selection
                    .spans(snapshot)
                    .into_iter()
                    .filter_map(|(line, start, end)| {
                        let row = line - top;
                        (row >= 0 && (row as usize) < snapshot.rows).then_some((
                            row as usize,
                            start,
                            end,
                        ))
                    })
                    .collect()
            }
            _ => Vec::new(),
        };
        let grid = GridElement {
            pane: cx.entity(),
            snapshot: self.snapshot.clone(),
            palette: self.palette.clone(),
            text: self.text.clone(),
            selection,
            focus: self.focus.clone(),
            metrics: self.metrics.clone(),
        };
        div()
            .size_full()
            .track_focus(&self.focus)
            .key_context("Terminal")
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .child(grid)
            .children(self.notice.clone().map(|notice| {
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .p_2()
                    .bg(colour(self.palette.background))
                    .text_color(colour(self.palette.foreground))
                    .child(notice)
            }))
    }
}

impl EntityInputHandler for TerminalPane {
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
        self.marked
            .as_ref()
            .map(|text| 0..text.encode_utf16().count())
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Committed text from the input method (or a plain key): the program reads it as typed.
        self.marked = None;
        self.send(text.as_bytes());
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        self.marked = (!new_text.is_empty()).then(|| new_text.to_owned());
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // The input method's window opens at the cursor.
        self.cursor_bounds()
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

impl gpui::Focusable for TerminalPane {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}
