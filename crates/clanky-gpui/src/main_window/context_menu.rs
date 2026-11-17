//! The pane's menu: a right-click in a terminal pane (Shift+right-click while the program takes the mouse) opens it at
//! the pointer, in the window's popover. Its rows and their keys are `clanky_state::context_menu`'s; each row runs the
//! code behind its shortcut, or the server request behind the block toolbar's button.
//!
//! A press outside the menu closes it and does nothing else, as in today's window: a backdrop under the popover takes
//! that press, so a left click outside does not also clear the pane's selection.

use clanky_core::{layout::Axis, platform::UrlOpener};
use clanky_state::{
    context_menu::{self, Item, Menu},
    keys::{Action, Chord, chords_for},
    list_selection::ListKey,
};
use gpui::{
    AnyElement, App, ClipboardItem, Context, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Point, Styled, Window, div, px,
};
use serde_json::json;

use super::{MainWindow, chord_of};
use crate::{
    control_look::paint,
    controls::{Control, Group, Kind},
    popover::{Opener, Placement},
    tokens as t,
};

/// The menu's screen name in the control record.
pub(super) const SCREEN: &str = "context-menu";
const ROWS: Group = Group::vertical("context-menu-rows");
/// Row ids in the record: the first row's, counting on down the menu.
const FIRST_ROW_ID: u32 = 1;

/// The open menu's pane and rows, and the helpers that open web addresses from it.
#[derive(Default)]
pub(super) struct PaneMenu {
    open: Option<(String, Menu)>,
    links: UrlOpener,
}

