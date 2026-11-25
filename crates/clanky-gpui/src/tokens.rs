//! The calm interface's design tokens: one table for every size, radius, spacing step, height, shadow
//! and motion value the window chrome, palette, menus and settings draw with.
//!
//! Sizes are logical pixels; screens multiply them by the window scale where they draw. Colours are
//! the theme's roles (`themes::Colors`: background, card, accent, foreground, muted_foreground,
//! border, input, err, ok, agent); this table only holds the few colour rules that are not theme
//! data, such as how the window frame is mixed from the card colour.
//!
//! The values follow Terax's interface (numbers and ideas only), with GitHub's Primer as the fallback
//! where a Terax size reads too small, and the design rulebook's amendments for the values its table left
//! open. Screens use these names, never a number of their own, so that neighbouring parts never end up a
//! pixel or two apart.
use clanky_state::themes::{Color, Colors, Theme};

// Text sizes. Inter for the interface, the terminal font for terminal text and code. The terminal
// keeps its own size from the config (16 px by default) and stays the largest text on screen.

/// File and list rows, recent folders.
pub const TEXT_TREE: f32 = 13.0;
/// Settings row titles and palette rows (weight 500).
pub const TEXT_TITLE: f32 = 12.5;
/// Tabs, menus, chips, buttons and the completion list.
pub const TEXT_UI: f32 = 12.0;
/// Settings section tabs.
pub const TEXT_TAB_SWITCH: f32 = 11.5;
/// The status line, small buttons and the palette footer.
pub const TEXT_STATUS: f32 = 11.0;
/// Settings descriptions, in the muted foreground.
pub const TEXT_DESCRIPTION: f32 = 10.5;
/// Block meta (folder, time, duration, exit code), with tabular digits.
pub const TEXT_META: f32 = 10.0;

/// Two weights for running text, and one heavier weight for the single title of a dialog.
pub const WEIGHT_REGULAR: u16 = 400;
pub const WEIGHT_MEDIUM: u16 = 500;
pub const WEIGHT_DIALOG_TITLE: u16 = 600;

// Corner radii. An item inside a container is one step smaller than the container.

/// Menu items, chips inside a bar, swatches.
pub const RADIUS_SMALL: f32 = 6.0;
/// Tabs, buttons in bars, fields, the block toolbar, command block cards.
pub const RADIUS_MEDIUM: f32 = 8.0;
/// Menus, the palette, settings cards, dialogs.
pub const RADIUS_LARGE: f32 = 10.0;
/// The window itself, where Clanky draws its own frame.
pub const RADIUS_WINDOW: f32 = 12.0;
/// Each control's corner, by the containment rule: buttons and icon buttons at every size, fields, list rows and
/// palette rows (inside large containers) are medium; a segment (inside the medium switcher pill) is small; a switch
/// is fully rounded.
pub const RADIUS_BUTTON: f32 = RADIUS_MEDIUM;
pub const RADIUS_FIELD: f32 = RADIUS_MEDIUM;
pub const RADIUS_ROW: f32 = RADIUS_MEDIUM;
pub const RADIUS_SEGMENT: f32 = RADIUS_SMALL;
pub const RADIUS_SWITCH: f32 = SWITCH_CONTROL_HEIGHT / 2.0;

// The terminal's text when the config sets none: the rulebook's terminal font at the size Marc kept.

pub const TERMINAL_FONT_FAMILY: &str = "JetBrains Mono";
pub const TERMINAL_FONT_SIZE: f32 = 16.0;
pub const TERMINAL_LINE_HEIGHT: f32 = 1.2;

// Spacing, on a 4 px grid with two half steps. Nothing else is used between parts.

pub const SPACE_2: f32 = 2.0;
pub const SPACE_4: f32 = 4.0;
pub const SPACE_6: f32 = 6.0;
pub const SPACE_8: f32 = 8.0;
pub const SPACE_12: f32 = 12.0;
pub const SPACE_16: f32 = 16.0;
pub const SPACE_24: f32 = 24.0;
pub const SPACE_32: f32 = 32.0;
/// Every allowed spacing step, smallest first.
pub const SPACING_STEPS: [f32; 8] = [
    SPACE_2, SPACE_4, SPACE_6, SPACE_8, SPACE_12, SPACE_16, SPACE_24, SPACE_32,
];

