//! The pane's find bar: Ctrl+Shift+F opens it over the focused pane's top right corner, a popover holding a field
//! ("Find in this pane", the match count at its right end) and three icon buttons: previous match, next match and
//! close. Typing looks for the text in the pane's history and screen and marks every match, the current one in the
//! selection colour; Enter goes to the next match, Shift+Enter to the previous one; Esc closes the bar, takes the
//! marks away and gives the keyboard back to the pane. The search itself is `clanky_state::pane_find`.

use clanky_state::{
    keys::{Action, Key, action_for},
    pane_find::PaneFind,
};
use gpui::{
    AnyElement, Context, FocusHandle, IntoElement, KeyDownEvent, ParentElement, Styled, Window,
    div, point, px,
};

use super::{MainWindow, chord_of};
use crate::{
    control_look::paint,
    controls::{Control, Group, Kind},
    icons::Icon,
    popover::{Opener, Placement},
    tokens as t,
};

/// The bar's screen name in the control record.
const FIND_BAR: &str = "find-bar";
const FIELD_ID: u32 = 1;
const PREVIOUS_ID: u32 = 2;
const NEXT_ID: u32 = 3;
const CLOSE_ID: u32 = 4;
const BUTTONS: Group = Group::horizontal("find");
const PLACEHOLDER: &str = "Find in this pane";

/// The open bar: what it looks for, in which pane, and its field's focus.
pub(super) struct FindBar {
    model: PaneFind,
    field: FocusHandle,
}

impl MainWindow {
    /// Ctrl+Shift+F opens the bar on the focused pane. True when the key was it.
    pub(super) fn find_bar_key(
        &mut self,
        chord: clanky_state::keys::Chord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if action_for(chord) != Some(Action::FindInScrollback) {
            return false;
        }
        if self.popover.screen() == Some(FIND_BAR) {
            return true;
        }
        let Some(pane) = self.focused_pane(window, cx) else {
            return true;
        };
        let Some([x, y, w, _]) = self
            .panes
            .get(&pane)
            .and_then(|(entity, _)| entity.read(cx).grid_area())
        else {
            return true;
        };
        // The point the bar opens below: its left edge, the gap above its top.
        let at = point(
            px(x + w - t::FIND_BAR_INSET - t::FIND_BAR_WIDTH),
            px(y + t::FIND_BAR_INSET - t::POPOVER_OPENER_GAP),
        );
        self.popover.open(
            FIND_BAR,
            Opener::point(at, pane.clone()),
            Placement::Beside,
            window,
            cx,
        );
        self.find = Some(FindBar {
            model: PaneFind::new(pane),
            field: cx.focus_handle(),
        });
        cx.notify();
        true
    }

    /// Called every frame: the bar's landmarks are empty unless drawn, and a bar closed some other way (Esc, a click
    /// outside, another popover) takes its marks off the pane.
    pub(super) fn find_bar_frame(&mut self, cx: &mut Context<Self>) {
        let record = self.kit.record();
        record.landmark_absent("find-match", 0);
        record.landmark_absent("find-count", 0);
        if self.popover.screen() == Some(FIND_BAR) {
            return;
        }
        if let Some(find) = self.find.take()
            && let Some((pane, _)) = self.panes.get(find.model.pane())
        {
            pane.update(cx, |pane, cx| pane.show_found(Vec::new(), cx));
        }
    }

    /// A key the bar's own focus or its field gets: typed text, Backspace, Enter and Shift+Enter.
    fn find_bar_typed(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(find) = self.find.as_mut() else {
            return;
        };
        if !(self.popover.holds_keyboard(window) || find.field.is_focused(window)) {
            return;
        }
        let Some(chord) = chord_of(&event.keystroke) else {
            return;
        };
        let typed = event
            .keystroke
            .key_char
            .clone()
            .filter(|_| !chord.mods.ctrl && !chord.mods.alt);
        let result = match chord.key {
            Key::Enter if !chord.mods.ctrl && !chord.mods.alt => {
                find.model.step(&self.client, chord.mods.shift)
            }
            Key::Backspace => find.model.backspace(&self.client),
            _ => match typed {
                Some(text) => find.model.type_text(&self.client, &text),
                None => return,
            },
        };
        cx.stop_propagation();
        self.find_bar_answered(result, cx);
    }

