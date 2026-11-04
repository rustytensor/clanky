//! Studio pixel geometry and deterministic motion, independent of a renderer.
use clanky_core::layout::{Axis, Layout as WorkspaceLayout, MIN_COLUMNS, MIN_ROWS, stable_floor};

mod theme;
#[cfg(test)]
mod theme_tests;
pub use theme::{Color, Metrics, Theme, COLOR_TOKENS};
mod scene_builder;
pub use scene_builder::*;

// Compatibility names refer to the single metrics definition, never repeat values.
pub const TOP_BAR: f64 = Metrics::DEFAULT.top_bar;
pub const LEFT_RAIL: f64 = Metrics::DEFAULT.left_rail;
pub const RIGHT_RAIL: f64 = Metrics::DEFAULT.right_rail;
pub const GAP: f64 = Metrics::DEFAULT.pane_gap;
pub const PADDING: f64 = Metrics::DEFAULT.pane_padding;
pub const HEADER: f64 = Metrics::DEFAULT.pane_header;
pub const PANE_RADIUS: f64 = Metrics::DEFAULT.pane_radius;
pub const BLOCK_RADIUS: f64 = Metrics::DEFAULT.block_radius;
pub const CARD_INSET: f64 = Metrics::DEFAULT.card_inset;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PixelRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl PixelRect {
    pub fn array(self) -> [f32; 4] {
        [self.x, self.y, self.w, self.h]
    }
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x as f64
            && y >= self.y as f64
            && x < self.x as f64 + self.w as f64
            && y < self.y as f64 + self.h as f64
    }
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> PixelRect {
    PixelRect {
        x: x as f32,
        y: y as f32,
        w: w.max(0.0) as f32,
        h: h.max(0.0) as f32,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaneLayout {
    pub id: String,
    pub rect: PixelRect,
    pub grid_origin: [f32; 2],
    pub grid_clip: PixelRect,
    pub rows: usize,
    pub cols: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Divider {
    /// Stable tree address: false = first child, true = second child.
    pub path: Vec<bool>,
    pub axis: Axis,
    pub rect: PixelRect,
    pub container: PixelRect,
    pub minimum_first: f64,
    pub minimum_second: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub window_px: [u32; 2],
    pub scale: f64,
    pub rails_shown: bool,
    pub cell_px: [u32; 2],
    /// Physical metrics used by both layout and scene generation.
    pub metrics: Metrics,
    pub logical_metrics: Metrics,
    pub top_bar: PixelRect,
    pub left_rail: Option<PixelRect>,
    pub right_rail: Option<PixelRect>,
    pub content: PixelRect,
    pub panes: Vec<PaneLayout>,
    pub dividers: Vec<Divider>,
    pub workspace_layout: WorkspaceLayout,
    /// The window should enforce this minimum before accepting divider drags.
    pub minimum_window_px: [u32; 2],
    pub fits_minimum: bool,
    pub valid: bool,
}
impl Layout {
    pub fn pane(&self, id: &str) -> Option<&PaneLayout> {
        self.panes.iter().find(|pane| pane.id == id)
    }
    pub fn pane_at(&self, x: f64, y: f64) -> Option<&PaneLayout> {
        self.panes.iter().find(|pane| pane.rect.contains(x, y))
    }
    pub fn divider_at(&self, x: f64, y: f64) -> Option<usize> {
        self.dividers
            .iter()
            .position(|divider| divider.rect.contains(x, y))
    }
}

fn minimum(tree: &WorkspaceLayout, metrics: &Metrics, cell: [u32; 2]) -> [f64; 2] {
    match tree {
        WorkspaceLayout::Leaf(id) if id.is_empty() => [0.0, 0.0],
        WorkspaceLayout::Leaf(_) => [
            2.0 * metrics.pane_padding + MIN_COLUMNS as f64 * cell[0] as f64,
            (2.0 * metrics.pane_padding + metrics.pane_header) + MIN_ROWS as f64 * cell[1] as f64,
        ],
        WorkspaceLayout::Split {
            axis,
            first,
            second,
            ..
        } => {
            let a = minimum(first, metrics, cell);
            let b = minimum(second, metrics, cell);
            match axis {
                Axis::Columns => [a[0] + b[0] + metrics.pane_gap, a[1].max(b[1])],
                Axis::Rows => [a[0].max(b[0]), a[1] + b[1] + metrics.pane_gap],
            }
        }
    }
}
fn valid_tree(tree: &WorkspaceLayout) -> bool {
    match tree {
        WorkspaceLayout::Leaf(_) => true,
        WorkspaceLayout::Split {
            ratio,
            first,
            second,
            ..
        } => {
            ratio.is_finite()
                && (0.0..=1.0).contains(ratio)
                && valid_tree(first)
                && valid_tree(second)
        }
    }
}
fn allocation(available: f64, ratio: f64, min_a: f64, min_b: f64) -> f64 {
    let ideal = stable_floor(available * ratio);
    if available >= min_a.ceil() + min_b.ceil() {
        ideal.clamp(min_a.ceil(), (available - min_b).floor())
    } else {
        ideal.clamp(0.0, available)
    }
}

/// Physical window/cell sizes; only chrome constants are multiplied by scale.
/// Tiny windows report fits_minimum=false rather than inventing off-window space.
pub fn layout(
    window_px: [u32; 2],
    scale: f64,
    rails_shown: bool,
    workspace_layout: &WorkspaceLayout,
    cell_px: [u32; 2],
) -> Layout {
    layout_with_metrics(window_px, scale, rails_shown, workspace_layout, cell_px, &Metrics::default())
}

/// Configurable logical metrics enter here; the compatibility layout uses defaults.
pub fn layout_with_metrics(
    window_px: [u32; 2], scale: f64, rails_shown: bool,
    workspace_layout: &WorkspaceLayout, cell_px: [u32; 2], logical_metrics: &Metrics,
) -> Layout {
    let valid = scale.is_finite()
        && scale > 0.0
        && logical_metrics.scaled(scale).is_valid()
        && cell_px[0] > 0
        && cell_px[1] > 0
        && valid_tree(workspace_layout);
    let scale = if valid { scale } else { 1.0 };
    let [w, h] = window_px.map(f64::from);
    let metrics = if valid { logical_metrics.scaled(scale) } else { Metrics::default() };
    let gap = metrics.outer_gap;
    let top = metrics.top_bar.min(h);
    let left = if rails_shown {
        metrics.left_rail.min(w)
    } else {
        0.0
    };
    let right = if rails_shown {
        metrics.right_rail.min(w)
    } else {
        0.0
    };
    let x = (left + gap).min(w);
    let y = (top + gap).min(h);
    let content = rect(x, y, (w - right - gap - x).max(0.0), (h - gap - y).max(0.0));
    let minimum = minimum(workspace_layout, &metrics, cell_px);
    let min_window = [
        minimum[0]
            + 2.0 * gap
            + if rails_shown {
                metrics.left_rail + metrics.right_rail
            } else {
                0.0
            },
        minimum[1] + 2.0 * gap + metrics.top_bar,
    ];
    let mut out = Layout {
        window_px,
        scale,
        rails_shown,
        cell_px,
        metrics,
        logical_metrics: *logical_metrics,
        top_bar: rect(0.0, 0.0, w, top),
        left_rail: rails_shown.then(|| rect(0.0, top, left, h - top)),
        right_rail: rails_shown.then(|| rect(w - right, top, right, h - top)),
        content,
        panes: Vec::new(),
        dividers: Vec::new(),
        workspace_layout: workspace_layout.clone(),
        minimum_window_px: min_window.map(|v| v.ceil() as u32),
        fits_minimum: valid && content.w as f64 >= minimum[0] && content.h as f64 >= minimum[1],
        valid,
    };
    if valid {
        collect(workspace_layout, content, &mut Vec::new(), &mut out);
    }
    out
}
fn collect(tree: &WorkspaceLayout, area: PixelRect, path: &mut Vec<bool>, out: &mut Layout) {
    if area.w <= 0.0 || area.h <= 0.0 {
        return;
    }
    let metrics = out.metrics;
    match tree {
        WorkspaceLayout::Leaf(id) => {
            if id.is_empty() {
                return;
            }
            let cols =
                stable_floor((area.w as f64 - 2.0 * metrics.pane_padding).max(0.0) / out.cell_px[0] as f64)
                    as usize;
            let rows = stable_floor(
                (area.h as f64 - (2.0 * metrics.pane_padding + metrics.pane_header)).max(0.0) / out.cell_px[1] as f64,
            ) as usize;
            let origin = [
                area.x as f64 + metrics.pane_padding,
                area.y as f64 + metrics.pane_padding + metrics.pane_header,
            ];
            out.panes.push(PaneLayout {
                id: id.clone(),
                rect: area,
                grid_origin: origin.map(|v| v as f32),
                grid_clip: rect(
                    origin[0],
                    origin[1],
                    cols as f64 * out.cell_px[0] as f64,
                    rows as f64 * out.cell_px[1] as f64,
                ),
                rows,
                cols,
            });
        }
        WorkspaceLayout::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            let a_min = minimum(first, &metrics, out.cell_px);
            let b_min = minimum(second, &metrics, out.cell_px);
            let dimension = if *axis == Axis::Columns { 0 } else { 1 };
            let extent = if *axis == Axis::Columns {
                area.w
            } else {
                area.h
            } as f64;
            let gap = metrics.pane_gap.min(extent);
            let available = (extent - gap).max(0.0);
            let first_extent = allocation(available, *ratio, a_min[dimension], b_min[dimension]);
            let second_extent = available - first_extent;
            let [x, y, _, _] = area.array().map(f64::from);
            let (a, b, divider) = match axis {
                Axis::Columns => (
                    rect(x, y, first_extent, area.h as f64),
                    rect(x + first_extent + gap, y, second_extent, area.h as f64),
                    rect(x + first_extent, y, gap, area.h as f64),
                ),
                Axis::Rows => (
                    rect(x, y, area.w as f64, first_extent),
                    rect(x, y + first_extent + gap, area.w as f64, second_extent),
                    rect(x, y + first_extent, area.w as f64, gap),
                ),
            };
            out.dividers.push(Divider {
                path: path.clone(),
                axis: *axis,
                rect: divider,
                container: area,
                minimum_first: a_min[dimension],
                minimum_second: b_min[dimension],
            });
            path.push(false);
            collect(first, a, path, out);
            path.pop();
            path.push(true);
            collect(second, b, path, out);
            path.pop();
        }
    }
}

#[derive(Clone, Debug)]
pub struct DragResult {
    pub ratio: f64,
    pub workspace_layout: WorkspaceLayout,
    pub layout: Layout,
}
/// Move the centre of a gap to a physical x (columns) or y (rows), clamped to
/// both subtrees' minimum grids. The input layout/tree are never mutated.
pub fn drag_divider(
    before: &Layout,
    divider_index: usize,
    pointer: f64,
) -> Result<DragResult, String> {
    if !before.valid || !before.fits_minimum || !pointer.is_finite() {
        return Err("divider drag requires a valid window with room for minimum grids".into());
    }
    let divider = before
        .dividers
        .get(divider_index)
        .ok_or("unknown divider")?;
    let (origin, extent, gap) = match divider.axis {
        Axis::Columns => (divider.container.x, divider.container.w, divider.rect.w),
        Axis::Rows => (divider.container.y, divider.container.h, divider.rect.h),
    };
    let available = (extent - gap) as f64;
    if available <= 0.0 {
        return Err("divider has no available space".into());
    }
    let first = allocation(
        available,
        ((pointer - origin as f64 - gap as f64 / 2.0) / available).clamp(0.0, 1.0),
        divider.minimum_first,
        divider.minimum_second,
    );
    let ratio = first / available;
    let mut tree = before.workspace_layout.clone();
    tree.set_ratio(&divider.path, ratio)?;
    let next = layout_with_metrics(
        before.window_px,
        before.scale,
        before.rails_shown,
        &tree,
        before.cell_px,
        &before.logical_metrics,
    );
    Ok(DragResult {
        ratio,
        workspace_layout: tree,
        layout: next,
    })
}

/// Clock-free easing shared by cursor, panel and scroll animations.
pub fn ease_out_cubic(
    from: f32,
    to: f32,
    elapsed_ms: f32,
    duration_ms: f32,
    animations: bool,
) -> f32 {
    if !animations || duration_ms <= 0.0 || !duration_ms.is_finite() {
        return to;
    }
    let t = if elapsed_ms.is_nan() {
        0.0
    } else {
        (elapsed_ms / duration_ms).clamp(0.0, 1.0)
    };
    from + (to - from) * (1.0 - (1.0 - t).powi(3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, layout_store};
    use std::collections::BTreeMap;

    #[test]
    fn studio_geometry_drag_motion_config() {
        // Fixed design-m11-look.md cases 1–3, 6–7, calculated before implementation.
        let tree = WorkspaceLayout::branch(
            Axis::Rows,
            WorkspaceLayout::leaf("A"),
            WorkspaceLayout::leaf("B"),
        );
        let shown = layout([1240, 760], 1.0, true, &tree, [10, 20]);
        assert_eq!(shown.content.array(), [220.0, 50.0, 740.0, 700.0]);
        assert_eq!(shown.panes[0].rect.array(), [220.0, 50.0, 740.0, 345.0]);
        assert_eq!(shown.panes[1].rect.array(), [220.0, 405.0, 740.0, 345.0]);
        assert_eq!(shown.panes[0].grid_origin, [232.0, 86.0]);
        assert_eq!(shown.panes[1].grid_origin, [232.0, 441.0]);
        assert!(
            shown
                .panes
                .iter()
                .all(|pane| (pane.cols, pane.rows) == (71, 14))
        );
        let hidden = layout([1240, 760], 1.0, false, &tree, [10, 20]);
        assert_eq!(hidden.panes[0].rect.array(), [10.0, 50.0, 1220.0, 345.0]);
        assert_eq!(hidden.panes[1].rect.array(), [10.0, 405.0, 1220.0, 345.0]);
        assert!(
            hidden
                .panes
                .iter()
                .all(|pane| (pane.cols, pane.rows) == (119, 14))
        );
        let moved = drag_divider(&shown, 0, 300.0).unwrap();
        assert_eq!(
            moved.layout.panes[0].rect.array(),
            [220.0, 50.0, 740.0, 245.0]
        );
        assert_eq!(
            moved.layout.panes[1].rect.array(),
            [220.0, 305.0, 740.0, 445.0]
        );
        assert_eq!(
            (moved.layout.panes[0].rows, moved.layout.panes[1].rows),
            (9, 19)
        );
        // Minimum height is 48 chrome + 3 * 20 cell pixels = 108.
        assert_eq!(
            drag_divider(&shown, 0, 0.0).unwrap().layout.panes[0].rect.h,
            108.0
        );
        assert_eq!(
            drag_divider(&shown, 0, 1000.0).unwrap().layout.panes[1]
                .rect
                .h,
            108.0
        );
        let stored = serde_json::to_string(&moved.workspace_layout).unwrap();
        let restored: WorkspaceLayout = serde_json::from_str(&stored).unwrap();
        let remapped = layout_store::remap(
            &restored,
            &BTreeMap::from([("A".into(), "t1".into()), ("B".into(), "t2".into())]),
        )
        .unwrap();
        let restored = layout([1240, 760], 1.0, true, &remapped, [10, 20]);
        assert_eq!(restored.panes[0].rect.array(), [220.0, 50.0, 740.0, 245.0]);
        assert_eq!(restored.panes[1].rect.array(), [220.0, 305.0, 740.0, 445.0]);
        let legacy =
            serde_json::json!({"Split":{"axis":"Rows","first":{"Leaf":"A"},"second":{"Leaf":"B"}}});
        assert_eq!(serde_json::to_value(&tree).unwrap(), legacy);
        assert_eq!(
            serde_json::from_value::<WorkspaceLayout>(legacy).unwrap(),
            tree
        );
        for (elapsed, expected) in [(0.0, 100.0), (40.0, 187.5), (80.0, 200.0), (120.0, 200.0)] {
            assert_eq!(ease_out_cubic(100.0, 200.0, elapsed, 80.0, true), expected);
        }
        assert_eq!(ease_out_cubic(0.0, 1.0, 60.0, 120.0, true), 0.875);
        assert_eq!(ease_out_cubic(100.0, 200.0, 0.0, 80.0, false), 200.0);
        assert_eq!(ease_out_cubic(0.0, 1.0, 0.0, 120.0, false), 1.0);
        assert!(Config::from_json(r#"{"opacity":0.3}"#).is_err());
        assert!(Config::from_json(r#"{"rails":"yes"}"#).is_err());
        assert_eq!(
            Config::from_json(r#"{"opacity":1}"#)
                .unwrap()
                .opacity_alpha(),
            255
        );
        assert_eq!(Config::default().opacity_alpha(), 240);
        let defaults = Config::from_json("{}").unwrap();
        assert_eq!(defaults.font_family, "JetBrains Mono");
        assert_eq!(defaults.ui_font_family, "Inter");
    }
}