// Heights.

/// The window's title bar, which holds the tabs.
pub const TITLE_BAR_HEIGHT: f32 = 40.0;
/// A tab. The active tab is a pill of the foreground at `ACTIVE_TAB_FILL` with an inner ring at
/// `ACTIVE_TAB_RING`.
pub const TAB_HEIGHT: f32 = 28.0;
/// The widest a tab grows; a longer label ends in an ellipsis.
pub const TAB_MAX_WIDTH: f32 = 200.0;
/// List rows (files, recent folders).
pub const ROW_HEIGHT: f32 = 24.0;
/// Context menu and dropdown items.
pub const MENU_ITEM_HEIGHT: f32 = 28.0;
/// A menu's width. The rulebook gives none. The pane's longest row, "Start an agent here…" (Inter 12 px, 118.8 by the
/// font file's advances) with its icon and "Ctrl+Shift+G" (JetBrains Mono 12 px, 86.4), needs 263 with the popover's
/// padding and the row's padding and gaps; 272 is the next multiple of 8, so the label never ends in an ellipsis.
pub const MENU_WIDTH: f32 = 272.0;
/// Palette results.
pub const PALETTE_ROW_HEIGHT: f32 = 36.0;
/// The palette's search field.
pub const PALETTE_INPUT_HEIGHT: f32 = 44.0;
/// The status line and the palette footer.
pub const STATUS_HEIGHT: f32 = 32.0;
/// Buttons: small, in a bar, default, large. Buttons in bars are `BUTTON_HEIGHT_BAR`.
pub const BUTTON_HEIGHT_SMALL: f32 = 24.0;
pub const BUTTON_HEIGHT_BAR: f32 = 28.0;
pub const BUTTON_HEIGHT_DEFAULT: f32 = 32.0;
pub const BUTTON_HEIGHT_LARGE: f32 = 36.0;
/// Command bar chips.
pub const CHIP_HEIGHT: f32 = 28.0;
/// Icon buttons: as tall as bar buttons in bars.
pub const ICON_BUTTON_HEIGHT_BAR: f32 = BUTTON_HEIGHT_BAR;
/// A button inside another control (a tab's close): square at the smallest hit size (rulebook amendment,
/// 2026-11-17).
pub const ICON_BUTTON_INLINE_SIZE: f32 = MIN_HIT_SIZE;
/// The command block toolbar, and the size of each of its buttons.
pub const BLOCK_TOOLBAR_HEIGHT: f32 = 18.0;
pub const BLOCK_TOOLBAR_BUTTON_WIDTH: f32 = 22.0;
/// Dialog and settings headers.
pub const DIALOG_HEADER_HEIGHT: f32 = 44.0;
/// Text fields, at the default button height.
pub const FIELD_HEIGHT: f32 = BUTTON_HEIGHT_DEFAULT;
/// A segment of a segmented switcher: the switcher is a tab-high pill and holds its segments inset by
/// `SPACE_2` on every side.
pub const SEGMENT_HEIGHT: f32 = TAB_HEIGHT - 2.0 * SPACE_2;
/// The smallest box a pointer can hit: no control is smaller than this either way.
pub const MIN_HIT_SIZE: f32 = 24.0;
/// A switch control, a full pill: as tall as the smallest hit box, so what is drawn is what is hit.
pub const SWITCH_CONTROL_HEIGHT: f32 = MIN_HIT_SIZE;
pub const SWITCH_CONTROL_WIDTH: f32 = 40.0;

// Widths and the parts of a list row.