    /// The previous or next match, from the bar's buttons.
    fn find_bar_step(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let Some(find) = self.find.as_mut() else {
            return;
        };
        let result = find.model.step(&self.client, backwards);
        self.find_bar_answered(result, cx);
    }

    /// After the server answered: the pane shows the matches, or the window its notice.
    fn find_bar_answered(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        self.notice = result.err();
        if let Some(find) = &self.find
            && let Some((pane, _)) = self.panes.get(find.model.pane())
        {
            let runs = find.model.found().runs.clone();
            pane.update(cx, |pane, cx| pane.show_found(runs, cx));
        }
        cx.notify();
    }

    /// The bar, in its popover.
    pub(super) fn find_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let Some(find) = self.find.as_ref() else {
            self.close_popover(window, cx);
            return None;
        };
        let typing = self.popover.holds_keyboard(window) || find.field.is_focused(window);
        let field = self.find_field(find, typing, window);
        let hair = t::hairline(window.scale_factor());
        let buttons = 3.0;
        let field_width = t::FIND_BAR_WIDTH
            - 2.0 * (hair + t::POPOVER_PADDING)
            - buttons * (t::FIND_BAR_GAP + t::ICON_BUTTON_HEIGHT_BAR);
        let field = Control::new(Kind::Field, FIELD_ID, find.model.query().to_owned())
            .focus(find.field.clone())
            .width(field_width)
            .content(field);
        let mut bar = div()
            .flex()
            .items_center()
            .gap(px(t::FIND_BAR_GAP))
            .p(px(t::POPOVER_PADDING))
            .child(self.kit.build(FIND_BAR, field, window, cx));
        for (id, label, icon) in [
            (PREVIOUS_ID, "Previous", Icon::ChevronUp),
            (NEXT_ID, "Next", Icon::ChevronDown),
            (CLOSE_ID, "Close", Icon::Close),
        ] {
            let this = cx.entity();
            let button = Control::new(Kind::IconButton, id, label)
                .icon(icon)
                .group(BUTTONS)
                .on_click(move |window, cx| {
                    this.update(cx, |this, cx| match id {
                        CLOSE_ID => this.close_popover(window, cx),
                        _ => this.find_bar_step(id == PREVIOUS_ID, cx),
                    });
                });
            bar = bar.child(self.kit.build(FIND_BAR, button, window, cx));
        }
        let keys = cx.entity();
        let closer = cx.entity();
        self.popover.element(
            &self.kit,
            &self.colors,
            bar.into_any_element(),
            move |event, window, cx| {
                keys.update(cx, |this, cx| this.find_bar_typed(event, window, cx));
            },
            move |window, cx| closer.update(cx, |this, cx| this.close_popover(window, cx)),
            window,
        )
    }

    /// The field's inside: the text (or the placeholder), the caret while the bar takes typing, and the count.
    fn find_field(&self, find: &FindBar, typing: bool, window: &Window) -> AnyElement {
        let query = find.model.query();
        let count = find.model.count_label();
        let colors = &self.colors;
        let text = if query.is_empty() {
            div()
                .text_color(paint(colors.muted_foreground, 1.0))
                .child(PLACEHOLDER)
        } else {
            div()
                .text_color(paint(colors.foreground, 1.0))
                .child(query.to_owned())
        }
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis();
        let caret = typing.then(|| {
            div()
                .flex_none()
                .w(px(t::hairline(window.scale_factor())))
                .h(px(t::CARET_HEIGHT))
                .bg(paint(colors.foreground, 1.0))
        });
        let mut line = div()
            .flex()
            .items_center()
            .w_full()
            .text_size(px(t::TEXT_UI));
        line = if query.is_empty() {
            line.children(caret).child(text)
        } else {
            line.child(text).children(caret)
        };
        line.child(div().flex_1())
            .child(
                div()
                    .relative()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_color(paint(colors.muted_foreground, 1.0))
                    .child(count.clone())
                    .child(self.kit.record().landmark_with_text("find-count", 0, count)),
            )
            .into_any_element()
    }
}
