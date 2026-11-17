//! How a control looks: each kind's height, radius and sizes, the state it is drawn in, and the fill and edge of each
//! state, from the token table and the design rulebook's states table with its amendments. `controls` builds the
//! elements from these.

use clanky_state::themes::{Color, Colors};
use gpui::{Hsla, rgb_to_hsla, rgba};

use crate::tokens as t;

/// What a control is. Each kind has one height, radius, text size and icon size, all from the token table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    ButtonSmall,
    ButtonBar,
    Button,
    ButtonLarge,
    IconButton,
    /// A small icon button inside another control (a tab's close).
    IconButtonInline,
    Chip,
    Tab,
    Switch,
    Field,
    MenuItem,
    ListRow,
    /// A search-and-pick list's row: the palette's results, the agent picker's tools.
    PaletteRow,
    Segment,
}

/// How a kind looks at rest: no fill or border of its own, an outline, a text field, or a switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Look {
    Ghost,
    Outlined,
    Field,
    Switch,
}

impl Kind {
    /// The kind's name in the control record.
    pub fn name(self) -> &'static str {
        match self {
            Kind::ButtonSmall => "button-small",
            Kind::ButtonBar => "button-bar",
            Kind::Button => "button",
            Kind::ButtonLarge => "button-large",
            Kind::IconButton => "icon-button",
            Kind::IconButtonInline => "icon-button-inline",
            Kind::Chip => "chip",
            Kind::Tab => "tab",
            Kind::Switch => "switch",
            Kind::Field => "field",
            Kind::MenuItem => "menu-item",
            Kind::ListRow => "list-row",
            Kind::PaletteRow => "palette-row",
            Kind::Segment => "segment",
        }
    }

    pub fn height(self) -> f32 {
        match self {
            Kind::ButtonSmall => t::BUTTON_HEIGHT_SMALL,
            Kind::ButtonBar => t::BUTTON_HEIGHT_BAR,
            Kind::Button => t::BUTTON_HEIGHT_DEFAULT,
            Kind::ButtonLarge => t::BUTTON_HEIGHT_LARGE,
            Kind::IconButton => t::ICON_BUTTON_HEIGHT_BAR,
            Kind::IconButtonInline => t::ICON_BUTTON_INLINE_SIZE,
            Kind::Chip => t::CHIP_HEIGHT,
            Kind::Tab => t::TAB_HEIGHT,
            Kind::Switch => t::SWITCH_CONTROL_HEIGHT,
            Kind::Field => t::FIELD_HEIGHT,
            Kind::MenuItem => t::MENU_ITEM_HEIGHT,
            Kind::ListRow => t::ROW_HEIGHT,
            Kind::PaletteRow => t::PALETTE_ROW_HEIGHT,
            Kind::Segment => t::SEGMENT_HEIGHT,
        }
    }

    pub fn radius(self) -> f32 {
        match self {
            Kind::ButtonSmall
            | Kind::ButtonBar
            | Kind::Button
            | Kind::ButtonLarge
            | Kind::IconButton => t::RADIUS_BUTTON,
            Kind::Tab => t::RADIUS_MEDIUM,
            Kind::Field => t::RADIUS_FIELD,
            Kind::ListRow | Kind::PaletteRow => t::RADIUS_ROW,
            Kind::Chip | Kind::MenuItem | Kind::IconButtonInline => t::RADIUS_SMALL,
            Kind::Segment => t::RADIUS_SEGMENT,
            Kind::Switch => t::RADIUS_SWITCH,
        }
    }

    /// The label's text size.
    pub fn text_size(self) -> f32 {
        match self {
            Kind::ButtonSmall => t::TEXT_STATUS,
            Kind::ListRow => t::TEXT_TREE,
            Kind::PaletteRow => t::TEXT_TITLE,
            Kind::Segment => t::TEXT_TAB_SWITCH,
            _ => t::TEXT_UI,
        }
    }

    /// The icon's box: rows hold the row icon, everything else sits in a bar.
    pub fn icon_size(self) -> f32 {
        if self.is_row() {
            t::ICON_SIZE_ROW
        } else {
            t::ICON_SIZE_BAR
        }
    }

    /// The rows of a menu or a list: their content starts at the left, and keyboard focus takes the hover fill too,
    /// so the keyboard's place reads the same as the pointer's.
    pub fn is_row(self) -> bool {
        matches!(self, Kind::MenuItem | Kind::ListRow | Kind::PaletteRow)
    }

    /// Whether a pointer click gives the control the keyboard. Only a field (to type in) and a menu item (inside a
    /// popover, which holds the keyboard while open) take it; every other control leaves it with the pane, so a
    /// click on a tab or a bar button never stops typing from reaching the terminal. Tab still reaches them all.
    pub fn takes_focus_on_click(self) -> bool {
        matches!(self, Kind::Field | Kind::MenuItem)
    }

    pub fn is_icon_only(self) -> bool {
        matches!(self, Kind::IconButton | Kind::IconButtonInline)
    }

    fn look(self) -> Look {
        match self {
            Kind::ButtonSmall | Kind::Button | Kind::ButtonLarge | Kind::Chip => Look::Outlined,
            Kind::Field => Look::Field,
            Kind::Switch => Look::Switch,
            _ => Look::Ghost,
        }
    }
}