/// The side column (tools panel, side tabs).
pub const SIDE_COLUMN_WIDTH: f32 = 236.0;
/// The side column folded into a rail of icons: a row's icon with the column's padding and some room around it
/// (the side tabs mockup, `.side.rail`).
pub const SIDE_COLUMN_FOLDED_WIDTH: f32 = 52.0;
/// The `side-width` planted fault draws the side column this much wider than `SIDE_COLUMN_WIDTH`.
pub const SIDE_COLUMN_FAULT_EXTRA: f32 = 4.0;
/// A side tab row: two lines of text beside the session icon (the mockup's `.row`, `min-height: 48px`).
pub const SIDE_TAB_ROW_HEIGHT: f32 = 48.0;
/// The side column's filter field, as tall as a tab (the mockup's `.filter`).
pub const SIDE_FILTER_HEIGHT: f32 = TAB_HEIGHT;
/// The session icon of a side tab row: a rounded square holding the program's mark (the mockup's `.ico`).
pub const SESSION_ICON_SIZE: f32 = 28.0;
/// The mark's text inside the session icon, in the terminal font.
pub const SESSION_MARK_TEXT: f32 = TEXT_UI;
/// An agent's session icon is filled with the agent colour at this opacity (the mockup's `.ico.agent`).
pub const SESSION_AGENT_FILL: f32 = 0.14;
/// The state badge on the session icon's bottom-right corner: a circle this wide, its edge `BADGE_EDGE` of the
/// frame colour, standing out of the icon by `BADGE_OUTSET` (the mockup's `.badge`).
pub const BADGE_SIZE: f32 = 12.0;
pub const BADGE_EDGE: f32 = SPACE_2;
pub const BADGE_OUTSET: f32 = 3.0;
/// The notch that steps round a working badge: a square of the frame colour this wide.
pub const SPINNER_NOTCH_SIZE: f32 = SPACE_2;
/// How far a waiting badge's dot shrinks at the low point of its pulse, from each side.
pub const PULSE_SHRINK: f32 = 1.5;
/// A waiting badge pulses once in this many milliseconds (the mockup's `pulse 1.6s`).
pub const PULSE_PERIOD_MS: f32 = 1600.0;
/// The state dot before a top tab's title (the mockup's top-tab `.badge`, 8 px).
pub const TAB_DOT_SIZE: f32 = SPACE_8;
/// A side tab row's sub line (branch or folder) in the terminal font, and its icon.
pub const SIDE_SUB_TEXT: f32 = TEXT_DESCRIPTION;
pub const SIDE_SUB_ICON_SIZE: f32 = 11.0;
/// The state words at the sub line's right end.
pub const SIDE_STATE_TEXT: f32 = TEXT_META;
/// A group's name over its rows (the mockup's `.groupname`).
pub const SIDE_GROUP_TEXT: f32 = TEXT_DESCRIPTION;
/// The command palette.
pub const PALETTE_WIDTH: f32 = 560.0;
/// The palette's top edge: today's palette sits 24 px below the title bar.
pub const PALETTE_TOP: f32 = TITLE_BAR_HEIGHT + SPACE_24;
/// The most result rows the palette shows at once, as today's; the rest scroll into view with the selection.
pub const PALETTE_VISIBLE_ROWS: usize = 10;
/// The typing caret in the palette's search line: one logical pixel wide, as tall as today's.
pub const CARET_WIDTH: f32 = BORDER_WIDTH;
pub const PALETTE_CARET_HEIGHT: f32 = 16.0;
/// The title bar's search box, which opens the palette: today's width, at the field height.
pub const SEARCH_BOX_WIDTH: f32 = 320.0;
/// Panes sit flush, square-cornered, edge to edge with each other and with the panels around them; one hairline in
/// the border colour divides each from the next. The pointer grabs a divider anywhere in this width around it.
pub const DIVIDER_HIT_WIDTH: f32 = SPACE_8;
/// Inside the side column, around its header and list.
pub const SIDE_COLUMN_PADDING: f32 = SPACE_8;
/// Inside each pane, around the terminal grid.
pub const PANE_PADDING: f32 = SPACE_12;
/// A list row: indent per level, chevron, icon, and the gap between them.
pub const ROW_INDENT: f32 = SPACE_12;
pub const ROW_CHEVRON_SIZE: f32 = 12.0;
pub const ROW_ICON_SIZE: f32 = 16.0;
pub const ROW_GAP: f32 = SPACE_8;
/// Interface icons: one stroke family, 16 px in rows and 14 px in bars.
pub const ICON_STROKE_WIDTH: f32 = 1.75;
pub const ICON_SIZE_ROW: f32 = 16.0;
pub const ICON_SIZE_BAR: f32 = 14.0;
/// Pane edges, dividers and field outlines.
pub const BORDER_WIDTH: f32 = 1.0;

