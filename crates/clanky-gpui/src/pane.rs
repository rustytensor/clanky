//! One terminal pane: the latest snapshot from the session server, and every input that goes back to it
//! (keys, input-method text, paste, mouse reports, wheel, selection). The grid itself is drawn by `GridElement`.

use std::{
    cell::{Cell as Shared, RefCell},
    ops::Range,
    rc::Rc,
    sync::Arc,
    time::Instant,
};

use clanky::config::Zoom;
use clanky::mouse::{self, Clicks, Kind, Mods, Pos, Selection};
use clanky_api::Client;
use clanky_core::{MODE_BRACKETED_PASTE, TerminalSnapshot};
use clanky_state::{cursor_blink::Blink, themes::Colors};
use gpui::{
    App, Bounds, ClipboardItem, Context, EntityInputHandler, EventEmitter, FocusHandle,
    InteractiveElement, IntoElement, KeyDownEvent, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render, ScrollDelta, ScrollWheelEvent,
    Styled, Subscription, Task, UTF16Selection, Window, div, point, px, size,
};
use serde_json::Value;

mod blocks;
mod command;

pub use command::SharedInput;

use crate::{
    control_record::ControlRecord,
    grid_element::{GridElement, Metrics, TextSettings, colour},
    keys,
    link::{Link, Update},
    palette::Palette,
    tokens as t,
};

/// CSS points per logical pixel: the config gives the text size in pixels, `Zoom` counts points.
const POINTS_PER_PIXEL: f32 = 0.75;

/// Lines the history scrolls per wheel notch, as VTE.
const WHEEL_LINES: f32 = 3.0;
/// Lines gpui-ce reports for one wheel notch on X11 and Wayland (`SCROLL_LINES` in `gpui_linux`).
const GPUI_LINES_PER_NOTCH: f32 = 3.0;

/// Wheel notches in a scroll event: a mouse wheel's lines, or a touchpad's pixels as rows of history.
fn wheel_notches(delta: ScrollDelta, scale: f32, line_height: f32) -> f32 {
    match delta {
        ScrollDelta::Lines(delta) => delta.y / GPUI_LINES_PER_NOTCH,
        ScrollDelta::Pixels(delta) => {
            f32::from(delta.y) * scale / line_height.max(1.0) / WHEEL_LINES
        }
    }
}

pub struct TerminalPane {
    client: Client,
    tab: String,
    snapshot: Option<Arc<TerminalSnapshot>>,
    /// The server's command blocks for `snapshot`, from the same reply.
    blocks: Arc<Value>,
    palette: Palette,
    /// The interface colours, for the blocks' hairlines, meta text and sticky line.
    colors: Colors,
    record: ControlRecord,
    home: Option<String>,
    /// Wakes the pane on the next whole second while a running time counts.
    tick: Option<Task<()>>,
    /// The command input under the grid; None with the classic input (the shell's own prompt).
    input: Option<command::Input>,
    /// The rows as last drawn with space between command blocks, and that space.
    breath: Option<blocks::Spaced>,
    /// Device pixels the wheel took back from the lift that keeps the newest row in view.
    lift: f32,
    text: TextSettings,
    /// The text size in points, changed by Ctrl+wheel and Ctrl+= / Ctrl+- / Ctrl+0, as today's window.
    zoom: Zoom,
    focus: FocusHandle,
    metrics: Rc<Shared<Metrics>>,
    /// The size last sent to the server, so a resize is sent once per change.
    size_sent: (usize, usize),
    selection: Option<Selection>,
    selecting: bool,
    clicks: Clicks,
    /// The button held while the program receives mouse reports, and the cell last reported.
    reporting_button: Option<(u8, (usize, usize))>,
    /// The cell last reported for motion with no button held (mode 1003).
    motion_cell: Option<(usize, usize)>,
    wheel_rest: f32,
    /// Text the input method is composing; it is shown nowhere yet and sent only when committed.
    marked: Option<String>,
    notice: Option<String>,
    /// Clanky's opt-in frame log (`CLANKY_FRAME_TRACE`), shared by the window's panes: the pane logs its keys, the
    /// window logs its frames.
    trace: SharedTrace,
    blink: Blink,
    /// When the blink last restarted: a key press, or the window gaining or losing focus.
    blink_start: Instant,
    /// Wakes the pane when the cursor next appears or disappears; none once the blink rests.
    blink_task: Option<Task<()>>,
    window_active: bool,
    _activation: Subscription,
    _link: Link,
    _updates: Task<()>,
}

pub struct PaneSetup {
    pub client: Client,
    pub tab: String,
    pub palette: Palette,
    pub colors: Colors,
    pub record: ControlRecord,
    /// Set when the pane shows a command input.
    pub input: Option<SharedInput>,
    pub text: TextSettings,
    pub size_sent: (usize, usize),
    pub link: Link,
    pub updates: async_channel::Receiver<Update>,
    pub trace: SharedTrace,
    /// The cursor's and the input caret's blink, from the config.
    pub blink: Blink,
}

/// The window's one frame trace, shared by its panes.
pub type SharedTrace = Rc<RefCell<clanky::frame_trace::FrameTrace>>;

