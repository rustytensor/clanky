//! The window's controls: buttons, icon buttons, chips, tabs, switches, fields, menu items, list rows and segmented
//! switchers, built by this one module so that every control of a kind has one height, one radius and one look per
//! state, and answers the pointer and the keyboard the same way. Screens never draw a control by hand.
//!
//! A screen describes a control with [`Control`] and hands it to [`Kit::build`]. The kit gives it its state's look
//! (the design rulebook's states table: rest, hover, pressed, keyboard focus, disabled) and its behaviour:
//! - it fires on release inside, through gpui's click, which a release outside cancels; Enter and Space fire the
//!   focused control the same way;
//! - Tab and Shift+Tab reach every enabled control in the order the screen builds them, which is reading order;
//! - the arrows move within a group (a menu, a list, a segmented switcher);
//! - keyboard focus is drawn only after keyboard input, never after a click;
//! - a pointer click leaves the keyboard where it was (with the pane), except on a field or a menu item, which take
//!   it ([`Kind::takes_focus_on_click`]).
//!
//! Every control is wrapped by the control record ([`ControlRecord::wrap`]), so the gates list each one with its
//! painted box, hit box and state. The element that takes the clicks is the one the record wraps, and it fills the
//! wrapper, as the record's rule asks.

use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    sync::Arc,
    time::Instant,
};

use clanky_state::themes::{Color, Colors};
use gpui::{
    AnyElement, App, ElementId, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px, svg,
};

pub use crate::control_look::Kind;
use crate::{
    control_look::{Pointer, State, border, fill, hover_progress, paint, ring_width},
    control_record::{ControlRecord, ControlSpec, Part, Via},
    icons::Icon,
    tokens as t,
};

/// The interface font, bundled with the window.
pub const INTERFACE_FONT_FAMILY: &str = "Inter";

/// The direction the arrows move in within a group: menus and lists go down, switchers across.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

impl Axis {
    fn name(self) -> &'static str {
        match self {
            Axis::Vertical => "vertical",
            Axis::Horizontal => "horizontal",
        }
    }

    /// One step back (-1) or forward (1) for an arrow key along this axis; other keys do not move.
    fn step(self, key: &str) -> Option<isize> {
        match (self, key) {
            (Axis::Vertical, "up") | (Axis::Horizontal, "left") => Some(-1),
            (Axis::Vertical, "down") | (Axis::Horizontal, "right") => Some(1),
            _ => None,
        }
    }
}

/// Controls the arrow keys move between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Group {
    pub name: &'static str,
    pub axis: Axis,
}

impl Group {
    pub const fn vertical(name: &'static str) -> Self {
        Group {
            name,
            axis: Axis::Vertical,
        }
    }

    pub const fn horizontal(name: &'static str) -> Self {
        Group {
            name,
            axis: Axis::Horizontal,
        }
    }
}

/// What a control does when it fires.
pub type Action = Rc<dyn Fn(&mut Window, &mut App)>;

/// One control, as a screen describes it.
pub struct Control {
    id: u32,
    kind: Kind,
    /// The text shown, and the control's name in the record; an icon button's label is only its name.
    label: SharedString,
    icon: Option<Icon>,
    enabled: bool,
    primary: bool,
    /// A tab's active state, a switch's value, a segment's selection.
    checked: Option<bool>,
    group: Option<Group>,
    /// A shortcut shown at a row's right end.
    keys: Option<SharedString>,
    /// A row's width is its container's; other kinds size to their content unless given one.
    width: Option<f32>,
    /// A field's text and caret, drawn by its owner.
    content: Option<AnyElement>,
    /// Something at the control's right end, inside it: a tab's close button.
    trailing: Option<AnyElement>,
    /// The focus of an owner that handles the control's keys itself (a field's editor).
    focus: Option<FocusHandle>,
    action: Option<Action>,
}

impl Control {
    pub fn new(kind: Kind, id: u32, label: impl Into<SharedString>) -> Self {
        Control {
            id,
            kind,
            label: label.into(),
            icon: None,
            enabled: true,
            primary: false,
            checked: if kind == Kind::Switch {
                Some(false)
            } else {
                None
            },
            group: None,
            keys: None,
            width: None,
            content: None,
            trailing: None,
            focus: None,
            action: None,
        }
    }