/// The width of a hairline (pane edges, dividers, the status line's top line) in logical pixels: one device pixel at
/// every scale, so it is never drawn two pixels wide at a fractional scale.
pub fn hairline(scale: f32) -> f32 {
    1.0 / scale.max(1.0)
}

// Settings window layout. Settings is a dialog over the window: a header with the section switcher,
// one narrow column of setting cards, and a footer.

pub const SETTINGS_COLUMN_MAX_WIDTH: f32 = 640.0;
pub const SETTINGS_PADDING_SIDE: f32 = SPACE_32;
pub const SETTINGS_PADDING_TOP: f32 = SPACE_24;
pub const SETTINGS_PADDING_BOTTOM: f32 = SPACE_24;
/// The dialog is exactly the column and its side padding wide.
pub const SETTINGS_PANEL_WIDTH: f32 = SETTINGS_COLUMN_MAX_WIDTH + 2.0 * SETTINGS_PADDING_SIDE;
/// Its height: this tall where the window has room, never shorter than the minimum (the page scrolls).
pub const SETTINGS_PANEL_MAX_HEIGHT: f32 = 600.0;
pub const SETTINGS_PANEL_MIN_HEIGHT: f32 = 300.0;
/// The room kept between the dialog and the window's edges.
pub const SETTINGS_PANEL_MARGIN: f32 = SPACE_12;
/// Each setting is a card: its padding across and down, inside its 1 px border.
pub const SETTINGS_CARD_PADDING_X: f32 = SPACE_12;
pub const SETTINGS_CARD_PADDING_Y: f32 = 10.0;
/// The setting card's fill and border are the card and border colours at this opacity. The dialog's
/// header uses the same fill and divider.
pub const SETTINGS_CARD_OPACITY: f32 = 0.6;
/// Between cards of one group, between a group's label and its first card, and between groups.
pub const SETTINGS_CARD_GAP: f32 = SPACE_8;
pub const SETTINGS_GROUP_GAP: f32 = SPACE_24;
/// Between a card's title and its description, and between its text and its control.
pub const SETTINGS_TITLE_GAP: f32 = SPACE_2;
pub const SETTINGS_CONTROL_GAP: f32 = SPACE_16;
/// The section switcher and segmented controls: a pill of the muted colour at this opacity, holding
/// small buttons inset by `SPACE_2`.
pub const SWITCHER_FILL_OPACITY: f32 = 0.4;
/// The dialog's corner: the window's own radius, as the dialog is a window of its own over the main one.
pub const SETTINGS_PANEL_RADIUS: f32 = RADIUS_WINDOW;
/// The header holding the section switcher, its bottom divider included.
pub const SETTINGS_HEADER_HEIGHT: f32 = DIALOG_HEADER_HEIGHT;
/// The section switcher's pill and a row's segment tray: tab high, the button radius, in the card colour.
pub const SETTINGS_TAB_PILL_HEIGHT: f32 = TAB_HEIGHT;
pub const SETTINGS_PILL_RADIUS: f32 = RADIUS_BUTTON;
/// A setting card's corner.
pub const SETTINGS_CARD_RADIUS: f32 = RADIUS_LARGE;
/// A row's field and slider, at the card's right end.
pub const SETTINGS_FIELD_WIDTH: f32 = 120.0;
pub const SETTINGS_SLIDER_WIDTH: f32 = 160.0;
/// One wheel line over the page body; a notch of the wheel is three lines on X11, so a notch scrolls 48.
pub const SETTINGS_SCROLL_LINE: f32 = 16.0;
/// The error banner at the dialog's bottom, as tall as the status line.
pub const SETTINGS_BANNER_HEIGHT: f32 = STATUS_HEIGHT;
/// How often the window looks at the settings file for a change made outside it (live reload).
pub const SETTINGS_POLL_MS: u64 = 500;
/// Inter's own line box, ascender plus descender (1984 + 494 units of its 2048 an em): Settings sets its titles,
/// descriptions and group labels in it, so a line's top is its ascender and no half leading moves the text down.
pub const INTER_LINE_SPAN: f32 = 2478.0 / 2048.0;

