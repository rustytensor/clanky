//! The popover: the one floating element under the palette, the menus, the agent picker, the completion list and the
//! find bar. A `card` box with the large radius, a one-device-pixel `border` edge and the popover shadow, drawn above
//! the window, placed next to what opened it and never over it, kept inside the window. While it is open the keyboard
//! stays in it (Tab and Shift+Tab walk only its controls); Esc closes it and gives the keyboard back to whatever had it
//! when it opened, and so does a click outside it.
//!
//! A window keeps one [`Popover`] (one popover is open at a time), opens it with [`Popover::open`] from an
//! [`Opener`], sends it its keys first ([`Popover::key`]) and draws it with [`Popover::element`] around the screen's
//! own content. Each floating screen has its own name in the control record, given at open, and its controls are
//! built under that name.
//!
//! gpui's `anchored` keeps an element on screen but, when it overflows, flips it onto the point it was anchored to,
//! which covers the opener; so the side is chosen here ([`place`]) from the opener's box, the popover's measured size
//! and the window, and gpui has no focus restore of its own, so the popover saves `Window::focused` at open.

use clanky_state::{
    keys::{Chord, Key},
    list_selection::{ListKey, ListSelection},
    themes::Colors,
};
use gpui::{
    AnyElement, App, Bounds, Display, Element, FocusHandle, GlobalElementId, InspectorElementId,
    InteractiveElement, IntoElement, KeyDownEvent, LayoutId, ParentElement, Pixels, Point,
    Position, Style, Styled, Window, deferred, div, point, px,
};

use crate::{control_look::paint, control_record::Faults, controls::Kit, tokens as t};

/// A box in logical pixels, from the window's top-left corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Area {
    pub fn of(bounds: Bounds<Pixels>) -> Self {
        Area {
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            w: f32::from(bounds.size.width),
            h: f32::from(bounds.size.height),
        }
    }

    fn right(self) -> f32 {
        self.x + self.w
    }

    fn bottom(self) -> f32 {
        self.y + self.h
    }

    fn logical(self) -> [f32; 4] {
        [self.x, self.y, self.w, self.h]
    }
}

/// Which side of its opener a popover went to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Below,
    Above,
    Right,
    Left,
}

/// Where a popover goes: beside its opener (menus, the completion list, the find bar's own buttons), or centred
/// across the window with its top at `top` (a search-and-pick list such as the palette).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    Beside,
    Centred { top: f32 },
}

/// The popover's top-left corner for its size, next to the opener: below it when it fits, else above, else to its
/// right, else to its left, `POPOVER_OPENER_GAP` away, and slid along that side to stay `POPOVER_WINDOW_MARGIN` inside
/// the window. A popover that fits on no side is only slid inside the window, as near its opener as it can be, and may
/// then cover it; the side named is the one with more room, below or above.
pub fn place(opener: Area, size: (f32, f32), window: (f32, f32)) -> (f32, f32, Side) {
    let (gap, margin) = (t::POPOVER_OPENER_GAP, t::POPOVER_WINDOW_MARGIN);
    let (width, height) = size;
    let along_x = slide(opener.x, width, window.0);
    let along_y = slide(opener.y, height, window.1);
    let room_below = window.1 - margin - (opener.bottom() + gap);
    let room_above = opener.y - gap - margin;
    if height <= room_below {
        return (along_x, opener.bottom() + gap, Side::Below);
    }
    if height <= room_above {
        return (along_x, opener.y - gap - height, Side::Above);
    }
    if width <= window.0 - margin - (opener.right() + gap) {
        return (opener.right() + gap, along_y, Side::Right);
    }
    if width <= opener.x - gap - margin {
        return (opener.x - gap - width, along_y, Side::Left);
    }
    let side = if room_below >= room_above {
        Side::Below
    } else {
        Side::Above
    };
    (along_x, along_y, side)
}