    #[must_use]
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// The primary button of a dialog.
    #[must_use]
    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }

    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    #[must_use]
    pub fn group(mut self, group: Group) -> Self {
        self.group = Some(group);
        self
    }

    #[must_use]
    pub fn keys(mut self, keys: impl Into<SharedString>) -> Self {
        self.keys = Some(keys.into());
        self
    }

    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    #[must_use]
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    #[must_use]
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_any_element());
        self
    }

    #[must_use]
    pub fn focus(mut self, focus: FocusHandle) -> Self {
        self.focus = Some(focus);
        self
    }

    #[must_use]
    pub fn on_click(mut self, action: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.action = Some(Rc::new(action));
        self
    }
}

/// What a control keeps from frame to frame.
struct Kept {
    focus: FocusHandle,
    /// When the pointer came over it; `None` while it is elsewhere.
    hover_since: Option<Instant>,
    /// The left button went down on it and has not come up yet.
    pressed: bool,
}

/// The focus handles of each group's enabled controls in the frame being built, in build order.
type Groups = Rc<RefCell<BTreeMap<(&'static str, &'static str), Vec<FocusHandle>>>>;

/// Builds a window's controls. One per window, cloned freely: the clones share the record and the frame's groups.
#[derive(Clone)]
pub struct Kit {
    record: ControlRecord,
    colors: Colors,
    /// Animations on (the config's `animations`); off, hover shows at once.
    motion: bool,
    groups: Groups,
    /// Enabled controls built so far in this frame: their places in the Tab order.
    tab_stops: Rc<Cell<usize>>,
}

impl Kit {
    pub fn new(record: ControlRecord, colors: Colors, motion: bool) -> Self {
        Kit {
            record,
            colors,
            motion,
            groups: Rc::default(),
            tab_stops: Rc::default(),
        }
    }

    pub fn record(&self) -> &ControlRecord {
        &self.record
    }

    pub fn set_colors(&mut self, colors: Colors) {
        self.colors = colors;
    }

    /// Called at the start of every frame the window draws, before any control is built.
    pub fn begin_frame(&self) {
        self.record.begin_frame();
        self.groups.borrow_mut().clear();
        self.tab_stops.set(0);
    }

    /// Gives the keyboard to the first enabled control of a group, as Down or Right does from no focus.
    pub fn focus_first(
        &self,
        screen: &'static str,
        group: Group,
        window: &mut Window,
        cx: &mut App,
    ) {
        let first = self
            .groups
            .borrow()
            .get(&(screen, group.name))
            .and_then(|handles| handles.first().cloned());
        if let Some(handle) = first {
            window.focus(&handle, cx);
        }
    }

    /// The control, drawn in its state and wired to the pointer and the keyboard.
    pub fn build(
        &self,
        screen: &'static str,
        control: Control,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        // Keyed apart from the body's own element id, whose element state gpui keeps under that id.
        let kept = window.use_keyed_state(
            ElementId::NamedChild(
                Arc::new(ElementId::NamedInteger(
                    screen.into(),
                    u64::from(control.id),
                )),
                "kept".into(),
            ),
            cx,
            |_, cx| Kept {
                focus: cx.focus_handle(),
                hover_since: None,
                pressed: false,
            },
        );
        let (own_focus, hover_since, pressed) = {
            let kept = kept.read(cx);
            (kept.focus.clone(), kept.hover_since, kept.pressed)
        };
        let focus = control.focus.clone().unwrap_or(own_focus);
        let keyboard_focus = focus.is_focused(window) && window.last_input_was_keyboard();
        let pointer = match (hover_since.is_some(), pressed) {
            (false, _) => Pointer::Away,
            (true, false) => Pointer::Over,
            (true, true) => Pointer::PressingOver,
        };
        let state = State::of(control.enabled, pointer, keyboard_focus);
        let hover = match hover_since {
            Some(since) if state == State::Hover => {
                let progress = hover_progress(since.elapsed().as_secs_f32() * 1000.0, self.motion);
                if progress < 1.0 {
                    window.request_animation_frame();
                }
                progress
            }
            _ => 1.0,
        };
        let scale = window.scale_factor();
        let spec = ControlSpec {
            screen,
            id: control.id,
            kind: control.kind.name(),
            label: control.label.to_string(),
            enabled: control.enabled,
            group: control.group.map(|group| group.name),
            axis: control.group.map(|group| group.axis.name()),
            checked: control.checked,
            primary: control.primary,
            focus: Some(focus.clone()),
        };
        let mut control = control;
        let field_content = (control.content.take(), control.trailing.take());
        // A tab shows its trailing close button while it is active, under the pointer, or holding the keyboard
        // (itself or the button); elsewhere the button's room stays, so no tab moves when another changes state.
        let show_trailing = control.kind != Kind::Tab
            || control.checked == Some(true)
            || matches!(state, State::Hover | State::Pressed | State::Focus)
            || focus.contains_focused(window, cx);
        let body = self.body(
            screen,
            &control,
            field_content,
            (state, hover, scale, show_trailing),
        );
        let body = if control.enabled {
            self.wire(screen, control, body, &kept, &focus)
        } else {
            body.opacity(t::DISABLED_OPACITY).into_any_element()
        };
        self.record.wrap(spec, body)
    }