/// A slider (Opacity): as tall as a switch, a full pill, `SETTINGS_SLIDER_WIDTH` wide in a settings row.
pub const SLIDER_HEIGHT: f32 = MIN_HIT_SIZE;
pub const RADIUS_SLIDER: f32 = SLIDER_HEIGHT / 2.0;

// Colour rules that are not theme data. Colour carries meaning only (failure, agent, success);
// hierarchy comes from foreground against muted foreground and from opacity, never from a new grey.

/// The border and input roles are the foreground's opposite (white on dark, black on light) at
/// these opacities; the theme files hold the result.
pub const BORDER_OPACITY: f32 = 0.09;
pub const INPUT_OPACITY: f32 = 0.13;
/// The window frame behind the panes: the card colour moved this far toward the foreground.
pub const FRAME_MIX: f32 = 0.03;
/// The active tab pill: the foreground at this opacity, with an inner ring at `ACTIVE_TAB_RING`.
pub const ACTIVE_TAB_FILL: f32 = 0.07;
pub const ACTIVE_TAB_RING: f32 = 0.05;
/// A button inside another control: hover and pressed are the foreground at these opacities over whatever lies
/// under it, so they show on the active tab's pill too.
pub const INLINE_BUTTON_HOVER_FILL: f32 = 0.08;
pub const INLINE_BUTTON_PRESSED_FILL: f32 = 0.14;
/// The command block toolbar at rest; it is fully opaque under the pointer.
pub const BLOCK_TOOLBAR_REST_OPACITY: f32 = 0.72;
/// The line between command blocks: the foreground at this opacity, or `err` at
/// `BLOCK_DIVIDER_FAILED` after a failed command.
pub const BLOCK_DIVIDER: f32 = 0.08;
pub const BLOCK_DIVIDER_FAILED: f32 = 0.5;
/// The space between command blocks with the command input on (the mockup's `.blk` and `.meta` on the 4 px grid):
/// under a block's last row, then the divider, then over the next block's meta row, the meta row itself (the
/// rulebook's 18), and between the meta row and the command.
pub const BLOCK_SPACE_BELOW: f32 = SPACE_12;
pub const BLOCK_SPACE_ABOVE: f32 = SPACE_8;
pub const BLOCK_META_ROW_HEIGHT: f32 = 18.0;
pub const BLOCK_META_GAP: f32 = SPACE_2;

/// Keyboard focus: a ring this wide outside the control in the ring colour at this opacity, and the
/// control's own border in the ring colour.
pub const FOCUS_RING_WIDTH: f32 = SPACE_2;
pub const FOCUS_RING_OPACITY: f32 = 0.25;
/// A disabled control is drawn at this opacity.
pub const DISABLED_OPACITY: f32 = 0.5;

/// Fully transparent: the fill of a shape drawn only for its shadow.
pub const TRANSPARENT: Color = [0, 0, 0, 0];

/// The terminal's selection background: the theme's own, else its hover colour.
pub fn selection(theme: &Theme) -> Color {
    theme.selection.unwrap_or(theme.colors.accent)
}

/// The window frame colour for a theme.
pub fn frame(colors: &Colors) -> Color {
    let mix = |card: u8, foreground: u8| {
        (f32::from(card) + (f32::from(foreground) - f32::from(card)) * FRAME_MIX).round() as u8
    };
    let (card, foreground) = (colors.card, colors.foreground);
    [
        mix(card[0], foreground[0]),
        mix(card[1], foreground[1]),
        mix(card[2], foreground[2]),
        255,
    ]
}

// Shadows. Panes have none: only floating things do.

/// A drop shadow: how far down it falls, how far it blurs, and the opacity of its black.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowToken {
    pub offset_y: f32,
    pub blur: f32,
    pub opacity: f32,
}
impl ShadowToken {
    /// The shadow's colour: black at the token's opacity.
    pub fn color(&self) -> Color {
        [0, 0, 0, (self.opacity * 255.0).round() as u8]
    }

