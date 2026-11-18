//! Dragging in the main window: a divider between two panes moves their split, and a tab moves along the title bar.
//!
//! While a drag lasts, one listener over the whole window takes the pointer's moves and the release, so nothing under
//! the pointer (a pane selecting text) sees them. The workspace changes on the screen as the pointer moves and is
//! written to the server once, on the release.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use clanky_core::layout::{Axis, Layout};
use gpui::{
    AnyElement, App, Bounds, Context, CursorStyle, DispatchPhase, Div, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    Point, Styled, Window, canvas, deferred, div, px,
};

use super::MainWindow;
use crate::tokens as t;

/// How far a tab must move with the button held before it is a drag and not a click.
const TAB_DRAG_THRESHOLD: f32 = 4.0;

/// What is being dragged.
pub(super) enum Drag {
    /// The divider of the split at `path` (false for the first child, true for the second, from the root) in the
    /// active tab.
    Divider {
        path: Vec<bool>,
        axis: Axis,
        /// The split's box and the least room each side needs, in logical pixels along the axis.
        area: Bounds<Pixels>,
        least: (f32, f32),
        /// The divider's own width along the axis: one device pixel.
        divider: f32,
    },
    /// The tab first pressed at `index`, now at `at`; `moved` once it has gone past the threshold.
    Tab {
        at: usize,
        start_x: f32,
        moved: bool,
    },
}

/// Where the splits and tabs of the frame were laid out, for the drag that starts on them.
#[derive(Clone, Default)]
pub(super) struct Places {
    splits: Rc<RefCell<HashMap<Vec<bool>, Bounds<Pixels>>>>,
    tabs: Rc<RefCell<Vec<Bounds<Pixels>>>>,
}

impl Places {
    pub(super) fn clear(&self) {
        self.splits.borrow_mut().clear();
        self.tabs.borrow_mut().clear();
    }

    /// A measuring child for the split at `path`: absolute and full size inside the split's box.
    pub(super) fn split(&self, path: Vec<bool>) -> AnyElement {
        let splits = Rc::clone(&self.splits);
        measure(move |bounds| {
            splits.borrow_mut().insert(path.clone(), bounds);
        })
    }

    /// A measuring child for the tab at `index`.
    pub(super) fn tab(&self, index: usize) -> AnyElement {
        let tabs = Rc::clone(&self.tabs);
        measure(move |bounds| {
            let mut tabs = tabs.borrow_mut();
            if tabs.len() <= index {
                tabs.resize(index + 1, Bounds::default());
            }
            tabs[index] = bounds;
        })
    }
}

fn measure(store: impl Fn(Bounds<Pixels>) + 'static) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .child(canvas(move |bounds, _, _| store(bounds), |_, (), _, _| ()).size_full())
        .into_any_element()
}

/// The ratio that puts a divider under the pointer: the first child's share of the split's room once the divider's own
/// width is taken out, kept so each side has at least its least room.
pub(super) fn ratio_at(
    pointer: f32,
    start: f32,
    length: f32,
    least: (f32, f32),
    divider: f32,
) -> f64 {
    let room = (length - divider).max(1.0);
    let first = (pointer - start - divider / 2.0)
        .min(room - least.1)
        .max(least.0)
        .clamp(0.0, room);
    f64::from(first / room)
}

/// The tab slot under the pointer: the last tab whose middle the pointer has passed, else the first.
pub(super) fn slot_at(pointer: f32, tabs: &[Bounds<Pixels>]) -> usize {
    tabs.iter()
        .rposition(|tab| pointer >= f32::from(tab.origin.x + tab.size.width / 2.0))
        .unwrap_or(0)
}

/// The least room a subtree needs along an axis: its minimum cells, each pane's padding, and the dividers between panes.
fn least_room(layout: &Layout, axis: Axis, cell: (f32, f32), divider: f32) -> f32 {
    let (columns, rows) = layout.minimum_size();
    let panes_across = panes_along(layout, axis) as f32;
    let (cells, size) = match axis {
        Axis::Columns => (columns, cell.0),
        Axis::Rows => (rows, cell.1),
    };
    cells as f32 * size
        + panes_across * 2.0 * crate::grid_element::PANE_PADDING
        + (panes_across - 1.0) * divider
}