    /// The control's box, fill, border, focus ring and content, for its state.
    fn body(
        &self,
        screen: &'static str,
        control: &Control,
        field_content: (Option<AnyElement>, Option<AnyElement>),
        appearance: (State, f32, f32, bool),
    ) -> gpui::Stateful<gpui::Div> {
        let (state, hover, scale, show_trailing) = appearance;
        let colors = &self.colors;
        let kind = control.kind;
        let height = kind.height();
        let hair = t::hairline(scale);
        let mut body = div()
            .id(ElementId::NamedInteger(
                SharedString::from(screen),
                u64::from(control.id),
            ))
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(px(height))
            .min_w(px(t::MIN_HIT_SIZE))
            .rounded(px(kind.radius()));
        body = match (kind, control.width) {
            (_, Some(width)) => body.w(px(width)),
            (Kind::Switch, None) => body.w(px(t::SWITCH_CONTROL_WIDTH)),
            (Kind::IconButton | Kind::IconButtonInRow, None) => body.w(px(height)),
            (Kind::MenuItem | Kind::ListRow | Kind::Field, None) => body.w_full(),
            _ => body,
        };
        if let Some(fill) = fill(kind, state, control.primary, control.checked, colors) {
            let strength = if state == State::Hover && !control.primary {
                hover
            } else {
                1.0
            };
            body = body.bg(paint(fill, strength));
        }
        // Every control has its one-pixel edge in every state, see-through where the state draws none: an edge
        // adds to a box's size, so a control that gained one on focus or when active would grow.
        let edge = border(kind, state, control.checked, colors).unwrap_or(t::TRANSPARENT);
        let width = if state == State::Focus && self.record.faults().border_two_pixels() {
            2.0 * hair
        } else {
            hair
        };
        body = body.border(px(width)).border_color(paint(edge, 1.0));
        if state == State::Focus {
            body = body.child(focus_ring(kind, scale, colors));
        }
        if kind == Kind::Tab {
            body = body.max_w(px(t::TAB_MAX_WIDTH));
        }
        body.child(self.content(screen, control, field_content, (scale, show_trailing)))
    }