    /// The Gaussian's standard deviation, which is what gpui's `blur_radius` takes: a CSS blur radius, as the
    /// rulebook writes shadows, is twice the standard deviation.
    pub fn sigma(&self) -> f32 {
        self.blur / 2.0
    }
}
/// Menus, the palette, the completion list and dialogs.
pub const SHADOW_POPOVER: ShadowToken = ShadowToken {
    offset_y: 12.0,
    blur: 32.0,
    opacity: 0.35,
};
/// The command block toolbar.
pub const SHADOW_TOOLBAR: ShadowToken = ShadowToken {
    offset_y: 4.0,
    blur: 14.0,
    opacity: 0.28,
};

// Popovers: the one floating element under the palette, the menus, the agent picker, the completion list and the
// find bar. Its edge is `RADIUS_LARGE` with a hairline in `border` on `card`, its shadow `SHADOW_POPOVER`.

/// The room between a popover and what opened it: next to it, never touching or covering it.
pub const POPOVER_OPENER_GAP: f32 = SPACE_4;
/// How close a popover may come to the window's edge: one spacing step, as the old pane margin was, so a popover
/// never touches the window's rounded corner.
pub const POPOVER_WINDOW_MARGIN: f32 = SPACE_8;
/// The room between a popover's edge and its rows: the rows' radius is one step below the popover's.
pub const POPOVER_PADDING: f32 = SPACE_4;
/// The pane's find bar, a popover over the pane's top right corner: as wide as the find bar it replaces (calm
/// `scene.rs` `find_bar`, 360), with a field (`FIELD_HEIGHT`) and bar icon buttons (`ICON_BUTTON_HEIGHT_BAR`).
pub const FIND_BAR_WIDTH: f32 = 360.0;
/// How far the find bar sits in from the pane's top and right edges, as today's bar sits in from the right.
pub const FIND_BAR_INSET: f32 = SPACE_8;
/// Between the find bar's field and its buttons, and between the buttons.
pub const FIND_BAR_GAP: f32 = SPACE_4;
/// A field's caret: one device pixel wide, as tall as `text_ui` with a pixel of room above and below.
pub const CARET_HEIGHT: f32 = TEXT_UI + SPACE_2;

// Motion. Every animation pauses while the window is unfocused or hidden, and reduced motion turns
// them off, so an idle window draws no frames.

pub const MOTION_FAST_MS: f32 = 160.0;
pub const MOTION_MEDIUM_MS: f32 = 240.0;
pub const MOTION_SLOW_MS: f32 = 320.0;
/// The one easing curve, as the control points of a CSS cubic-bezier.
pub const EASING: [f32; 4] = [0.16, 1.0, 0.3, 1.0];
/// Spinners step 12 times a turn, 4 steps a second.
pub const SPINNER_STEPS_PER_TURN: u32 = 12;
pub const SPINNER_STEPS_PER_SECOND: u32 = 4;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphite_selects_with_its_own_colour_and_other_themes_with_accent() {
        let graphite =
            clanky_state::themes::builtin("graphite-dark").expect("graphite-dark is built in");
        assert_eq!(selection(&graphite), [0x39, 0x39, 0x39, 255]);
        let light =
            clanky_state::themes::builtin("graphite-light").expect("graphite-light is built in");
        assert_eq!(selection(&light), [0xce, 0xce, 0xce, 255]);
        let stone = clanky_state::themes::builtin("stone-dark").expect("stone-dark is built in");
        assert_eq!(selection(&stone), stone.colors.accent);
    }

    #[test]
    fn a_hairline_is_one_device_pixel() {
        assert_eq!(hairline(1.0) * 1.0, 1.0);
        assert_eq!(hairline(1.5) * 1.5, 1.0);
        assert_eq!(hairline(2.0) * 2.0, 1.0);
    }

    #[test]
    fn frame_mixes_card_toward_foreground() {
        // Graphite dark: card #1d1d1d (29), foreground #f7f7f7 (247); 29 + 0.03 x 218 = 35.54, so 36.
        let theme =
            clanky_state::themes::builtin("graphite-dark").expect("graphite-dark is built in");
        assert_eq!(frame(&theme.colors), [36, 36, 36, 255]);
    }
}
