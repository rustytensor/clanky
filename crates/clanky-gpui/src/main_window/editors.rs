//! The window's editor panes. A file clicked in the Files tree opens in an editor pane to the right of the focused
//! terminal pane, as in today's window. The session server's workspace holds terminals only, so editor panes are the
//! window's own (today's window kept them in a layout of its own around the terminals too): each sits beside one
//! terminal pane and shares that pane's room in the split tree equally with it.

use std::{path::Path, rc::Rc};

use clanky::editor::{EditorPane, SyntaxColors};
use gpui::{
    AnyElement, AppContext, Context, Entity, IntoElement, ParentElement, Styled, Subscription,
    Window, div, px,
};

use super::MainWindow;
use crate::{
    editor_pane::{EditorEvent, EditorSetup, EditorView},
    grid_element::colour,
};
use clanky_core::layout::Axis;

struct Attached {
    /// The terminal pane the editor sits beside.
    beside: String,
    /// The pane's slot, the index of its landmarks.
    slot: u32,
    view: Entity<EditorView>,
    _closed: Subscription,
}

pub(super) struct Editors {
    attached: Vec<Attached>,
    /// The next pane's slot: its landmarks' index and its controls' ids, never reused while the window lives.
    next_slot: u32,
    syntax: SyntaxColors,
}

impl Editors {
    pub(super) fn new(syntax: SyntaxColors) -> Self {
        Editors {
            attached: Vec::new(),
            next_slot: 0,
            syntax,
        }
    }
}

impl MainWindow {
    /// Opens a file in an editor pane beside the focused terminal pane and gives it the keyboard.
    pub(super) fn open_editor(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(beside) = self.focused_pane(window, cx) else {
            self.notice = Some("there is no pane to open the file beside".into());
            cx.notify();
            return;
        };
        let editor = match EditorPane::open(Path::new(path), 1, 1) {
            Ok(editor) => editor,
            Err(error) => {
                self.notice = Some(format!("opening {path}: {error}"));
                cx.notify();
                return;
            }
        };
        let setup = EditorSetup {
            editor,
            palette: self.palette.clone(),
            text: self.text.clone(),
            syntax: self.editors.syntax,
            colors: self.colors.clone(),
            kit: self.kit.clone(),
            trace: Rc::clone(&self.trace),
            slot: self.editors.next_slot,
        };
        let slot = self.editors.next_slot;
        self.editors.next_slot += 1;
        let view = cx.new(|cx| EditorView::new(setup, cx));
        let closed = cx.subscribe_in(&view, window, |this, view, event, window, cx| match event {
            EditorEvent::Closed => this.drop_editor(view, window, cx),
        });
        let focus = view.read(cx).focus_handle().clone();
        window.focus(&focus, cx);
        self.editors.attached.push(Attached {
            beside,
            slot,
            view,
            _closed: closed,
        });
        self.notice = None;
        cx.notify();
    }

    /// Takes a closed editor pane out and gives the keyboard to the terminal pane it sat beside.
    fn drop_editor(
        &mut self,
        view: &Entity<EditorView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(place) = self
            .editors
            .attached
            .iter()
            .position(|attached| &attached.view == view)
        else {
            return;
        };
        let attached = self.editors.attached.remove(place);
        if let Some(pane) = self.panes.get(&attached.beside) {
            window.focus(&pane.0.read(cx).focus_handle().clone(), cx);
        }
        cx.notify();
    }

    /// Ctrl+Shift+W in an editor pane: it closes, or says why it does not. False when no editor pane has the keyboard.
    pub(super) fn close_focused_editor(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        let Some(view) = self
            .editors
            .attached
            .iter()
            .map(|attached| attached.view.clone())
            .find(|view| view.read(cx).focus_handle().is_focused(window))
        else {
            return false;
        };
        view.update(cx, EditorView::request_close);
        true
    }

    /// Editor panes whose terminal pane has gone move beside the active tab's focused pane, so none is lost.
    pub(super) fn rehome_editors(&mut self) {
        let leaves: Vec<String> = self
            .workspace
            .groups
            .iter()
            .flat_map(|group| group.layout.leaves())
            .collect();
        let Some(home) = self
            .workspace
            .groups
            .get(self.workspace.active)
            .map(|group| group.focus.clone())
        else {
            return;
        };
        for attached in &mut self.editors.attached {
            if !leaves.contains(&attached.beside) {
                attached.beside.clone_from(&home);
            }
        }
    }

    /// A terminal pane's room in the split tree, with the editor panes beside it sharing it equally.
    pub(super) fn editors_beside(
        &self,
        id: &str,
        share: f32,
        pane: AnyElement,
        scale: f32,
    ) -> AnyElement {
        let editors: Vec<AnyElement> = self
            .editors
            .attached
            .iter()
            .filter(|attached| attached.beside == id)
            .map(|attached| {
                div()
                    .flex()
                    .flex_basis(px(0.0))
                    .flex_grow(share)
                    .min_w_0()
                    .min_h_0()
                    .relative()
                    .child(
                        div()
                            .flex()
                            .size_full()
                            .overflow_hidden()
                            .bg(colour(self.palette.background))
                            .child(attached.view.clone()),
                    )
                    // The landmark measures the pane's box; flush panes have no border, the divider before it does.
                    .child(self.kit.record().landmark("editor-pane", attached.slot))
                    .into_any_element()
            })
            .collect();
        if editors.is_empty() {
            return pane;
        }
        let mut row = div()
            .flex()
            .flex_row()
            .flex_basis(px(0.0))
            .flex_grow(share)
            .min_w_0()
            .min_h_0()
            .child(pane);
        for editor in editors {
            row = row.child(self.hairline(Axis::Columns, scale)).child(editor);
        }
        row.into_any_element()
    }
}