/// The state a control is drawn in, strongest first: a disabled control is disabled whatever else happens to it,
/// then pressed, keyboard focus, hover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Rest,
    Hover,
    Pressed,
    Focus,
    Disabled,
}

/// Where the pointer is, for a control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pointer {
    Away,
    Over,
    /// Over it, with the left button held since it went down on it.
    PressingOver,
}

impl State {
    pub fn of(enabled: bool, pointer: Pointer, keyboard_focus: bool) -> Self {
        if !enabled {
            State::Disabled
        } else if pointer == Pointer::PressingOver {
            State::Pressed
        } else if keyboard_focus {
            State::Focus
        } else if pointer == Pointer::Over {
            State::Hover
        } else {
            State::Rest
        }
    }
}

/// The fill of a control in a state, or none where the container shows through. Hover's fill eases in; the caller
/// applies the ease.
pub fn fill(
    kind: Kind,
    state: State,
    primary: bool,
    checked: Option<bool>,
    colors: &Colors,
) -> Option<Color> {
    let active = checked == Some(true);
    if primary {
        return Some(match state {
            State::Hover | State::Pressed => colors.primary_pressed,
            _ => colors.primary,
        });
    }
    match (kind.look(), state) {
        (Look::Switch, _) => Some(if active {
            colors.primary
        } else {
            colors.accent
        }),
        (Look::Field, _) => None,
        // A button inside another control tints whatever lies under it.
        (_, State::Pressed) if kind == Kind::IconButtonInline => {
            Some(with_alpha(colors.foreground, t::INLINE_BUTTON_PRESSED_FILL))
        }
        (_, State::Hover) if kind == Kind::IconButtonInline => {
            Some(with_alpha(colors.foreground, t::INLINE_BUTTON_HOVER_FILL))
        }
        (_, State::Pressed) => Some(colors.pressed),
        // The selected segment stays lifted under the pointer: hover's fill on the switcher's tray would hide which one
        // is selected.
        (_, State::Hover | State::Focus) if kind == Kind::Segment && active => {
            Some(colors.background)
        }
        (_, State::Hover) => Some(colors.accent),
        (_, State::Focus) if kind.is_row() => Some(colors.accent),
        // A list's selected row, while the keyboard stays in the list's field or popover.
        _ if kind.is_row() && active => Some(colors.accent),
        _ if kind == Kind::Tab && active => Some(with_alpha(colors.foreground, t::ACTIVE_TAB_FILL)),
        _ if kind == Kind::Segment && active => Some(colors.background),
        _ => None,
    }
}

/// The control's own one-pixel edge in a state, or none.
pub fn border(kind: Kind, state: State, checked: Option<bool>, colors: &Colors) -> Option<Color> {
    if state == State::Focus {
        Some(colors.ring)
    } else if kind == Kind::Tab && checked == Some(true) {
        Some(with_alpha(colors.foreground, t::ACTIVE_TAB_RING))
    } else if matches!(kind.look(), Look::Outlined | Look::Field | Look::Switch) {
        Some(colors.input)
    } else {
        None
    }
}

/// The ring's width in logical pixels: two logical pixels, rounded to whole device pixels.
pub fn ring_width(scale: f32) -> f32 {
    (t::FOCUS_RING_WIDTH * scale).round() / scale
}

/// How far hover's fill has come in after `elapsed_ms`: the rulebook's one curve over the fast motion time, or all
/// the way at once with animations off.
pub fn hover_progress(elapsed_ms: f32, motion: bool) -> f32 {
    if !motion || elapsed_ms >= t::MOTION_FAST_MS {
        return 1.0;
    }
    ease(elapsed_ms.max(0.0) / t::MOTION_FAST_MS)
}