/// Where along one axis a popover of `length` starting at `start` stays inside `0..window` with the margin: moved
/// back from the far edge, and never before the near margin.
fn slide(start: f32, length: f32, window: f32) -> f32 {
    let margin = t::POPOVER_WINDOW_MARGIN;
    start.min(window - margin - length).max(margin)
}

/// What opened a popover: the box it goes next to, and what gets the keyboard back.
#[derive(Clone, Debug, PartialEq)]
pub struct Opener {
    pub area: Area,
    /// The control that opened it, by id.
    pub control: Option<u32>,
    /// The pane that opened it (a right-click, or a key typed in the pane), by id.
    pub pane: Option<String>,
}

impl Opener {
    /// A control of the window, as last drawn; None when it is not drawn.
    pub fn control(kit: &Kit, screen: &'static str, id: u32) -> Option<Self> {
        Some(Opener {
            area: Area::of(kit.bounds_of(screen, id)?),
            control: Some(id),
            pane: None,
        })
    }

    /// The control that has the keyboard, if one has.
    pub fn focused_control(kit: &Kit, window: &Window) -> Option<Self> {
        let (_, id, bounds) = kit.focused_control(window)?;
        Some(Opener {
            area: Area::of(bounds),
            control: Some(id),
            pane: None,
        })
    }

    /// A point in a pane, such as where a right-click landed: a box of no size.
    pub fn point(at: Point<Pixels>, pane: String) -> Self {
        Opener {
            area: Area {
                x: f32::from(at.x),
                y: f32::from(at.y),
                ..Area::default()
            },
            control: None,
            pane: Some(pane),
        }
    }
}

struct Open {
    screen: &'static str,
    opener: Opener,
    placement: Placement,
    /// What had the keyboard when the popover opened.
    give_back: Option<FocusHandle>,
    /// The popover's own focus, which it holds until a control of its own takes the keyboard.
    focus: FocusHandle,
    /// The selected row of the list it shows, if it shows one.
    list: ListSelection,
}

/// The window's popover: closed, or one screen open.
#[derive(Default)]
pub struct Popover {
    open: Option<Open>,
}

impl Popover {
    /// Opens a screen's popover and takes the keyboard. A popover opened over another one replaces it and keeps what
    /// the first one would give the keyboard back to.
    pub fn open(
        &mut self,
        screen: &'static str,
        opener: Opener,
        placement: Placement,
        window: &mut Window,
        cx: &mut App,
    ) {
        let give_back = match self.open.take() {
            Some(open) => open.give_back,
            None => window.focused(cx),
        };
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.open = Some(Open {
            screen,
            opener,
            placement,
            give_back,
            focus,
            list: ListSelection::default(),
        });
    }

    /// The open popover's list now holds `len` rows, with its first row selected: a screen calls this when it opens
    /// and after each change to what the list holds (a keystroke that filters it). A menu, which opens with no row
    /// selected, calls [`Popover::resize_list`] instead.
    pub fn restart_list(&mut self, len: usize) {
        if let Some(open) = self.open.as_mut() {
            open.list = ListSelection::first_of(len);
        }
    }

    /// The open popover's list now holds `len` rows; the selection stays where it was, or on the new last row.
    pub fn resize_list(&mut self, len: usize) {
        if let Some(open) = self.open.as_mut() {
            open.list.resize(len);
        }
    }

    /// The selected row of the open popover's list.
    pub fn selected_row(&self) -> Option<usize> {
        self.open.as_ref().and_then(|open| open.list.selected())
    }

    /// A key for the open popover's list: the arrows, Home and End move its selection, Enter picks the selected row.
    /// [`ListKey::Ignored`] while closed. The screen acts on a pick (and usually closes the popover).
    pub fn list_key(&mut self, chord: Chord) -> ListKey {
        self.open
            .as_mut()
            .map_or(ListKey::Ignored, |open| open.list.key(chord))
    }

