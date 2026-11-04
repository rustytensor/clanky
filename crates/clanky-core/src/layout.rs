//! Session-only terminal layout in cell coordinates, independent of PTYs and rendering.
//!
//! Each branch reserves one divider cell and applies its saved split ratio.
//! Legacy splits default to one half. Resizing never changes leaf IDs. Empty IDs are
//! reserved for an empty layout after its final leaf is removed.

use serde::{Deserialize, Serialize};

/// A pane's content rectangle in zero-based cell coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rect {
    pub col: usize,
    pub row: usize,
    pub cols: usize,
    pub rows: usize,
}

/// The dimension divided by a split (columns for side-by-side panes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    Columns,
    Rows,
}

/// A binary tree whose nonempty leaves are stable terminal IDs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Layout {
    Leaf(String),
    Split {
        axis: Axis,
        #[serde(
            default = "half_ratio",
            deserialize_with = "read_ratio",
            skip_serializing_if = "is_half"
        )]
        ratio: f64,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}

pub const MIN_COLUMNS: usize = 8;
pub const MIN_ROWS: usize = 3;

fn half_ratio() -> f64 {
    0.5
}
fn is_half(ratio: &f64) -> bool {
    *ratio == 0.5
}
fn read_ratio<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    let ratio = f64::deserialize(deserializer)?;
    if ratio.is_finite() && (0.0..=1.0).contains(&ratio) {
        Ok(ratio)
    } else {
        Err(serde::de::Error::custom(
            "split ratio must be finite and between zero and one",
        ))
    }
}

/// Remove only floating-point arithmetic noise at integral pixel/cell boundaries.
pub fn stable_floor(value: f64) -> f64 {
    let nearest = value.round();
    if (value - nearest).abs() <= 4.0 * f64::EPSILON * value.abs().max(1.0) {
        nearest
    } else {
        value.floor()
    }
}

impl Layout {
    /// Construct a half split without requiring callers to repeat its default.
    pub fn branch(axis: Axis, first: Self, second: Self) -> Self {
        Self::Split {
            axis,
            ratio: 0.5,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Set a split selected by false=first / true=second child steps.
    /// Invalid paths and values leave the tree unchanged.
    pub fn set_ratio(&mut self, path: &[bool], value: f64) -> Result<(), String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("split ratio must be finite and between zero and one".into());
        }
        let mut node = self;
        for second_child in path {
            node = match node {
                Self::Split { first, second, .. } => {
                    if *second_child {
                        second
                    } else {
                        first
                    }
                }
                Self::Leaf(_) => return Err("split path does not exist".into()),
            };
        }
        match node {
            Self::Split { ratio, .. } => {
                *ratio = value;
                Ok(())
            }
            Self::Leaf(_) => Err("split path names a leaf".into()),
        }
    }

    /// Minimum grid dimensions, including all nested one-cell dividers.
    pub fn minimum_size(&self) -> (usize, usize) {
        match self {
            Self::Leaf(id) if id.is_empty() => (0, 0),
            Self::Leaf(_) => (MIN_COLUMNS, MIN_ROWS),
            Self::Split {
                axis,
                first,
                second,
                ..
            } => {
                let (ac, ar) = first.minimum_size();
                let (bc, br) = second.minimum_size();
                match axis {
                    Axis::Columns => (ac.saturating_add(bc).saturating_add(1), ar.max(br)),
                    Axis::Rows => (ac.max(bc), ar.saturating_add(br).saturating_add(1)),
                }
            }
        }
    }

    /// Construct a leaf. An empty ID represents an empty layout.
    pub fn leaf(id: impl Into<String>) -> Self {
        Self::Leaf(id.into())
    }

    /// Return nonempty IDs in first-child-before-second-child order.
    pub fn leaves(&self) -> Vec<String> {
        let mut leaves = Vec::new();
        self.collect_leaves(&mut leaves);
        leaves
    }

    fn collect_leaves(&self, leaves: &mut Vec<String>) {
        match self {
            Self::Leaf(id) if !id.is_empty() => leaves.push(id.clone()),
            Self::Leaf(_) => {}
            Self::Split { first, second, .. } => {
                first.collect_leaves(leaves);
                second.collect_leaves(leaves);
            }
        }
    }

    /// Whether a nonempty terminal ID belongs to this tree, even when invisible.
    pub fn contains(&self, id: &str) -> bool {
        if id.is_empty() {
            return false;
        }
        match self {
            Self::Leaf(leaf) => leaf == id,
            Self::Split { first, second, .. } => first.contains(id) || second.contains(id),
        }
    }

    /// Return visible content rectangles in leaf order, excluding divider cells.
    ///
    /// A tiny resize may give a child no cells; such leaves retain their IDs but
    /// are omitted here. If an input origin plus a child offset cannot fit in
    /// `usize`, that child is likewise omitted rather than wrapping coordinates.
    pub fn rectangles(&self, rect: Rect) -> Vec<(String, Rect)> {
        let mut panes = Vec::new();
        self.collect_rectangles(rect, &mut panes);
        panes
    }