/// The rulebook's easing curve, a CSS cubic-bezier through `EASING`'s control points, at `time` from 0 to 1.
pub fn ease(time: f32) -> f32 {
    let [x1, y1, x2, y2] = t::EASING;
    let bezier = |at: f32, first: f32, second: f32| {
        let rest = 1.0 - at;
        3.0 * rest * rest * at * first + 3.0 * rest * at * at * second + at * at * at
    };
    // The curve's x grows with its parameter, so halving finds the parameter whose x is `time`.
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for _ in 0..32 {
        let middle = f32::midpoint(low, high);
        if bezier(middle, x1, x2) < time {
            low = middle;
        } else {
            high = middle;
        }
    }
    bezier(f32::midpoint(low, high), y1, y2)
}

pub fn with_alpha(color: Color, alpha: f32) -> Color {
    [
        color[0],
        color[1],
        color[2],
        (f32::from(color[3]) * alpha).round() as u8,
    ]
}

/// A theme colour for gpui, at `alpha` times its own opacity.
pub fn paint(color: Color, alpha: f32) -> Hsla {
    let [red, green, blue, opacity] = with_alpha(color, alpha);
    rgb_to_hsla(rgba(
        u32::from(red) << 24 | u32::from(green) << 16 | u32::from(blue) << 8 | u32::from(opacity),
    ))
}

