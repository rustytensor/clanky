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
    AnyElement, App, Bounds, ElementId, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, Pixels, SharedString, StatefulInteractiveElement,
    Styled, Window, canvas, div, px, svg,
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
    /// A row of a tree: how deep it sits, and its chevron (open or closed) for a folder, none for a file, whose
    /// chevron's room stays so names line up.
    tree: Option<(usize, Option<bool>)>,
    /// The focus of an owner that handles the control's keys itself (a field's editor).
    focus: Option<FocusHandle>,
    action: Option<Action>,
    /// Set for a control inside another control (a tab's close): its press stays with it and never reaches what
    /// holds it (the tab's press would start a drag), and the value says whether Tab reaches it (not while it is
    /// kept hidden with its room reserved, as on an inactive tab).
    inside: Option<bool>,
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
            tree: None,
            focus: None,
            action: None,
            inside: None,
        }
    }

    /// The control sits inside another control; `reachable`: whether Tab reaches it now.
    #[must_use]
    pub fn inside(mut self, reachable: bool) -> Self {
        self.inside = Some(reachable);
        self
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
    pub fn tree(mut self, depth: usize, chevron: Option<bool>) -> Self {
        self.tree = Some((depth, chevron));
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

/// An enabled control Tab reaches: its screen and id, and its box as last drawn.
struct Stop {
    screen: &'static str,
    id: u32,
    focus: FocusHandle,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

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
    /// The frame's controls that Tab reaches, with their drawn boxes, for walking them in reading order.
    stops: Rc<RefCell<Vec<Stop>>>,
    /// The screen of the open popover, set while it is drawn: Tab, Shift+Tab and Ctrl+F6 then stay among its
    /// controls.
    trap: Rc<Cell<Option<&'static str>>>,
}

impl Kit {
    pub fn new(record: ControlRecord, colors: Colors, motion: bool) -> Self {
        Kit {
            record,
            colors,
            motion,
            groups: Rc::default(),
            tab_stops: Rc::default(),
            stops: Rc::default(),
            trap: Rc::default(),
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
        self.stops.borrow_mut().clear();
        self.trap.set(None);
    }

    /// Keeps the keyboard's walk among one screen's controls for this frame: the open popover's.
    pub fn trap(&self, screen: &'static str) {
        self.trap.set(Some(screen));
    }

    /// The box a control of the window was last drawn in, if Tab reaches it.
    pub fn bounds_of(&self, screen: &'static str, id: u32) -> Option<Bounds<Pixels>> {
        self.stops
            .borrow()
            .iter()
            .find(|stop| stop.screen == screen && stop.id == id)
            .and_then(|stop| stop.bounds.get())
    }

    /// The control that has the keyboard: its screen, id and last drawn box.
    pub fn focused_control(&self, window: &Window) -> Option<(&'static str, u32, Bounds<Pixels>)> {
        let stops = self.stops.borrow();
        let stop = stops.iter().find(|stop| stop.focus.is_focused(window))?;
        Some((stop.screen, stop.id, stop.bounds.get()?))
    }

    /// Gives the keyboard to a group's control by its place among the group's enabled controls.
    pub fn focus_in_group(
        &self,
        screen: &'static str,
        group: Group,
        index: usize,
        window: &mut Window,
        cx: &mut App,
    ) {
        let handle = self
            .groups
            .borrow()
            .get(&(screen, group.name))
            .and_then(|handles| handles.get(index).cloned());
        if let Some(handle) = handle {
            window.focus(&handle, cx);
        }
    }

    /// The controls Tab reaches, in reading order: by the top of their drawn box, then its left, as a reader scans
    /// the window. A control not drawn yet goes last.
    fn reading_order(&self) -> Vec<FocusHandle> {
        let stops = self.stops.borrow();
        let trap = self.trap.get();
        let mut placed: Vec<(f32, f32, &FocusHandle)> = stops
            .iter()
            .filter(|stop| trap.is_none_or(|screen| stop.screen == screen))
            .map(|stop| {
                let at = stop.bounds.get().map_or((f32::MAX, f32::MAX), |b| {
                    (f32::from(b.origin.y).round(), f32::from(b.origin.x).round())
                });
                (at.0, at.1, &stop.focus)
            })
            .collect();
        placed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        placed
            .into_iter()
            .map(|(_, _, focus)| focus.clone())
            .collect()
    }

    /// Whether a control of the window has the keyboard.
    pub fn has_focus(&self, window: &Window) -> bool {
        self.stops
            .borrow()
            .iter()
            .any(|stop| stop.focus.is_focused(window))
    }

    /// Gives the keyboard to the first control in reading order (Ctrl+F6 from a pane). False when there is none.
    pub fn focus_first_control(&self, window: &mut Window, cx: &mut App) -> bool {
        let Some(first) = self.reading_order().into_iter().next() else {
            return false;
        };
        window.focus(&first, cx);
        true
    }

    /// Tab or Shift+Tab among the controls, wrapping at either end. False when no control has the keyboard.
    pub fn focus_step(&self, forward: bool, window: &mut Window, cx: &mut App) -> bool {
        let order = self.reading_order();
        let Some(index) = order.iter().position(|focus| focus.is_focused(window)) else {
            return false;
        };
        let next = if forward {
            (index + 1) % order.len()
        } else {
            (index + order.len() - 1) % order.len()
        };
        window.focus(&order[next], cx);
        true
    }

    /// Gives the keyboard to the first enabled control of a group, as Down or Right does from no focus.
    pub fn focus_first(
        &self,
        screen: &'static str,
        group: Group,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.focus_in_group(screen, group, 0, window, cx);
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
            (Kind::IconButton | Kind::IconButtonInline, None) => body.w(px(height)),
            (Kind::MenuItem | Kind::ListRow | Kind::PaletteRow | Kind::Field, None) => {
                body.w_full()
            }
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
            body = body.child(focus_ring(kind, scale, width, colors));
        }
        if kind == Kind::Tab {
            body = body.max_w(px(t::TAB_MAX_WIDTH));
        }
        // Icons are muted at rest and take the label's colour on hover, press, focus or when selected.
        let lit = matches!(state, State::Hover | State::Pressed | State::Focus)
            || control.checked == Some(true);
        body.child(self.content(screen, control, field_content, (scale, show_trailing, lit)))
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
        let reached = control.inside.unwrap_or(true) && !faults.tab_skip(place);
        let mut body = body.track_focus(focus).tab_stop(reached);
        if reached {
            let bounds: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::default();
            self.stops.borrow_mut().push(Stop {
                screen,
                id: control.id,
                focus: focus.clone(),
                bounds: Rc::clone(&bounds),
            });
            body = body.child(
                canvas(move |drawn, _, _| bounds.set(Some(drawn)), |_, (), _, _| {})
                    .absolute()
                    .size_full(),
            );
        }
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
        let keeps_press = control.inside.is_some();
        body.on_mouse_down(MouseButton::Left, move |_, window, cx| {
            press(&down, kind, window, cx);
            if keeps_press {
                cx.stop_propagation();
            }
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
    fn label(&self, screen: &'static str, control: &Control, ink: Color, scale: f32) -> AnyElement {
        let kind = control.kind;
        // The active tab and the search-and-pick lists' rows (text_title) are set in the medium weight.
        let weight =
            if (kind == Kind::Tab && control.checked == Some(true)) || kind == Kind::PaletteRow {
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
            // not; the label is centred in that room. Its line fills the tab, so the line's top falls on
            // a whole device pixel at every scale and only the baseline's rounding is left: the tab's height inside
            // its two one-device-pixel edges.
            let line = kind.height() - 2.0 * t::hairline(scale);
            let text = |weight: u16| text(weight).line_height(px(line));
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
        } else if matches!(
            kind,
            Kind::MenuItem | Kind::PaletteRow | Kind::ButtonBar | Kind::ListRow
        ) {
            // A menu item's, list or popover row's or bar button's line fills the control, as a tab's does: the
            // control's top is a whole device pixel, and a shorter centred line would put its top, and so the
            // baseline, off by part of a pixel.
            text(weight)
                .line_height(px(kind.height()))
                .relative()
                .child(self.record.part(screen, control.id, Part::Label))
        } else if kind == Kind::Field {
            // A field's line fills the field inside its two one-device-pixel edges, as a tab's does, so the line's
            // top, and so the baseline, is not left half a device pixel off at 1.5.
            text(weight)
                .line_height(px(kind.height() - 2.0 * t::hairline(scale)))
                .relative()
                .child(self.record.part(screen, control.id, Part::Label))
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
        place: (f32, bool, bool),
    ) -> AnyElement {
        let (scale, show_trailing, lit) = place;
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
        if let Some((depth, chevron)) = control.tree {
            // The rulebook's list row: 12 px a level of indent, a 12 px chevron, then the icon.
            inner = inner.pl(px(t::SPACE_8 + depth as f32 * t::ROW_INDENT));
            let mut room = div().flex_none().size(px(t::ROW_CHEVRON_SIZE));
            if let Some(open) = chevron {
                let icon = if open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                };
                room = room.child(
                    svg()
                        .data(icon.svg())
                        .size(px(t::ROW_CHEVRON_SIZE))
                        .text_color(paint(colors.muted_foreground, 1.0)),
                );
            }
            inner = inner.child(room);
        }
        if let Some(icon) = control.icon {
            let size = kind.icon_size();
            // In a field whose inside is an odd number of device pixels taller than its icon (32 and 14 at 1.5), the
            // icon can only sit half a pixel off centre, and gpui rounds it up, where the ink's soft lower edge adds
            // to the offset. A one-device-pixel top margin puts it the half pixel low instead.
            let inside = (kind.height() - 2.0 * t::hairline(scale)) * scale;
            let field_nudge = kind == Kind::Field && (inside - size * scale).round() % 2.0 != 0.0;
            // An icon-only control whose spare room is an odd count of device pixels (a 14 px icon in 28 at 1.5) cannot
            // centre its icon on whole pixels. Layout rounds the half pixel toward the top left, where the icon's
            // antialiased edges already lean, which tips it past half a pixel off; one device pixel of margin before
            // the icon puts the spare pixel there instead, half a pixel the other way.
            let spare = ((kind.height() - size) * scale).round();
            let button_nudge = kind.is_icon_only() && spare.rem_euclid(2.0) > 0.5;
            let top = if field_nudge || button_nudge {
                t::hairline(scale)
            } else {
                0.0
            };
            let left = if button_nudge {
                t::hairline(scale)
            } else {
                0.0
            };
            inner = inner.child(
                div()
                    .relative()
                    .flex_none()
                    .size(px(size))
                    .mt(px(top))
                    .ml(px(left))
                    .child(svg().data(icon.svg()).size(px(size)).text_color(paint(
                        if lit || control.primary {
                            ink
                        } else {
                            colors.muted_foreground
                        },
                        1.0,
                    )))
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
            inner = inner.child(self.label(screen, control, ink, scale));
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
                .pr(px((kind.height() - t::ICON_BUTTON_INLINE_SIZE) / 2.0))
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
/// colour at `FOCUS_RING_OPACITY`. It is drawn around the box, so the fill inside stays the state's own. An absolute
/// child is placed from inside the control's own edge (`edge` wide), so the ring reaches out past that edge too.
fn focus_ring(kind: Kind, scale: f32, edge: f32, colors: &Colors) -> AnyElement {
    let ring = ring_width(scale);
    let out = px(-(ring + edge));
    div()
        .absolute()
        .top(out)
        .left(out)
        .right(out)
        .bottom(out)
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
