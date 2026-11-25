//! Settings in the main window. The title bar's last button, Ctrl+, and the palette's "Settings" open it; the button
//! and Ctrl+, close it again, and so does Escape, which gives the keyboard back to the pane that had it. While it is
//! open its keys come first. Every change it writes is applied to the window at once, and the settings file is
//! looked at every `SETTINGS_POLL_MS` so an edit made outside the window shows too (live reload, as today).

use std::rc::Rc;

use clanky::config::Config;
use clanky_state::keys::{Action, action_for};
use gpui::{AnyElement, Context, KeyDownEvent, Window};

use super::{MainWindow, SCREEN as MAIN_SCREEN, chord_of, interface_theme};
use crate::{
    controls::{Control, Kind},
    icons::Icon,
    settings::{Changes, Host, KeyOutcome, Request},
};

/// The title bar's Settings button, right of the search box.
pub(super) const SETTINGS_BUTTON_ID: u32 = 3;

impl MainWindow {
    /// The title bar's Settings button: it opens Settings, or closes it while it is open.
    pub(super) fn settings_button(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let this = cx.entity();
        let dead = self.kit.record().faults().settings_button_dead();
        let button = Control::new(Kind::IconButton, SETTINGS_BUTTON_ID, "Settings")
            .icon(Icon::Settings)
            .checked(self.settings.is_open())
            .on_click(move |window, cx| {
                if !dead {
                    this.update(cx, |this, cx| this.toggle_settings(window, cx));
                }
            });
        self.kit.build(MAIN_SCREEN, button, window, cx)
    }

    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.open(window, cx);
        cx.notify();
    }

    pub(super) fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.close(window, cx);
        cx.notify();
    }

    pub(super) fn toggle_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.is_open() {
            self.close_settings(window, cx);
        } else {
            self.open_settings(window, cx);
        }
    }

    /// A key while Settings is open and no popover is: Ctrl+, closes it, then Settings' own keys. True when taken.
    pub(super) fn settings_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.settings.is_open() || self.popover.is_open() {
            return false;
        }
        let typed = event.keystroke.key_char.as_deref();
        let Some(chord) = chord_of(&event.keystroke) else {
            let modifiers = &event.keystroke.modifiers;
            let taken = !modifiers.control
                && !modifiers.alt
                && typed.is_some_and(|typed| self.settings.typed(typed, window));
            if taken {
                cx.notify();
            }
            return taken;
        };
        if action_for(chord) == Some(Action::ToggleSettings) {
            self.close_settings(window, cx);
            return true;
        }
        match self.settings.key(chord, typed, &self.kit, window, cx) {
            KeyOutcome::Ignored => false,
            KeyOutcome::Taken => {
                cx.notify();
                true
            }
            KeyOutcome::Close => {
                self.close_settings(window, cx);
                true
            }
            KeyOutcome::Store(changes) => {
                self.store_settings(&changes, cx);
                true
            }
        }
    }

    /// Writes settings through the store and applies them to the window; a failure goes to Settings' banner.
    fn store_settings(&mut self, changes: &Changes, cx: &mut Context<Self>) {
        if let Some((before, after)) = self.settings.store(changes) {
            self.apply_config(&before, &after, cx);
        }
        cx.notify();
    }

    /// What a control of Settings asked for.
    fn settings_request(&mut self, request: Request, window: &mut Window, cx: &mut Context<Self>) {
        match request {
            Request::Page(page) => self.settings.show_page(page, window, cx),
            Request::Store(changes) => self.store_settings(&changes, cx),
            Request::Edit(row) => self.settings.begin_edit(row),
            Request::Slide(row) => {
                if let Some(changes) = self.settings.slide(row, &self.kit, window) {
                    self.store_settings(&changes, cx);
                }
            }
            Request::Scroll(delta) => self.settings.scroll_by(delta, window.scale_factor()),
        }
        cx.notify();
    }

    /// The Settings window while it is open, drawn over the window; an edit whose field lost the keyboard is
    /// committed first.
    pub(super) fn settings_layer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if let Some(changes) = self.settings.frame(window) {
            self.store_settings(&changes, cx);
        }
        let this = cx.entity();
        let host: Host = Rc::new(move |request, window, cx| {
            this.update(cx, |this, cx| this.settings_request(request, window, cx));
        });
        self.settings
            .render(&self.kit, &self.colors, &host, window, cx)
    }

    /// The settings file, looked at for a change made outside the window: new settings are applied, and Settings
    /// is drawn again when what it shows changed.
    pub(super) fn settings_poll(&mut self, cx: &mut Context<Self>) {
        let (changed, shown) = self.settings.poll();
        if let Some((before, after)) = &changed {
            self.apply_config(before, after, cx);
        }
        if changed.is_some() || shown {
            cx.notify();
        }
    }

    /// Applies new settings to the open window, as today's window does live: the tabs along the top or down the
    /// side and their grouping, the tools panel, animations, and the interface theme of the window's chrome.
    pub(super) fn apply_config(&mut self, before: &Config, after: &Config, cx: &mut Context<Self>) {
        if after.tab_style != self.tab_style {
            self.tab_style = after.tab_style;
            self.side.folded = false;
        }
        self.tab_groups = after.tab_groups;
        if after.tools_panel != before.tools_panel && after.tools_panel != self.tools.open {
            self.toggle_tools(cx);
        }
        if after.animations != before.animations {
            self.kit.set_motion(after.animations);
        }
        let theme_changed = after.theme != before.theme
            || after.theme_dark != before.theme_dark
            || after.theme_light != before.theme_light;
        if theme_changed {
            let named = self.settings.keys_set();
            match interface_theme(after, &|name| named.iter().any(|set| set == name)) {
                Ok(theme) => {
                    self.colors = theme.colors.clone();
                    self.kit.set_colors(theme.colors);
                }
                Err(message) => self.notice = Some(message),
            }
        }
        cx.notify();
    }
}
