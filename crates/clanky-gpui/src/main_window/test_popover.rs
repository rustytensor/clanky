//! A test popover, built only with the `test-popover` feature, so the gates can check what every floating screen
//! shares (placement, staying clear of the opener, the keyboard kept inside, Esc giving the keyboard back) before the
//! real screens exist. Ctrl+Shift+F11 opens it next to the control that has the keyboard, or next to the tools panel
//! button when the pane has it; Ctrl+Shift+F12 opens it from a point near the window's bottom-right corner, as a
//! right-click there would, where it has to go up and to the left.

use clanky_state::{
    keys::{Chord, Key, Mods},
    list_selection::ListKey,
};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, point, px};

use super::{MainWindow, SCREEN, TOOLS_TOGGLE_ID, chord_of};
use crate::{
    controls::{Control, Group, Kind},
    popover::{Opener, Placement},
    tokens as t,
};

/// The popover's screen name when opened from a control.
const BESIDE: &str = "test-popover";
/// Its screen name when opened from the bottom-right point.
const CORNER: &str = "test-popover-corner";
const ROWS: [(u32, &str); 3] = [(1, "First"), (2, "Second"), (3, "Third")];
const GROUP: Group = Group::vertical("test-popover-rows");

/// Whether a popover screen is this one.
pub(super) fn owns(screen: &str) -> bool {
    screen == BESIDE || screen == CORNER
}

impl MainWindow {
    /// Ctrl+Shift+F11 and Ctrl+Shift+F12 open the test popover. True when the key was one of them.
    pub(super) fn test_popover_key(
        &mut self,
        chord: Chord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if chord.mods != Mods::CTRL_SHIFT {
            return false;
        }
        let pane = self.focused_pane(window, cx);
        let (screen, opener) = match chord.key {
            Key::F(11) => {
                let opener = Opener::focused_control(&self.kit, window).or_else(|| {
                    Opener::control(&self.kit, SCREEN, TOOLS_TOGGLE_ID).map(|opener| Opener {
                        pane: pane.clone(),
                        control: None,
                        ..opener
                    })
                });
                (BESIDE, opener)
            }
            Key::F(12) => {
                let viewport = window.viewport_size();
                let at = point(
                    viewport.width - px(t::SPACE_32),
                    viewport.height - px(t::STATUS_HEIGHT + t::SPACE_32),
                );
                (CORNER, pane.map(|pane| Opener::point(at, pane)))
            }
            _ => return false,
        };
        if let Some(opener) = opener {
            self.popover
                .open(screen, opener, Placement::Beside, window, cx);
            self.popover.resize_list(ROWS.len());
            cx.notify();
        }
        true
    }

    /// The test popover: three menu items; any of them closes it. While the popover itself has the keyboard (no row
    /// has it), the arrows, Home and End move the list's selection, drawn as the selected row, and Enter picks it.
    pub(super) fn test_popover(
        &mut self,
        screen: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let mut rows = div()
            .flex()
            .flex_col()
            .w(px(t::SIDE_COLUMN_WIDTH))
            .p(px(t::POPOVER_PADDING));
        let selected = self
            .popover
            .selected_row()
            .filter(|_| self.popover.holds_keyboard(window));
        for (index, (id, label)) in ROWS.into_iter().enumerate() {
            let this = cx.entity();
            let row = Control::new(Kind::MenuItem, id, label)
                .group(GROUP)
                .checked(selected == Some(index))
                .on_click(move |window, cx| {
                    this.update(cx, |this, cx| this.close_popover(window, cx));
                });
            rows = rows.child(self.kit.build(screen, row, window, cx));
        }
        let keys = cx.entity();
        let closer = cx.entity();
        self.popover.element(
            &self.kit,
            &self.colors,
            rows.into_any_element(),
            move |event, window, cx| {
                let Some(chord) = chord_of(&event.keystroke) else {
                    return;
                };
                keys.update(cx, |this, cx| match this.popover.list_key(chord) {
                    ListKey::Moved => {
                        cx.stop_propagation();
                        cx.notify();
                    }
                    ListKey::Pick(_) => {
                        cx.stop_propagation();
                        this.close_popover(window, cx);
                    }
                    ListKey::Ignored => {}
                });
            },
            move |window, cx| closer.update(cx, |this, cx| this.close_popover(window, cx)),
            window,
        )
    }
}
