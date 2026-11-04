//! Server-owned workspace, shared by all attached clients.
use crate::layout::Layout;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub revision: u64,
    pub groups: Vec<WorkspaceGroup>,
    pub active: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceGroup {
    pub layout: Layout,
    pub focus: String,
}

impl Workspace {
    pub(crate) fn empty() -> Self {
        Self {
            revision: 0,
            groups: Vec::new(),
            active: 0,
        }
    }

    pub(crate) fn add(&mut self, id: String) {
        self.groups.push(WorkspaceGroup {
            layout: Layout::leaf(id.clone()),
            focus: id,
        });
        self.revision += 1;
    }

    pub(crate) fn remove(&mut self, id: &str) {
        let active_id = self.groups.get(self.active).map(|g| g.focus.clone());
        for group in &mut self.groups {
            if group.layout.remove(id) && !group.layout.contains(&group.focus) {
                group.focus = group.layout.leaves().into_iter().next().unwrap_or_default();
            }
        }
        self.groups.retain(|g| !g.focus.is_empty());
        self.active = active_id
            .and_then(|id| self.groups.iter().position(|g| g.layout.contains(&id)))
            .unwrap_or_else(|| self.active.min(self.groups.len().saturating_sub(1)));
        self.revision += 1;
    }

    pub(crate) fn valid<'a>(&self, live: impl Iterator<Item = &'a String>) -> bool {
        if self.groups.len() > 64
            || (self.groups.is_empty() && self.active != 0)
            || (!self.groups.is_empty() && self.active >= self.groups.len())
        {
            return false;
        }
        let mut seen = BTreeSet::new();
        let mut nodes = 0;
        for group in &self.groups {
            let mut pending = vec![(&group.layout, 1)];
            let mut focused = false;
            while let Some((node, depth)) = pending.pop() {
                nodes += 1;
                if nodes > 127 || depth > 16 {
                    return false;
                }
                match node {
                    Layout::Leaf(id) => {
                        focused |= id == &group.focus;
                        if id.is_empty() || !seen.insert(id) {
                            return false;
                        }
                    }
                    Layout::Split { first, second, ratio, .. } => {
                        if !ratio.is_finite() || !(0.0..=1.0).contains(ratio) { return false; }
                        pending.push((first, depth + 1));
                        pending.push((second, depth + 1));
                    }
                }
            }
            if !focused {
                return false;
            }
        }
        seen == live.collect()
    }
}