    /// The pointer, the keyboard and the action, on an enabled control's body.
    fn wire(
        &self,
        screen: &'static str,
        control: Control,
        body: gpui::Stateful<gpui::Div>,
        kept: &gpui::Entity<Kept>,
        focus: &FocusHandle,
    ) -> AnyElement {
        let faults = self.record.faults();
        let kind = control.kind;
        let place = self.tab_stops.get();
        self.tab_stops.set(place + 1);
        let mut body = body.track_focus(focus).tab_stop(!faults.tab_skip(place));
        if let Some(group) = control.group {
            let key = (screen, group.name);
            self.groups
                .borrow_mut()
                .entry(key)
                .or_default()
                .push(focus.clone());
            let groups = Rc::clone(&self.groups);
            let own = focus.clone();
            body = body.on_key_down(move |event: &KeyDownEvent, window, cx| {
                let Some(step) = group.axis.step(event.keystroke.key.as_str()) else {
                    return;
                };
                let next = groups.borrow().get(&key).and_then(|handles| {
                    let index = handles.iter().position(|handle| *handle == own)?;
                    let target = index.checked_add_signed(step)?;
                    handles.get(target).cloned()
                });
                if let Some(next) = next {
                    window.focus(&next, cx);
                }
                cx.stop_propagation();
            });
        }
        let hovered = kept.clone();
        body = body.on_hover(move |inside: &bool, _, cx| {
            let since = inside.then(Instant::now);
            hovered.update(cx, |kept, cx| {
                kept.hover_since = since;
                cx.notify();
            });
        });
        let (down, up, up_out) = (kept.clone(), kept.clone(), kept.clone());
        let release = move |kept: &gpui::Entity<Kept>, cx: &mut App| {
            kept.update(cx, |kept, cx| {
                if kept.pressed {
                    kept.pressed = false;
                    cx.notify();
                }
            });
        };
        body = body
            .on_mouse_up(MouseButton::Left, move |_, _, cx| release(&up, cx))
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| release(&up_out, cx));
        let Some(action) = control.action else {
            return body
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    press(&down, kind, window, cx);
                })
                .into_any_element();
        };
        let record = self.record.clone();
        let label = control.label.clone();
        let id = control.id;
        let fire = Rc::clone(&action);
        let early = (record.clone(), label.clone());
        body.on_mouse_down(MouseButton::Left, move |_, window, cx| {
            press(&down, kind, window, cx);
            if faults.press_fires() {
                early.0.activate(screen, id, &early.1, Via::Release);
                fire(window, cx);
            }
        })
        .on_click(move |event, window, cx| {
            let via = Via::of(event);
            if faults.press_fires() && via == Via::Release {
                return;
            }
            record.activate(screen, id, &label, via);
            action(window, cx);
        })
        .into_any_element()
    }

    /// A control's label text, with its measuring child.
    fn label(&self, screen: &'static str, control: &Control, ink: Color) -> AnyElement {
        let kind = control.kind;
        let weight = if kind == Kind::Tab && control.checked == Some(true) {
            t::WEIGHT_MEDIUM
        } else {
            t::WEIGHT_REGULAR
        };
        let text = |weight: u16| {
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .font_family(INTERFACE_FONT_FAMILY)
                .text_size(px(kind.text_size()))
                .font_weight(FontWeight(f32::from(weight)))
                .text_color(paint(ink, 1.0))
                .child(control.label.clone())
        };
        let label = if kind == Kind::Tab {
            // A tab's room is its label's at the active weight, whatever its own, so a tab is as wide active as
            // not; the label is centred in that room.
            div()
                .relative()
                .min_w_0()
                .child(text(t::WEIGHT_MEDIUM).invisible())
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .justify_center()
                        .child(text(weight).relative().child(self.record.part(
                            screen,
                            control.id,
                            Part::Label,
                        ))),
                )
        } else {
            text(weight)
                .relative()
                .child(self.record.part(screen, control.id, Part::Label))
        };
        label.into_any_element()
    }

    /// The icon, label and trailing keys; a switch's knob; a field's content.
    fn content(
        &self,
        screen: &'static str,
        control: &Control,
        field_content: (Option<AnyElement>, Option<AnyElement>),
        place: (f32, bool),
    ) -> AnyElement {
        let (scale, show_trailing) = place;
        let colors = &self.colors;
        let kind = control.kind;
        if kind == Kind::Switch {
            return switch_knob(control.checked == Some(true), scale, colors);
        }
        let ink = if control.primary {
            colors.primary_foreground
        } else if kind == Kind::Field {
            // A field without content shows its name as a placeholder.
            colors.muted_foreground
        } else {
            colors.foreground
        };
        let row_like = kind.is_row() || kind == Kind::Field;
        let mut inner = div()
            .flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .items_center()
            .gap(px(if row_like { t::ROW_GAP } else { t::SPACE_6 }))
            .px(px(if kind.is_icon_only() { 0.0 } else { t::SPACE_8 }));
        if !row_like {
            inner = inner.justify_center();
        }
        if let Some(icon) = control.icon {
            let size = kind.icon_size();
            inner = inner.child(
                div()
                    .relative()
                    .flex_none()
                    .size(px(size))
                    .child(
                        svg()
                            .data(icon.svg())
                            .size(px(size))
                            .text_color(paint(ink, 1.0)),
                    )
                    .child(self.record.part(screen, control.id, Part::Icon)),
            );
        }
        let (field_content, trailing) = field_content;
        if let Some(content) = field_content {
            // The owner's text and caret; the field only frames them.
            return inner
                .child(div().flex_1().min_w_0().child(content))
                .into_any_element();
        }
        if !kind.is_icon_only() && !control.label.is_empty() {
            inner = inner.child(self.label(screen, control, ink));
        }
        if let Some(keys) = &control.keys {
            inner = inner.child(
                div()
                    .ml_auto()
                    .flex_none()
                    .font_family(t::TERMINAL_FONT_FAMILY)
                    .text_size(px(kind.text_size()))
                    .text_color(paint(colors.muted_foreground, 1.0))
                    .child(keys.clone()),
            );
        }
        if let Some(trailing) = trailing {
            // A button inside the control sits as far in from the right edge as from the top and bottom.
            let mut room = div().flex_none().child(trailing);
            if !show_trailing {
                room = room.invisible();
            }
            inner = inner
                .pr(px((kind.height() - t::ICON_BUTTON_HEIGHT_ROW) / 2.0))
                .child(room);
        }
        inner.into_any_element()
    }
}

