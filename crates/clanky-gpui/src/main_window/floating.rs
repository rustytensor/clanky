//! The window's floating screens: the keys an open popover takes first, and what each screen draws inside it. Each
//! screen lives in a file of its own beside this one, opens the window's one popover under its own screen name
//! (`Popover::open`), and is drawn here by that name.

use clanky_state::keys::Chord;
use gpui::{AnyElement, Context, Window};

use super::MainWindow;

impl MainWindow {
    /// A key, before the window's own handling: an open popover's (Esc, Tab), then a floating screen's open key.
    /// True when it was taken.
    pub(super) fn floating_key(
        &mut self,
        chord: Chord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(taken) = self.completion_list_key(chord, window, cx) {
            return taken;
        }
        if self.popover.key(chord, &self.kit, window, cx) {
            cx.notify();
            return true;
        }
        #[cfg(feature = "test-popover")]
        if self.test_popover_key(chord, window, cx) {
            return true;
        }
        if self.find_bar_key(chord, window, cx) {
            return true;
        }
        if self.palette_key(chord, window, cx) {
            return true;
        }
        if self.agent_picker_key(chord, window, cx) {
            return true;
        }
        false
    }

    /// Closes the open popover, giving the keyboard back to what had it when it opened.
    pub(super) fn close_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.popover.close(&self.kit, window, cx) {
            cx.notify();
        }
    }

    /// The open popover with its screen's content; None while none is open.
    pub(super) fn floating_layer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.find_bar_frame(cx);
        let screen = self.popover.screen()?;
        #[cfg(feature = "test-popover")]
        if super::test_popover::owns(screen) {
            return self.test_popover(screen, window, cx);
        }
        if screen == "find-bar" {
            return self.find_bar(window, cx);
        }
        if screen == super::completion_list::LIST_SCREEN {
            return self.completion_list(window, cx);
        }
        if screen == "palette" {
            return self.command_palette(window, cx);
        }
        if screen == super::context_menu::SCREEN {
            return self.context_menu(window, cx);
        }
        if screen == "agent-picker" {
            return self.agent_picker(window, cx);
        }
        // A screen with no content here would hold the keyboard with nothing drawn: it is closed instead.
        eprintln!("the popover {screen} has nothing to draw; closing it");
        self.close_popover(window, cx);
        None
    }
}