    /// Closes the popover and gives the keyboard back to what had it at open. False when none was open.
    pub fn close(&mut self, kit: &Kit, window: &mut Window, cx: &mut App) -> bool {
        let Some(open) = self.open.take() else {
            return false;
        };
        if let Some(handle) = open.give_back
            && !kit.record().faults().esc_loses_focus()
        {
            window.focus(&handle, cx);
        }
        true
    }

    /// The open popover's screen name.
    pub fn screen(&self) -> Option<&'static str> {
        self.open.as_ref().map(|open| open.screen)
    }

    /// What opened the open popover.
    pub fn opener(&self) -> Option<&Opener> {
        self.open.as_ref().map(|open| &open.opener)
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Whether the popover's own focus has the keyboard (none of its controls has it yet).
    pub fn holds_keyboard(&self, window: &Window) -> bool {
        self.open
            .as_ref()
            .is_some_and(|open| open.focus.is_focused(window))
    }

    /// The window's keys go here first while a popover is open: Esc closes it; Tab and Shift+Tab walk its controls
    /// and wrap, never leaving it. True when the key was one of these.
    pub fn key(&mut self, chord: Chord, kit: &Kit, window: &mut Window, cx: &mut App) -> bool {
        if self.open.is_none() || chord.mods.ctrl || chord.mods.alt {
            return false;
        }
        match chord.key {
            Key::Escape => self.close(kit, window, cx),
            Key::Tab => {
                if !kit.focus_step(!chord.mods.shift, window, cx) {
                    kit.focus_first_control(window, cx);
                }
                true
            }
            _ => false,
        }
    }

    /// The open popover around a screen's content, drawn above the window; None while closed. `keys` takes the keys
    /// the popover's own focus gets (a list's arrows and Enter before a row has the keyboard); `dismiss` runs on a
    /// press outside the popover and should close it.
    pub fn element(
        &self,
        kit: &Kit,
        colors: &Colors,
        content: impl IntoElement,
        keys: impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static,
        dismiss: impl Fn(&mut Window, &mut App) + 'static,
        window: &Window,
    ) -> Option<AnyElement> {
        let open = self.open.as_ref()?;
        kit.trap(open.screen);
        let shadow = t::SHADOW_POPOVER;
        let card = div()
            .track_focus(&open.focus)
            .occlude()
            .flex()
            .flex_col()
            .rounded(px(t::RADIUS_LARGE))
            .border(px(t::hairline(window.scale_factor())))
            .border_color(paint(colors.border, 1.0))
            .bg(paint(colors.card, 1.0))
            .shadow(vec![gpui::BoxShadow {
                color: paint(shadow.color(), 1.0).into(),
                offset: point(px(0.0), px(shadow.offset_y)),
                blur_radius: px(shadow.sigma()),
                spread_radius: px(0.0),
                inset: false,
            }])
            .on_key_down(move |event, window, cx| keys(event, window, cx))
            .on_mouse_down_out(move |_, window, cx| dismiss(window, cx))
            .child(content);
        // The measuring child is the wrapper's, which has no edge of its own, so it measures the card's outer box.
        let measured = div().relative().child(card).child(kit.record().popover(
            open.screen,
            open.opener.area.logical(),
            open.opener.control,
            open.opener.pane.clone(),
        ));
        let placed = Placed {
            opener: open.opener.area,
            placement: open.placement,
            faults: kit.record().faults(),
            child: measured.into_any_element(),
        };
        Some(deferred(placed).priority_auto().into_any_element())
    }
}

/// Lays its child out at its own size, then moves it to where [`place`] (or the centred placement) puts it, on whole
/// device pixels.
struct Placed {
    opener: Area,
    placement: Placement,
    faults: Faults,
    child: AnyElement,
}

