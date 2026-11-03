//! Session-only terminal layout in cell coordinates, independent of PTYs and rendering.
//!
//! Each branch reserves one divider cell and gives the first child the smaller
//! half of the remaining extent. Resizing never changes leaf IDs. Empty IDs are
//! reserved for an empty layout after its final leaf is removed.

/// A pane's content rectangle in zero-based cell coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rect {
    pub col: usize,
    pub row: usize,
    pub cols: usize,
    pub rows: usize,
}

/// The dimension divided by a split (columns for side-by-side panes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Columns,
    Rows,
}

/// A binary tree whose nonempty leaves are stable terminal IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Layout {
    Leaf(String),
    Split {
        axis: Axis,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}

impl Layout {
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
                first,
                second,
            } => {
                let (a, b) = partition(rect, *axis);
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
    /// unable to leave both children at least one cell plus a one-cell divider.
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
        if extent < 3 {
            return Err("split requires two content cells and one divider cell".into());
        }
        // Also refuse a second child whose origin would overflow, even for a
        // caller-supplied rectangle extending beyond the coordinate domain.
        if partition(pane, axis).1.is_none() {
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
fn partition(rect: Rect, axis: Axis) -> (Rect, Option<Rect>) {
    let extent = match axis {
        Axis::Columns => rect.cols,
        Axis::Rows => rect.rows,
    };
    let available = extent.saturating_sub(1);
    let first_extent = available / 2;
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