/// How many panes a subtree puts side by side along an axis.
fn panes_along(layout: &Layout, axis: Axis) -> usize {
    match layout {
        Layout::Leaf(_) => 1,
        Layout::Split {
            axis: own,
            first,
            second,
            ..
        } => {
            let (a, b) = (panes_along(first, axis), panes_along(second, axis));
            if *own == axis { a + b } else { a.max(b) }
        }
    }
}

fn split_at<'a>(layout: &'a Layout, path: &[bool]) -> Option<&'a Layout> {
    path.iter().try_fold(layout, |node, second| match node {
        Layout::Split {
            first,
            second: other,
            ..
        } => Some(if *second {
            other.as_ref()
        } else {
            first.as_ref()
        }),
        Layout::Leaf(_) => None,
    })
}

impl MainWindow {
    /// A hairline in the border colour across the window, or down it: the edge between two flush neighbours.
    pub(super) fn hairline(&self, axis: Axis, scale: f32) -> Div {
        let line = div().flex_none().bg(super::paint(self.colors.border, 1.0));
        match axis {
            Axis::Columns => line.w(px(t::hairline(scale))).h_full(),
            Axis::Rows => line.h(px(t::hairline(scale))).w_full(),
        }
    }

    /// The divider between a split's two children: a hairline, which the pointer drags anywhere within the hit width
    /// around it. The hit strip is drawn last, over the panes on either side, so it takes the press before they do.
    pub(super) fn divider(
        &self,
        path: Vec<bool>,
        axis: Axis,
        scale: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let line = t::hairline(scale);
        let overhang = px(-(t::DIVIDER_HIT_WIDTH - line) / 2.0);
        let hit = match axis {
            Axis::Columns => div()
                .top_0()
                .bottom_0()
                .left(overhang)
                .w(px(t::DIVIDER_HIT_WIDTH))
                .cursor(CursorStyle::ResizeLeftRight),
            Axis::Rows => div()
                .left_0()
                .right_0()
                .top(overhang)
                .h(px(t::DIVIDER_HIT_WIDTH))
                .cursor(CursorStyle::ResizeUpDown),
        };
        let hit = hit
            .absolute()
            .id((
                "divider",
                path.iter()
                    .fold(1_u64, |key, &second| key * 2 + u64::from(second)),
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                    this.start_divider(path.clone(), axis, t::hairline(window.scale_factor()), cx);
                    cx.stop_propagation();
                }),
            );
        self.hairline(axis, scale)
            .relative()
            .child(deferred(hit))
            .into_any_element()
    }

    fn start_divider(&mut self, path: Vec<bool>, axis: Axis, divider: f32, cx: &mut Context<Self>) {
        let Some(area) = self.places.splits.borrow().get(&path).copied() else {
            return;
        };
        let Some(Layout::Split { first, second, .. }) = self
            .workspace
            .groups
            .get(self.workspace.active)
            .and_then(|group| split_at(&group.layout, &path))
        else {
            return;
        };
        let cell = self
            .panes
            .values()
            .find_map(|pane| pane.0.read(cx).cell_size())
            .unwrap_or(super::FALLBACK_CELL);
        let least = (
            least_room(first, axis, cell, divider),
            least_room(second, axis, cell, divider),
        );
        self.drag = Some(Drag::Divider {
            path,
            axis,
            area,
            least,
            divider,
        });
        cx.notify();
    }

    /// Called from a tab's own mouse-down: the tab may be dragged from here.
    pub(super) fn press_tab(
        &mut self,
        index: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.drag = Some(Drag::Tab {
            at: index,
            start_x: f32::from(position.x),
            moved: false,
        });
        cx.notify();
    }

    fn drag_to(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let active = self.workspace.active;
        match &mut self.drag {
            Some(Drag::Divider {
                path,
                axis,
                area,
                least,
                divider,
            }) => {
                let (pointer, start, length) = match axis {
                    Axis::Columns => (position.x, area.origin.x, area.size.width),
                    Axis::Rows => (position.y, area.origin.y, area.size.height),
                };
                let ratio = ratio_at(
                    f32::from(pointer),
                    f32::from(start),
                    f32::from(length),
                    *least,
                    *divider,
                );
                if let Some(group) = self.workspace.groups.get_mut(active)
                    && group.layout.set_ratio(path, ratio).is_ok()
                {
                    cx.notify();
                }
            }
            Some(Drag::Tab { at, start_x, moved }) => {
                let pointer = f32::from(position.x);
                *moved |= (pointer - *start_x).abs() > TAB_DRAG_THRESHOLD;
                if !*moved {
                    return;
                }
                let slot = slot_at(pointer, &self.places.tabs.borrow());
                if slot != *at && slot < self.workspace.groups.len() {
                    let focus = self.workspace.groups[active].focus.clone();
                    let group = self.workspace.groups.remove(*at);
                    self.workspace.groups.insert(slot, group);
                    self.workspace.active = self
                        .workspace
                        .groups
                        .iter()
                        .position(|group| group.focus == focus)
                        .unwrap_or(active);
                    *at = slot;
                    cx.notify();
                }
            }
            None => {}
        }
    }

    /// The release: a divider's or a moved tab's new workspace goes to the server.
    fn end_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let changed = match self.drag.take() {
            Some(Drag::Divider { .. } | Drag::Tab { moved: true, .. }) => true,
            Some(Drag::Tab { moved: false, .. }) | None => false,
        };
        if changed {
            self.act(window, cx, |this, _, _| this.save());
        } else {
            cx.notify();
        }
    }

    /// The window-wide pointer listener for drags. It is in every frame, not only once a drag has begun: a release
    /// that comes before the next frame (a quick click on a tab) must still end the drag.
    pub(super) fn drag_listener(cx: &mut Context<Self>) -> AnyElement {
        let (moves, release) = (cx.entity(), cx.entity());
        canvas(
            |_, _, _| (),
            move |_, (), window, _| {
                let moves = moves.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx: &mut App| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let dragging = moves.update(cx, |this, cx| {
                        let dragging = this.drag.is_some();
                        if dragging {
                            this.drag_to(event.position, cx);
                        }
                        dragging
                    });
                    if dragging {
                        cx.stop_propagation();
                    }
                });
                let release = release.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx: &mut App| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        release.update(cx, |this, cx| {
                            if this.drag.is_some() {
                                this.end_drag(window, cx);
                            }
                        });
                    }
                });
            },
        )
        .into_any_element()
    }
}