#[cfg(test)]
// The values compared are exact in binary (whole numbers, and the ends of the ease, which return 1.0 as such).
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    fn graphite() -> Colors {
        clanky_state::themes::builtin("graphite-dark")
            .expect("graphite-dark is built in")
            .colors
    }

    fn grey(level: u8) -> Color {
        [level, level, level, 255]
    }

    /// Height and radius per kind, copied from `scripts/window-chrome/rulebook-tokens.json` (the rulebook's table and
    /// its amendments), not from the token module.
    #[test]
    fn each_kind_has_the_rulebooks_height_and_radius() {
        let expected = [
            (Kind::ButtonSmall, "button-small", 24.0, 8.0),
            (Kind::ButtonBar, "button-bar", 28.0, 8.0),
            (Kind::Button, "button", 32.0, 8.0),
            (Kind::ButtonLarge, "button-large", 36.0, 8.0),
            (Kind::IconButton, "icon-button", 28.0, 8.0),
            (Kind::IconButtonInline, "icon-button-inline", 24.0, 6.0),
            (Kind::Chip, "chip", 28.0, 6.0),
            (Kind::Tab, "tab", 28.0, 8.0),
            (Kind::Switch, "switch", 24.0, 12.0),
            (Kind::Field, "field", 32.0, 8.0),
            (Kind::MenuItem, "menu-item", 28.0, 6.0),
            (Kind::ListRow, "list-row", 24.0, 8.0),
            (Kind::PaletteRow, "palette-row", 36.0, 8.0),
            (Kind::Segment, "segment", 24.0, 6.0),
        ];
        for (kind, name, height, radius) in expected {
            assert_eq!(
                (kind.name(), kind.height(), kind.radius()),
                (name, height, radius)
            );
        }
        // Text and icon sizes from the same file: rows 13 px text and 16 px icons, small buttons 11, segments 11.5.
        assert_eq!(
            (Kind::ListRow.text_size(), Kind::ListRow.icon_size()),
            (13.0, 16.0)
        );
        assert_eq!(
            (Kind::MenuItem.text_size(), Kind::MenuItem.icon_size()),
            (12.0, 16.0)
        );
        assert_eq!(
            (Kind::PaletteRow.text_size(), Kind::PaletteRow.icon_size()),
            (12.5, 16.0)
        );
        assert_eq!(
            (Kind::ButtonSmall.text_size(), Kind::Chip.icon_size()),
            (11.0, 14.0)
        );
        assert_eq!(Kind::Segment.text_size(), 11.5);
    }

    /// Fills per state on Graphite dark, from the rulebook's states table and the theme file's hex values.
    #[test]
    fn fills_follow_the_states_table() {
        let colors = graphite();
        let (accent, pressed) = (grey(0x2a), grey(0x34));
        let bar = |state| fill(Kind::ButtonBar, state, false, None, &colors);
        assert_eq!(bar(State::Rest), None);
        assert_eq!(bar(State::Hover), Some(accent));
        assert_eq!(bar(State::Pressed), Some(pressed));
        assert_eq!(bar(State::Focus), None);
        // Outlined buttons take the same state fills over their outline.
        assert_eq!(
            fill(Kind::Button, State::Hover, false, None, &colors),
            Some(accent)
        );
        // Rows take the hover fill under keyboard focus too.
        assert_eq!(
            fill(Kind::MenuItem, State::Focus, false, None, &colors),
            Some(accent)
        );
        // The primary button: primary at rest, primary_pressed on hover and press.
        assert_eq!(
            fill(Kind::Button, State::Rest, true, None, &colors),
            Some(grey(0xf7))
        );
        assert_eq!(
            fill(Kind::Button, State::Hover, true, None, &colors),
            Some(grey(0xe8))
        );
        // A field and a switch keep one look in every state.
        for state in [State::Rest, State::Hover, State::Pressed, State::Focus] {
            assert_eq!(fill(Kind::Field, state, false, None, &colors), None);
            assert_eq!(
                fill(Kind::Switch, state, false, Some(true), &colors),
                Some(grey(0xf7))
            );
            assert_eq!(
                fill(Kind::Switch, state, false, Some(false), &colors),
                Some(accent)
            );
        }
        // The active tab: the foreground at 7 % (0.07 x 255 = 17.85, so 18), with a 5 % ring (12.75, so 13).
        assert_eq!(
            fill(Kind::Tab, State::Rest, false, Some(true), &colors),
            Some([0xf7, 0xf7, 0xf7, 18])
        );
        assert_eq!(
            border(Kind::Tab, State::Rest, Some(true), &colors),
            Some([0xf7, 0xf7, 0xf7, 13])
        );
        // A disabled control keeps its rest fill.
        assert_eq!(bar(State::Disabled), None);
        // The selected segment keeps its lift under the pointer; the others take hover's fill.
        assert_eq!(
            fill(Kind::Segment, State::Hover, false, Some(true), &colors),
            Some(grey(0x14))
        );
        assert_eq!(
            fill(Kind::Segment, State::Hover, false, Some(false), &colors),
            Some(accent)
        );
    }

    #[test]
    fn borders_follow_the_states_table() {
        let colors = graphite();
        let ring = grey(0x73);
        assert_eq!(border(Kind::ButtonBar, State::Rest, None, &colors), None);
        assert_eq!(
            border(Kind::ButtonBar, State::Focus, None, &colors),
            Some(ring)
        );
        // White at 13 %: #ffffff21.
        let input = Some([0xff, 0xff, 0xff, 0x21]);
        assert_eq!(border(Kind::Button, State::Hover, None, &colors), input);
        assert_eq!(border(Kind::Field, State::Pressed, None, &colors), input);
        assert_eq!(
            border(Kind::Switch, State::Rest, Some(true), &colors),
            input
        );
    }

    #[test]
    fn only_fields_and_menu_items_take_focus_on_click() {
        let all = [
            Kind::ButtonSmall,
            Kind::ButtonBar,
            Kind::Button,
            Kind::ButtonLarge,
            Kind::IconButton,
            Kind::IconButtonInline,
            Kind::Chip,
            Kind::Tab,
            Kind::Switch,
            Kind::Field,
            Kind::MenuItem,
            Kind::ListRow,
            Kind::PaletteRow,
            Kind::Segment,
        ];
        let taking: Vec<Kind> = all
            .into_iter()
            .filter(|kind| kind.takes_focus_on_click())
            .collect();
        assert_eq!(taking, [Kind::Field, Kind::MenuItem]);
    }

    #[test]
    fn state_precedence() {
        assert_eq!(
            State::of(false, Pointer::PressingOver, true),
            State::Disabled
        );
        assert_eq!(State::of(true, Pointer::PressingOver, true), State::Pressed);
        // Pressed and then moved out is `Away`: not drawn pressed.
        assert_eq!(State::of(true, Pointer::Away, false), State::Rest);
        assert_eq!(State::of(true, Pointer::Over, true), State::Focus);
        assert_eq!(State::of(true, Pointer::Over, false), State::Hover);
    }

    #[test]
    fn the_focus_ring_is_whole_device_pixels() {
        // Two logical pixels, rounded in device pixels: 2 at scale 1, 3 at 1.5, 4 at 2.
        for (scale, device) in [(1.0, 2.0), (1.5, 3.0), (2.0, 4.0)] {
            assert_eq!(ring_width(scale) * scale, device);
        }
    }

    #[test]
    fn hover_eases_in_over_the_fast_motion_time() {
        assert!(hover_progress(0.0, true) < 0.001);
        assert_eq!(hover_progress(160.0, true), 1.0);
        assert_eq!(hover_progress(0.0, false), 1.0);
        // cubic-bezier(0.16, 1, 0.3, 1) is past 90 % of the way by a third of the time, and never goes back.
        assert!(hover_progress(160.0 / 3.0, true) > 0.9);
        let mut last = 0.0;
        for step in 0..=16 {
            let now = hover_progress(step as f32 * 10.0, true);
            assert!(now >= last && now <= 1.0);
            last = now;
        }
    }
}