/// What a pane tells the window.
pub enum PaneEvent {
    /// The shell the window started in the pane has exited.
    Finished,
}

impl EventEmitter<PaneEvent> for TerminalPane {}

impl TerminalPane {
    pub fn new(setup: PaneSetup, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let updates = setup.updates;
        let task = cx.spawn_in(window, async move |pane, cx| {
            while let Ok(update) = updates.recv().await {
                let finished = matches!(update, Update::Finished);
                let applied = pane.update(cx, |pane, cx| {
                    match update {
                        Update::Snapshot(snapshot, blocks) => {
                            pane.snapshot = Some(snapshot);
                            pane.blocks = blocks;
                            pane.learn_commands();
                        }
                        Update::Lost(message) => pane.notice = Some(message),
                        Update::Finished => cx.emit(PaneEvent::Finished),
                    }
                    cx.notify();
                });
                if applied.is_err() || finished {
                    break;
                }
            }
        });
        let focus = cx.focus_handle();
        Self {
            client: setup.client,
            tab: setup.tab,
            snapshot: None,
            blocks: Arc::new(Value::Null),
            palette: setup.palette,
            colors: setup.colors,
            record: setup.record,
            home: clanky_core::platform::home_dir()
                .and_then(|home| home.to_str().map(str::to_owned)),
            tick: None,
            input: setup.input.map(command::Input::new),
            breath: None,
            lift: 0.0,
            zoom: Zoom::new(setup.text.size * POINTS_PER_PIXEL),
            text: setup.text,
            focus,
            metrics: Rc::new(Shared::new(Metrics::default())),
            size_sent: setup.size_sent,
            selection: None,
            selecting: false,
            clicks: Clicks::default(),
            reporting_button: None,
            motion_cell: None,
            wheel_rest: 0.0,
            marked: None,
            notice: None,
            trace: setup.trace,
            blink: setup.blink,
            blink_start: Instant::now(),
            blink_task: None,
            window_active: window.is_window_active(),
            _activation: cx.observe_window_activation(window, |pane, window, cx| {
                pane.window_active = window.is_window_active();
                pane.blink_start = Instant::now();
                pane.blink_task = None;
                cx.notify();
            }),
            _link: setup.link,
            _updates: task,
        }
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// The grid size last sent to the server.
    pub fn size_sent(&self) -> (usize, usize) {
        self.size_sent
    }

    /// One cell's width and height in logical pixels, once the pane has been laid out.
    pub fn cell_size(&self) -> Option<(f32, f32)> {
        let metrics = self.metrics.get();
        (metrics.cell_width > 0.0 && metrics.scale > 0.0).then(|| {
            (
                metrics.cell_width / metrics.scale,
                metrics.line_height / metrics.scale,
            )
        })
    }

    /// The pane's width in logical pixels, once it has been laid out.
    pub fn width(&self) -> Option<f32> {
        let metrics = self.metrics.get();
        (metrics.scale > 0.0).then(|| metrics.pane_width / metrics.scale)
    }

    /// The exit code of the newest finished block on the screen whose command is `command`.
    pub fn last_exit(&self, command: &str) -> Option<i32> {
        self.blocks["blocks"]
            .as_array()?
            .iter()
            .rev()
            .find(|block| {
                block["complete"] == true
                    && block["command"].as_str().map(str::trim) == Some(command)
            })
            .and_then(|block| block["exit_code"].as_i64())
            .and_then(|code| i32::try_from(code).ok())
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

    /// Sends input to the program. True when the pane itself must be drawn again (a selection ended or the input
    /// failed); the echo the program writes back arrives as an update of its own, so typing alone draws no frame.
    fn send(&mut self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }
        // Typing returns the view to the bottom (out of the history and any wheel lift) and ends a selection, as in
        // VTE's scroll-on-keystroke.
        let ended_selection = self.selection.take().is_some();
        self.lift = 0.0;
        let back = self.snapshot.as_ref().map_or(0, |s| s.display_offset);
        if back > 0
            && let Err(error) = self
                .client
                .scroll(&self.tab, -i32::try_from(back).unwrap_or(i32::MAX))
        {
            self.notice = Some(format!("returning to the newest rows: {}", error.message));
        }
        for chunk in bytes.chunks(64 * 1024) {
            if let Err(error) = self.client.input(&self.tab, chunk) {
                self.notice = Some(format!("sending input: {}", error.message));
                return true;
            }
        }
        ended_selection
    }

    fn modes(&self) -> (u32, bool) {
        self.snapshot
            .as_ref()
            .map_or((0, false), |s| (s.modes, s.app_cursor))
    }

    /// One zoom step in or out, or back to the configured size for a step of zero.
    fn zoom_by(&mut self, step: f32, cx: &mut Context<Self>) {
        if step == 0.0 {
            self.zoom.reset();
        } else {
            self.zoom.adjust(step);
        }
        self.text.size = self.zoom.points() / POINTS_PER_PIXEL;
        cx.notify();
    }

    /// A paste goes into the command input while it takes keys, else to the program.
    fn paste(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.input_takes_keys() {
            self.input_text(text, cx);
            return;
        }
        let bracketed = self.modes().0 & MODE_BRACKETED_PASTE != 0;
        self.send(&mouse::pastify(text, bracketed));
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        // A key press restarts the blink, as in VTE.
        self.blink_start = Instant::now();
        self.blink_task = None;
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
                        self.paste(&text, cx);
                    }
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                _ => {}
            }
        }
        if m.control && !m.alt {
            let step = match keystroke.key.as_str() {
                "=" | "+" => Some(1.0),
                "-" => Some(-1.0),
                "0" => Some(0.0),
                _ => None,
            };
            if let Some(step) = step {
                self.zoom_by(step, cx);
                cx.stop_propagation();
                return;
            }
        }
        if self.input_takes_keys() {
            self.input_key(keystroke, cx);
            return;
        }
        if let Some(bytes) = keys::bytes_for(keystroke, self.modes().1) {
            self.trace.borrow_mut().key_event();
            if self.send(&bytes) {
                cx.notify();
            }
            cx.stop_propagation();
        }
    }

    fn top_line(&self) -> i64 {
        self.snapshot
            .as_ref()
            .map_or(0, |s| s.history as i64 - s.display_offset as i64)
    }

    fn pos_at(&self, x: Pixels, y: Pixels) -> (Pos, usize, usize) {
        let m = self.metrics.get();
        let (row, col, right) = m.cell_at(x, y);
        let row = self
            .row_at(f32::from(y) * m.scale - m.origin_y)
            .unwrap_or(row);
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
        self.input_release();
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
                    self.paste(&text, cx);
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
        // Mode 1003 reports motion with no button held too, once per new cell, as button 0 (as xterm and VTE).
        if event.pressed_button.is_none()
            && self.reports(event.modifiers)
            && mouse::reports_motion(self.modes().0, false)
            && self.motion_cell != Some((row, col))
        {
            self.motion_cell = Some((row, col));
            if let Some(bytes) = mouse::report(
                self.modes().0,
                0,
                Self::mods(event.modifiers),
                col,
                row,
                true,
                false,
            ) {
                self.send(&bytes);
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
        // Ctrl+wheel zooms, as in Ptyxis with enable-zoom-scroll-ctrl: one step per event.
        if event.modifiers.control {
            let lines = match event.delta {
                ScrollDelta::Lines(delta) => delta.y,
                ScrollDelta::Pixels(delta) => f32::from(delta.y),
            };
            if lines != 0.0 {
                self.zoom_by(lines.signum(), cx);
            }
            return;
        }
        let m = self.metrics.get();
        self.wheel_rest += wheel_notches(event.delta, m.scale, m.line_height);
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
            } else if !self.lift_by_wheel(if up { WHEEL_LINES } else { -WHEEL_LINES }) {
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
        let y = (m.origin_y + self.row_offset(row)) / m.scale;
        Some(Bounds::new(
            point(px(x), px(y)),
            size(px(m.cell_width / m.scale), px(m.line_height / m.scale)),
        ))
    }
}

impl TerminalPane {}

impl Render for TerminalPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let drawn = self.blocks_drawn(window, cx);
        // Asked every frame: it also wakes the pane for the next blink, for the cursor and the input's caret alike.
        let blink_on = self.caret_on(cx);
        let grid = GridElement {
            pane: cx.entity(),
            snapshot: drawn.snapshot,
            tops: drawn.tops,
            palette: self.palette.clone(),
            text: self.text.clone(),
            selection,
            focus: self.focus.clone(),
            metrics: self.metrics.clone(),
            // The command input has the caret while it takes keys; the shell's empty prompt shows none.
            show_cursor: !self.input_takes_keys() && (!self.focus.is_focused(window) || blink_on),
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
            .relative()
            .child(grid)
            .children(drawn.overlay)
            .children(self.text_landmark(window))
            .children(self.notice.clone().map(|notice| {
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .p(px(t::SPACE_8))
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
        // Committed text from the input method (or a plain key): the command input takes it while it takes keys,
        // else the program reads it as typed.
        let had_marked = self.marked.take().is_some();
        if self.input_takes_keys() {
            self.input_text(text, cx);
            return;
        }
        self.trace.borrow_mut().key_event();
        if self.send(text.as_bytes()) || had_marked {
            cx.notify();
        }
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

#[cfg(test)]
// The notch counts compared are exact in binary: whole numbers from whole-number inputs.
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use gpui::{point, px};

    #[test]
    fn one_wheel_notch_is_one_notch() {
        // gpui-ce reports one notch up as three lines; a mouse program must get one wheel report for it.
        assert_eq!(
            wheel_notches(ScrollDelta::Lines(point(0.0, 3.0)), 1.0, 19.0),
            1.0
        );
        assert_eq!(
            wheel_notches(ScrollDelta::Lines(point(0.0, -6.0)), 1.5, 29.0),
            -2.0
        );
        // A touchpad's 57 device pixels at 19 px rows are three rows of history, one notch.
        assert_eq!(
            wheel_notches(ScrollDelta::Pixels(point(px(0.0), px(57.0))), 1.0, 19.0),
            1.0
        );
    }
}