#[cfg(test)]
// The values compared are whole numbers, exact in binary.
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use gpui::{point, size};

    #[test]
    fn a_divider_follows_the_pointer_within_each_sides_least_room() {
        // A split 1002 px wide holds 1000 px of panes and the 2 px divider (one device pixel at scale 0.5 would be
        // smaller than any real one, so 2 keeps the half exact); the first side's width in whole pixels.
        let first = |pointer: f32, start: f32, least: (f32, f32)| {
            (ratio_at(pointer, start, 1002.0, least, 2.0) * 1000.0).round()
        };
        // The pointer at 501 puts the divider's middle there: 500 px to the first side.
        assert_eq!(first(501.0, 0.0, (0.0, 0.0)), 500.0);
        assert_eq!(first(251.0, 0.0, (0.0, 0.0)), 250.0);
        // Each side keeps its least room: 200 px first, 300 px second.
        assert_eq!(first(10.0, 0.0, (200.0, 300.0)), 200.0);
        assert_eq!(first(990.0, 0.0, (200.0, 300.0)), 700.0);
        // The split's own start counts.
        assert_eq!(first(601.0, 100.0, (0.0, 0.0)), 500.0);
    }

    #[test]
    fn a_dragged_tab_takes_the_slot_whose_middle_it_passed() {
        let tab = |x: f32| Bounds::new(point(px(x), px(6.0)), size(px(80.0), px(28.0)));
        let tabs = [tab(8.0), tab(94.0), tab(180.0)];
        // Middles at 48, 134, 220.
        assert_eq!(slot_at(20.0, &tabs), 0);
        assert_eq!(slot_at(133.0, &tabs), 0);
        assert_eq!(slot_at(134.0, &tabs), 1);
        assert_eq!(slot_at(500.0, &tabs), 2);
    }

    #[test]
    fn the_least_room_counts_cells_padding_and_gaps() {
        let leaf = |id: &str| Layout::leaf(id);
        // Two panes side by side: 17 columns (the layout counts one cell for its divider) of 10 px, two panes' 2 x 12 px
        // of padding, one 1 px divider: 170 + 48 + 1.
        let pair = Layout::branch(Axis::Columns, leaf("a"), leaf("b"));
        assert_eq!(least_room(&pair, Axis::Columns, (10.0, 20.0), 1.0), 219.0);
        // Stacked, they need one pane's columns across: 8 x 10 + 24.
        let stack = Layout::branch(Axis::Rows, leaf("a"), leaf("b"));
        assert_eq!(least_room(&stack, Axis::Columns, (10.0, 20.0), 1.0), 104.0);
    }
}
