//! The command input's completion list (mockup `.pop`), on the window's one popover: Tab with more than one candidate
//! opens it above the input, one menu item per candidate with its kind at the right end, and a footer naming the keys.
//!
//! The list belongs to the pane's input model, which keeps the keyboard while it is open, as today: Tab puts the
//! highlighted candidate in, Enter runs it, Up and Down move the highlight, any other key closes the list and goes on
//! to the text. So the popover follows the model rather than owning the keys: it opens when the focused pane's input
//! shows a list and closes when the list goes, and the pane gets the keyboard straight back after the popover takes
//! it at open. Ctrl+F6 still walks into the rows like any popover's controls; Esc closes the list from anywhere.

use std::{cell::Cell, rc::Rc};

use clanky_state::keys::{Chord, Key};
use gpui::{
    AnyElement, Bounds, Context, IntoElement, ParentElement, Pixels, Styled, Window, canvas, div,
    px,
};

use super::MainWindow;
use crate::{
    control_look::paint,
    controls::{Control, Group, INTERFACE_FONT_FAMILY, Kind},
    popover::{Area, Opener, Placement},
    tokens as t,
};

/// The list's name in the control record.
pub(super) const LIST_SCREEN: &str = "completion-list";
/// Control ids of the rows: the candidate's place in the list, from here.
const ROW_IDS: u32 = 7000;
/// Rows shown at once; the highlight scrolls the rest into view.
const MAX_ROWS: usize = 8;
const ROWS: Group = Group::vertical("completion-rows");
const FOOTER: &str = "Tab completes · Enter runs · Esc closes";

/// The command bar's box as last drawn, for the list to open next to.
pub(super) type Measured = Rc<Cell<Option<Bounds<Pixels>>>>;

impl MainWindow {
    /// A key while the completion list is open, before the popover's own keys: Esc closes the list; Tab, while the
    /// pane has the keyboard, goes on to the pane, which completes. `Some(taken)` decides the key; None leaves it to
    /// the popover and the window.
    pub(super) fn completion_list_key(
        &mut self,
        chord: Chord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<bool> {
        if self.popover.screen() != Some(LIST_SCREEN) || chord.mods.ctrl || chord.mods.alt {
            return None;
        }
        match chord.key {
            Key::Escape => {
                self.close_completion_list(window, cx);
                Some(true)
            }
            Key::Tab if !self.kit.has_focus(window) => Some(false),
            _ => None,
        }
    }

    /// The command bar of pane `id` was drawn with `listed` (its input shows a completion list) while the pane had
    /// the keyboard or not (`focused`): opens or closes the popover to match, after this frame, once its box is
    /// measured.
    pub(super) fn follow_completion_list(
        &self,
        id: &str,
        listed: bool,
        focused: bool,
        bar: &Measured,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let open_here = self.completion_pane().as_deref() == Some(id);
        if listed == open_here || (listed && !focused) {
            return;
        }
        let (id, bar) = (id.to_owned(), Rc::clone(bar));
        cx.defer_in(window, move |this, window, cx| {
            this.apply_completion_list(&id, &bar, window, cx);
        });
    }

    /// The pane the open completion list belongs to.
    fn completion_pane(&self) -> Option<String> {
        if self.popover.screen() != Some(LIST_SCREEN) {
            return None;
        }
        self.popover.opener()?.pane.clone()
    }

    fn apply_completion_list(
        &mut self,
        id: &str,
        bar: &Measured,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.panes.get(id).map(|pane| pane.0.clone()) else {
            return;
        };
        let (listed, handle) = {
            let read = pane.read(cx);
            (read.completion_listed(), read.focus_handle().clone())
        };
        let open_here = self.completion_pane().as_deref() == Some(id);
        if !listed && open_here {
            self.close_popover(window, cx);
            return;
        }
        let Some(bounds) = bar.get() else {
            return;
        };
        if listed && !open_here && handle.is_focused(window) {
            let opener = Opener {
                area: Area::of(bounds),
                control: None,
                pane: Some(id.to_owned()),
            };
            self.popover
                .open(LIST_SCREEN, opener, Placement::Beside, window, cx);
            window.focus(&handle, cx);
            cx.notify();
        }
    }

    /// Closes the list in its pane's input and the popover with it.
    fn close_completion_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pane) = self
            .completion_pane()
            .and_then(|id| self.panes.get(&id))
            .map(|pane| pane.0.clone())
        {
            pane.update(cx, |pane, cx| pane.close_completion(cx));
        }
        self.close_popover(window, cx);
    }

    /// A measuring element for a command bar, filling it, that keeps its box in `bar`.
    pub(super) fn measure_bar(bar: &Measured) -> AnyElement {
        let keep = Rc::clone(bar);
        canvas(move |bounds, _, _| keep.set(Some(bounds)), |_, (), _, _| {})
            .absolute()
            .size_full()
            .into_any_element()
    }

    /// The list's rows and footer in the popover; None when its pane no longer shows a list.
    pub(super) fn completion_list(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let pane = self.panes.get(&self.completion_pane()?)?.0.clone();
        let (items, selected, _) = pane.read(cx).input_shown()?.view.popup?;
        // The highlight is the pane's: shown while the pane has the keyboard, not while a row has it.
        let highlight = pane
            .read(cx)
            .focus_handle()
            .is_focused(window)
            .then_some(selected);
        let shown = items.len().min(MAX_ROWS);
        let first = selected.saturating_sub(shown.saturating_sub(1));
        let mut rows = div()
            .flex()
            .flex_col()
            .w(px(t::PALETTE_WIDTH / 2.0))
            .p(px(t::POPOVER_PADDING));
        for (index, (name, kind)) in items.into_iter().enumerate().skip(first).take(shown) {
            let (picker, this) = (pane.clone(), cx.entity());
            let row = Control::new(Kind::MenuItem, ROW_IDS + index as u32, name)
                .group(ROWS)
                .keys(kind)
                .checked(highlight == Some(index))
                .on_click(move |window, cx| {
                    picker.update(cx, |pane, cx| pane.pick_completion(index, cx));
                    this.update(cx, |this, cx| this.close_popover(window, cx));
                });
            rows = rows.child(self.kit.build(LIST_SCREEN, row, window, cx));
        }
        let footer = div()
            .px(px(t::SPACE_12))
            .py(px(t::SPACE_6))
            .border_t(px(t::hairline(window.scale_factor())))
            .border_color(paint(self.colors.border, 1.0))
            .font_family(INTERFACE_FONT_FAMILY)
            .text_size(px(t::TEXT_STATUS))
            .text_color(paint(self.colors.muted_foreground, 1.0))
            .child(FOOTER);
        let closer = cx.entity();
        self.popover.element(
            &self.kit,
            &self.colors,
            div()
                .flex()
                .flex_col()
                .child(rows)
                .child(footer)
                .into_any_element(),
            |_, _, _| {},
            move |window, cx| closer.update(cx, |this, cx| this.close_completion_list(window, cx)),
            window,
        )
    }
}