impl Placed {
    /// The child's top-left corner in the window for its laid-out size.
    fn origin(&self, size: (f32, f32), window: (f32, f32)) -> (f32, f32) {
        if self.faults.popover_over_opener() {
            return (self.opener.x, self.opener.y);
        }
        match self.placement {
            Placement::Beside => {
                let (x, y, _) = place(self.opener, size, window);
                (x, y)
            }
            Placement::Centred { top } => (slide((window.0 - size.0) / 2.0, size.0, window.0), top),
        }
    }
}

impl IntoElement for Placed {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Placed {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let child = self.child.request_layout(window, cx);
        let style = Style {
            position: Position::Absolute,
            display: Display::Flex,
            ..Style::default()
        };
        (window.request_layout(style, [child], cx), child)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let laid_out = window.layout_bounds(*child);
        let viewport = window.viewport_size();
        let (x, y) = self.origin(
            (
                f32::from(laid_out.size.width),
                f32::from(laid_out.size.height),
            ),
            (f32::from(viewport.width), f32::from(viewport.height)),
        );
        let scale = window.scale_factor();
        let whole = |value: f32| (value * scale).round() / scale;
        let offset = point(px(whole(x)), px(whole(y))) - bounds.origin;
        window.with_element_offset(offset, |window| self.child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _child: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gate's window: 1200 x 800 logical px. Gap 4 and margin 8 are the tokens' values.
    const WINDOW: (f32, f32) = (1200.0, 800.0);

    fn area(x: f32, y: f32, w: f32, h: f32) -> Area {
        Area { x, y, w, h }
    }

    #[test]
    fn a_popover_goes_below_its_opener_when_it_fits() {
        assert_eq!(
            place(area(100.0, 10.0, 28.0, 28.0), (236.0, 100.0), WINDOW),
            (100.0, 42.0, Side::Below)
        );
    }

    #[test]
    fn with_no_room_below_it_goes_above() {
        // Below would start at 760 + 28 + 4 = 792, leaving 0 of the 100 it needs before the margin.
        assert_eq!(
            place(area(100.0, 760.0, 28.0, 28.0), (236.0, 100.0), WINDOW),
            (100.0, 656.0, Side::Above)
        );
    }

    #[test]
    fn near_the_right_edge_it_slides_left_to_stay_inside() {
        // 1200 - 8 - 236 = 956.
        assert_eq!(
            place(area(1180.0, 10.0, 10.0, 10.0), (236.0, 100.0), WINDOW),
            (956.0, 24.0, Side::Below)
        );
    }

    #[test]
    fn too_tall_for_above_or_below_it_goes_beside() {
        // Below has 800 - 8 - 412 = 380, above 380 - 4 - 8 = 368: neither holds 500. Right of 628 + 4 = 632 there is
        // 560; it slides up to 800 - 8 - 500 = 292.
        assert_eq!(
            place(area(600.0, 380.0, 28.0, 28.0), (200.0, 500.0), WINDOW),
            (632.0, 292.0, Side::Right)
        );
        // Against the right edge it goes left: 1150 - 4 - 200 = 946.
        assert_eq!(
            place(area(1150.0, 380.0, 28.0, 28.0), (200.0, 500.0), WINDOW),
            (946.0, 292.0, Side::Left)
        );
    }

    #[test]
    fn a_right_click_in_the_bottom_right_corner_opens_up_and_left() {
        // The point 1170, 760; the menu 236 x 140 fits neither below (800 - 8 - 764 = 28) nor right of it.
        let (x, y, side) = place(area(1170.0, 760.0, 0.0, 0.0), (236.0, 140.0), WINDOW);
        assert_eq!((x, y, side), (956.0, 616.0, Side::Above));
        assert!(y + 140.0 <= 760.0, "clear of the point it opened at");
    }

    #[test]
    fn a_popover_larger_than_the_window_is_kept_at_its_top_left_margin() {
        assert_eq!(
            place(area(600.0, 300.0, 28.0, 28.0), (1300.0, 900.0), WINDOW),
            (8.0, 8.0, Side::Below)
        );
    }
}