    fn collect_rectangles(&self, rect: Rect, panes: &mut Vec<(String, Rect)>) {
        if rect.cols == 0 || rect.rows == 0 {
            return;
        }
        match self {
            Self::Leaf(id) if !id.is_empty() => panes.push((id.clone(), rect)),
            Self::Leaf(_) => {}
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let (a, b) = partition(
                    rect,
                    *axis,
                    *ratio,
                    first.minimum_size(),
                    second.minimum_size(),
                );
                first.collect_rectangles(a, panes);
                if let Some(b) = b {
                    second.collect_rectangles(b, panes);
                }
            }
        }
    }

    /// Replace `target` with two panes, preserving its ID as the first child.
    ///
    /// `rect` is the whole tree's rectangle, not the target pane's rectangle.
    /// Reject empty/duplicate IDs, missing or invisible targets, and any split
    /// unable to leave both children at least three rows/eight columns.
    /// Errors leave the tree unchanged.
    pub fn split(
        &mut self,
        target: &str,
        new_id: String,
        axis: Axis,
        rect: Rect,
    ) -> Result<(), String> {
        if new_id.is_empty() {
            return Err("terminal ID must not be empty".into());
        }
        if self.contains(&new_id) {
            return Err(format!("terminal ID already exists: {new_id}"));
        }
        let pane = self
            .rectangles(rect)
            .into_iter()
            .find_map(|(id, rect)| (id == target).then_some(rect))
            .ok_or_else(|| format!("target terminal is missing or invisible: {target}"))?;
        let extent = match axis {
            Axis::Columns => pane.cols,
            Axis::Rows => pane.rows,
        };
        let minimum = match axis {
            Axis::Columns => MIN_COLUMNS,
            Axis::Rows => MIN_ROWS,
        };
        if extent < 2 * minimum + 1 || pane.cols < MIN_COLUMNS || pane.rows < MIN_ROWS {
            return Err("split requires eight columns and three rows per pane".into());
        }
        // Also refuse a second child whose origin would overflow, even for a
        // caller-supplied rectangle extending beyond the coordinate domain.
        if partition(
            pane,
            axis,
            0.5,
            (MIN_COLUMNS, MIN_ROWS),
            (MIN_COLUMNS, MIN_ROWS),
        )
        .1
        .is_none()
        {
            return Err("split child origin exceeds cell coordinate range".into());
        }
        self.insert_split(target, new_id, axis);
        Ok(())
    }

    fn insert_split(&mut self, target: &str, new_id: String, axis: Axis) {
        match self {
            Self::Leaf(id) => {
                let old_id = std::mem::take(id);
                *self = Self::Split {
                    axis,
                    ratio: 0.5,
                    first: Box::new(Self::Leaf(old_id)),
                    second: Box::new(Self::Leaf(new_id)),
                };
            }
            Self::Split { first, second, .. } => {
                if first.contains(target) {
                    first.insert_split(target, new_id, axis);
                } else {
                    second.insert_split(target, new_id, axis);
                }
            }
        }
    }

    /// Remove a leaf and replace its parent with its sibling.
    ///
    /// Returns true exactly when a nonempty ID was found. Removing the last leaf
    /// leaves `Leaf("")`; the caller may then discard the containing window group.
    /// Surviving IDs and subtree order are unchanged.
    pub fn remove(&mut self, id: &str) -> bool {
        if id.is_empty() {
            return false;
        }
        match self {
            Self::Leaf(leaf) => {
                if leaf == id {
                    leaf.clear();
                    true
                } else {
                    false
                }
            }
            Self::Split { first, second, .. } => {
                let removed = first.remove(id) || second.remove(id);
                if removed {
                    if matches!(first.as_ref(), Self::Leaf(id) if id.is_empty()) {
                        *self = std::mem::replace(second.as_mut(), Self::Leaf(String::new()));
                    } else if matches!(second.as_ref(), Self::Leaf(id) if id.is_empty()) {
                        *self = std::mem::replace(first.as_mut(), Self::Leaf(String::new()));
                    }
                }
                removed
            }
        }
    }

    /// Find the visible pane containing a cell; dividers and outside cells miss.
    pub fn hit_test(&self, rect: Rect, col: usize, row: usize) -> Option<String> {
        self.rectangles(rect).into_iter().find_map(|(id, pane)| {
            // Subtraction avoids overflowing an exclusive right/bottom edge.
            let x = col.checked_sub(pane.col)?;
            let y = row.checked_sub(pane.row)?;
            (x < pane.cols && y < pane.rows).then_some(id)
        })
    }
}

/// Reserve a gutter even at tiny sizes, without subtracting below zero.
fn partition(
    rect: Rect,
    axis: Axis,
    ratio: f64,
    first_min: (usize, usize),
    second_min: (usize, usize),
) -> (Rect, Option<Rect>) {
    let extent = match axis {
        Axis::Columns => rect.cols,
        Axis::Rows => rect.rows,
    };
    let available = extent.saturating_sub(1);
    let ratio = if ratio.is_finite() {
        ratio.clamp(0.0, 1.0)
    } else {
        0.5
    };
    let ideal = if ratio == 0.5 {
        available / 2
    } else {
        stable_floor(available as f64 * ratio) as usize
    };
    let (min_a, min_b) = match axis {
        Axis::Columns => (first_min.0, second_min.0),
        Axis::Rows => (first_min.1, second_min.1),
    };
    let first_extent = if available >= min_a.saturating_add(min_b) {
        ideal.clamp(min_a, available - min_b)
    } else {
        ideal.min(available)
    };
    let second_extent = available - first_extent;
    let mut first = rect.clone();
    let mut second = rect;
    match axis {
        Axis::Columns => {
            first.cols = first_extent;
            second.cols = second_extent;
            let Some(col) = second
                .col
                .checked_add(first_extent)
                .and_then(|v| v.checked_add(1))
            else {
                return (first, None);
            };
            second.col = col;
        }
        Axis::Rows => {
            first.rows = first_extent;
            second.rows = second_extent;
            let Some(row) = second
                .row
                .checked_add(first_extent)
                .and_then(|v| v.checked_add(1))
            else {
                return (first, None);
            };
            second.row = row;
        }
    }
    (first, Some(second))
}