/// The left button went down on a control. gpui runs a control's own mouse-down listeners before the one that moves
/// the focus to it, so preventing the default here keeps the keyboard where it was for the kinds that do not take it.
fn press(kept: &gpui::Entity<Kept>, kind: Kind, window: &mut Window, cx: &mut App) {
    if !kind.takes_focus_on_click() {
        window.prevent_default();
    }
    kept.update(cx, |kept, cx| {
        kept.pressed = true;
        cx.notify();
    });
}

/// A segmented switcher's segments: ids and labels in order, the selected one, and what choosing one does (it gets
/// the chosen segment's index).
pub struct Switcher<'a> {
    pub group: Group,
    pub segments: &'a [(u32, &'a str)],
    pub selected: usize,
    pub choose: Choose,
}

/// What choosing a segment does, given its index.
pub type Choose = Rc<dyn Fn(usize, &mut Window, &mut App)>;

impl Kit {
    /// A segmented switcher: a tab-high pill holding its segments, inset by `SPACE_2`; the selected segment is
    /// lifted onto the background.
    pub fn segmented(
        &self,
        screen: &'static str,
        switcher: &Switcher<'_>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let mut pill = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(t::SPACE_2))
            .h(px(t::TAB_HEIGHT))
            .p(px(t::SPACE_2))
            .rounded(px(t::RADIUS_MEDIUM))
            .bg(paint(self.colors.muted, t::SWITCHER_FILL_OPACITY));
        for (index, (id, label)) in switcher.segments.iter().enumerate() {
            let choose = Rc::clone(&switcher.choose);
            let segment = Control::new(Kind::Segment, *id, (*label).to_owned())
                .checked(index == switcher.selected)
                .group(switcher.group)
                .on_click(move |window, cx| choose(index, window, cx));
            pill = pill.child(self.build(screen, segment, window, cx));
        }
        pill.into_any_element()
    }
}

/// The keyboard focus ring: `FOCUS_RING_WIDTH` outside the control, rounded to whole device pixels, in the ring
/// colour at `FOCUS_RING_OPACITY`. It is drawn around the box, so the fill inside stays the state's own.
fn focus_ring(kind: Kind, scale: f32, colors: &Colors) -> AnyElement {
    let ring = ring_width(scale);
    div()
        .absolute()
        .top(px(-ring))
        .left(px(-ring))
        .right(px(-ring))
        .bottom(px(-ring))
        .rounded(px(kind.radius() + ring))
        .border(px(ring))
        .border_color(paint(colors.ring, t::FOCUS_RING_OPACITY))
        .into_any_element()
}

/// A switch's knob: `SPACE_2` in from the track's outer edge, at the right when on.
fn switch_knob(on: bool, scale: f32, colors: &Colors) -> AnyElement {
    let knob = t::SWITCH_CONTROL_HEIGHT - 2.0 * t::SPACE_2;
    let ink = if on {
        colors.primary_foreground
    } else {
        colors.muted_foreground
    };
    let mut track = div()
        .flex()
        .size_full()
        .items_center()
        .px(px(t::SPACE_2 - t::hairline(scale)));
    if on {
        track = track.justify_end();
    }
    track
        .child(
            div()
                .flex_none()
                .size(px(knob))
                .rounded(px(knob / 2.0))
                .bg(paint(ink, 1.0)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_move_along_their_groups_axis_only() {
        assert_eq!(Axis::Vertical.step("down"), Some(1));
        assert_eq!(Axis::Vertical.step("up"), Some(-1));
        assert_eq!(Axis::Vertical.step("right"), None);
        assert_eq!(Axis::Horizontal.step("left"), Some(-1));
        assert_eq!(Axis::Horizontal.step("down"), None);
    }
}