impl MainWindow {
    /// Opens the menu over a pane at a point in the window, for what lies under it.
    pub(super) fn open_pane_menu(
        &mut self,
        pane: &str,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entity) = self.panes.get(pane).map(|pane| pane.0.clone()) else {
            return;
        };
        let mut context = entity.read(cx).menu_context(at);
        context.post_ai = self.post_ai;
        if !self.post_ai {
            context.access = match self.client.agent_permission(pane) {
                Ok(allowed) => allowed,
                Err(error) => {
                    self.notice = Some(format!(
                        "reading the pane's agent access: {}",
                        error.message
                    ));
                    false
                }
            };
        }
        self.popover.open(
            SCREEN,
            Opener::point(at, pane.to_owned()),
            Placement::Beside,
            window,
            cx,
        );
        self.pane_menu.open = Some((pane.to_owned(), Menu::new(context)));
        cx.notify();
    }

    /// The menu's rows inside the popover, over a backdrop that takes a press outside it.
    pub(super) fn context_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let Some((_, menu)) = &self.pane_menu.open else {
            // The popover was opened under this name without a menu: nothing to show, so it closes.
            self.close_popover(window, cx);
            return None;
        };
        let selected = menu
            .selected()
            .filter(|_| self.popover.holds_keyboard(window));
        let scale = window.scale_factor();
        let mut rows = div()
            .flex()
            .flex_col()
            .w(px(t::MENU_WIDTH))
            .p(px(t::POPOVER_PADDING))
            .text_size(px(t::TEXT_UI));
        let mut index = 0;
        for (number, section) in menu.sections().iter().enumerate() {
            if number > 0 {
                rows = rows.child(
                    div()
                        .flex_none()
                        .my(px(t::SPACE_4))
                        .h(px(t::hairline(scale)))
                        .bg(paint(self.colors.border, 1.0)),
                );
            }
            if let Some(heading) = section.heading {
                rows = rows.child(
                    div()
                        .flex_none()
                        .h(px(t::MENU_ITEM_HEIGHT))
                        .line_height(px(t::MENU_ITEM_HEIGHT))
                        .px(px(t::SPACE_8))
                        .text_color(paint(self.colors.muted_foreground, 1.0))
                        .child(heading),
                );
            }
            for row in &section.rows {
                let this = cx.entity();
                let mut control =
                    Control::new(Kind::MenuItem, FIRST_ROW_ID + index as u32, row.label)
                        .group(ROWS)
                        .enabled(row.enabled)
                        .checked(selected == Some(index))
                        .on_click(move |window, cx| {
                            this.update(cx, |this, cx| this.pick_menu_row(index, window, cx));
                        });
                if !row.keys.is_empty() {
                    control = control.keys(row.keys);
                }
                rows = rows.child(self.kit.build(SCREEN, control, window, cx));
                index += 1;
            }
        }
        let keys = cx.entity();
        let closer = cx.entity();
        let card = self.popover.element(
            &self.kit,
            &self.colors,
            rows.into_any_element(),
            move |event, window, cx| {
                let Some(chord) = chord_of(&event.keystroke) else {
                    return;
                };
                keys.update(cx, |this, cx| this.menu_key(chord, window, cx));
            },
            move |window, cx| closer.update(cx, |this, cx| this.close_pane_menu(window, cx)),
            window,
        )?;
        let backdrop = cx.entity();
        let dismiss = move |window: &mut Window, cx: &mut App| {
            backdrop.update(cx, |this, cx| this.close_pane_menu(window, cx));
        };
        let dismiss_left = dismiss.clone();
        let dismiss_middle = dismiss.clone();
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    dismiss_left(window, cx);
                })
                .on_mouse_down(MouseButton::Middle, move |_, window, cx| {
                    dismiss_middle(window, cx);
                })
                .on_mouse_down(MouseButton::Right, move |_, window, cx| dismiss(window, cx))
                .child(card)
                .into_any_element(),
        )
    }

    /// A key the menu's own focus gets: the arrows move its selection, Enter runs the selected row.
    fn menu_key(&mut self, chord: Chord, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, menu)) = self.pane_menu.open.as_mut() else {
            return;
        };
        match menu.key(chord) {
            ListKey::Moved => {
                cx.stop_propagation();
                cx.notify();
            }
            ListKey::Pick(index) => {
                cx.stop_propagation();
                self.pick_menu_row(index, window, cx);
            }
            ListKey::Ignored => {}
        }
    }

    fn close_pane_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pane_menu.open = None;
        if self.popover.screen() == Some(SCREEN) {
            self.close_popover(window, cx);
        }
    }

    /// Closes the menu, which gives the keyboard back to its pane, then runs the row.
    fn pick_menu_row(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((pane, menu)) = self.pane_menu.open.take() else {
            return;
        };
        if self.popover.screen() == Some(SCREEN) {
            self.close_popover(window, cx);
        }
        let Some(row) = menu.row(index).filter(|row| row.enabled) else {
            return;
        };
        let item = row.item;
        match item {
            Item::SplitRight => self.act(window, cx, |this, window, cx| {
                this.split(Axis::Columns, window, cx)
            }),
            Item::SplitBelow => self.act(window, cx, |this, window, cx| {
                this.split(Axis::Rows, window, cx)
            }),
            Item::ClosePane => self.act(window, cx, move |this, _, _| {
                this.close_terminals(std::slice::from_ref(&pane))
            }),
            Item::Find => self.replay(Action::FindInScrollback, "Find", window, cx),
            Item::StartAgent => {
                self.replay(Action::ToggleAgentPicker, "The agent picker", window, cx)
            }
            _ => {
                self.notice = self.run_pane_row(item, &pane, menu.context, cx).err();
                cx.notify();
            }
        }
    }

    /// A row that opens another floating screen runs that screen's own shortcut.
    fn replay(&mut self, action: Action, what: &str, window: &mut Window, cx: &mut Context<Self>) {
        let taken = chords_for(action)
            .next()
            .is_some_and(|chord| self.floating_key(chord, window, cx));
        if !taken {
            self.notice = Some(format!("{what} is not in this window yet"));
            cx.notify();
        }
    }

    /// The rows that act on the pane itself or through the server.
    fn run_pane_row(
        &mut self,
        item: Item,
        pane: &str,
        context: context_menu::Context,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let entity = self
            .panes
            .get(pane)
            .map(|pane| pane.0.clone())
            .ok_or("that pane has closed")?;
        let block = || context.block.clone().ok_or("there is no command here");
        match item {
            Item::OpenLink => self
                .pane_menu
                .links
                .open(context.link.as_deref().unwrap_or_default())
                .map_err(|e| format!("opening the link: {e}")),
            Item::CopyLink => {
                cx.write_to_clipboard(ClipboardItem::new_string(context.link.unwrap_or_default()));
                Ok(())
            }
            Item::Copy => {
                entity.update(cx, |pane, cx| pane.copy_selection(cx));
                Ok(())
            }
            Item::Paste => {
                entity.update(cx, |pane, cx| pane.paste_clipboard(cx));
                Ok(())
            }
            Item::SelectAll => {
                entity.update(cx, |pane, cx| pane.select_all(cx));
                Ok(())
            }
            Item::CopyCommand => {
                cx.write_to_clipboard(ClipboardItem::new_string(block()?.command));
                Ok(())
            }
            Item::CopyOutput => {
                let bytes = context_menu::block_output(&self.client, pane, &block()?.id)?;
                cx.write_to_clipboard(ClipboardItem::new_string(
                    String::from_utf8_lossy(&bytes).into_owned(),
                ));
                Ok(())
            }
            Item::RunAgain => self
                .client
                .input(pane, &context_menu::rerun_bytes(&block()?.command)?)
                .map_err(|e| format!("running the command again: {}", e.message)),
            Item::AgentAccess => self
                .client
                .set_agent_permission(pane, !context.access)
                .map_err(|e| format!("changing the pane's agent access: {}", e.message)),
            Item::Reset | Item::ResetClear => {
                let clear = item == Item::ResetClear;
                self.client
                    .exchange(3, "reset", &json!({"tab_id": pane, "clear": clear}))
                    .map_err(|e| format!("resetting the terminal: {}", e.message))?;
                if clear {
                    entity.update(cx, |pane, cx| pane.clear_selection(cx));
                }
                Ok(())
            }
            Item::SplitRight
            | Item::SplitBelow
            | Item::ClosePane
            | Item::Find
            | Item::StartAgent => Ok(()),
        }
    }
}
