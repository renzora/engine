//! Dock tree data model — a binary tree of `Split`s and `Leaf`s describing a
//! dockable panel layout.
//!
//! This is the reusable, UI-framework-agnostic half of docking (no bevy, no
//! editor specifics): the tree shape + the mutations a dock UI performs
//! (resize, tab move/insert, split, collapse). The bevy_ui reconciler and
//! interaction systems that render and drive it live alongside this in
//! `renzora_ember`; consumers (the editor shell, games) supply the layouts and
//! per-panel content.

/// Direction of a split in the dock tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Children are side-by-side (left / right).
    Horizontal,
    /// Children are stacked (top / bottom).
    Vertical,
}

/// A node in the dock tree.
#[derive(Debug, Clone)]
pub enum DockTree {
    Split {
        direction: SplitDirection,
        /// Fraction of space given to the first child (0.1–0.9).
        ratio: f32,
        first: Box<DockTree>,
        second: Box<DockTree>,
    },
    Leaf {
        /// Panel IDs shown as tabs.
        tabs: Vec<String>,
        /// Index of the currently visible tab.
        active_tab: usize,
    },
    Empty,
}

impl DockTree {
    /// Single-tab leaf.
    pub fn leaf(id: impl Into<String>) -> Self {
        DockTree::Leaf {
            tabs: vec![id.into()],
            active_tab: 0,
        }
    }

    /// A leaf with several tabbed panels.
    pub fn tabs(tabs: &[&str]) -> Self {
        DockTree::Leaf {
            tabs: tabs.iter().map(|s| s.to_string()).collect(),
            active_tab: 0,
        }
    }

    /// Horizontal split (left / right).
    pub fn horizontal(first: DockTree, second: DockTree, ratio: f32) -> Self {
        DockTree::Split {
            direction: SplitDirection::Horizontal,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Vertical split (top / bottom).
    pub fn vertical(first: DockTree, second: DockTree, ratio: f32) -> Self {
        DockTree::Split {
            direction: SplitDirection::Vertical,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Set the split ratio at `path` — a chain of branch choices from the root
    /// (`false` = descend into the first child, `true` = the second). An empty
    /// path targets this node. Used to persist a divider drag.
    pub fn update_ratio(&mut self, path: &[bool], new_ratio: f32) {
        if let DockTree::Split {
            ratio,
            first,
            second,
            ..
        } = self
        {
            match path.split_first() {
                Some((&true, rest)) => second.update_ratio(rest, new_ratio),
                Some((&false, rest)) => first.update_ratio(rest, new_ratio),
                None => *ratio = new_ratio.clamp(0.1, 0.9),
            }
        }
    }

    /// The leaf that contains `panel`, if any.
    pub fn find_leaf_mut(&mut self, panel: &str) -> Option<&mut DockTree> {
        match self {
            DockTree::Split { first, second, .. } => first
                .find_leaf_mut(panel)
                .or_else(|| second.find_leaf_mut(panel)),
            DockTree::Leaf { tabs, .. } => tabs.iter().any(|t| t == panel).then_some(self),
            DockTree::Empty => None,
        }
    }

    /// Is `panel` the active (visible) tab in its leaf?
    pub fn is_active_tab(&self, panel: &str) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                first.is_active_tab(panel) || second.is_active_tab(panel)
            }
            DockTree::Leaf { tabs, active_tab } => {
                tabs.get(*active_tab).is_some_and(|t| t == panel)
            }
            DockTree::Empty => false,
        }
    }

    /// Make `panel` the active tab in its leaf.
    pub fn set_active_tab(&mut self, panel: &str) {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(panel) {
            if let Some(idx) = tabs.iter().position(|t| t == panel) {
                *active_tab = idx;
            }
        }
    }

    /// Append `new_panel` as a tab in the leaf containing `sibling`.
    pub fn add_tab(&mut self, sibling: &str, new_panel: String) -> bool {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(sibling) {
            tabs.push(new_panel);
            *active_tab = tabs.len() - 1;
            true
        } else {
            false
        }
    }

    /// Insert `new_panel` into `sibling`'s leaf immediately before `before`
    /// (or at the end if `before` is `None` / not present). Used for precise
    /// tab-bar drop insertion.
    pub fn add_tab_before(&mut self, sibling: &str, new_panel: String, before: Option<&str>) -> bool {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(sibling) {
            let idx = before
                .and_then(|b| tabs.iter().position(|t| t == b))
                .unwrap_or(tabs.len())
                .min(tabs.len());
            tabs.insert(idx, new_panel);
            *active_tab = idx;
            true
        } else {
            false
        }
    }

    /// Remove a panel from the tree, collapsing any emptied leaves/splits.
    pub fn remove_panel(&mut self, panel: &str) -> bool {
        let removed = match self {
            DockTree::Split { first, second, .. } => {
                first.remove_panel(panel) || second.remove_panel(panel)
            }
            DockTree::Leaf { tabs, active_tab } => {
                if let Some(idx) = tabs.iter().position(|t| t == panel) {
                    tabs.remove(idx);
                    if *active_tab >= tabs.len() && !tabs.is_empty() {
                        *active_tab = tabs.len() - 1;
                    }
                    true
                } else {
                    false
                }
            }
            DockTree::Empty => false,
        };
        if removed {
            self.cleanup_empty();
        }
        removed
    }

    /// Collapse empty leaves and splits with an empty side.
    fn cleanup_empty(&mut self) {
        if let DockTree::Split { first, second, .. } = self {
            first.cleanup_empty();
            second.cleanup_empty();
            let first_empty = first.is_empty();
            let second_empty = second.is_empty();
            if first_empty {
                *self = std::mem::replace(second, DockTree::Empty);
            } else if second_empty {
                *self = std::mem::replace(first, DockTree::Empty);
            }
        } else if let DockTree::Leaf { tabs, .. } = self {
            if tabs.is_empty() {
                *self = DockTree::Empty;
            }
        }
    }

    fn is_empty(&self) -> bool {
        matches!(self, DockTree::Empty)
            || matches!(self, DockTree::Leaf { tabs, .. } if tabs.is_empty())
    }

    /// Split the leaf containing `target`, placing `new_panel` on the given
    /// side. (Use [`add_tab`](Self::add_tab) for the center/tab case.)
    pub fn split_at(&mut self, target: &str, new_panel: String, zone: DropZone) -> bool {
        if let Some(leaf) = self.find_leaf_mut(target) {
            let old = std::mem::replace(leaf, DockTree::Empty);
            let new_leaf = DockTree::leaf(new_panel);
            *leaf = match zone {
                DropZone::Left => DockTree::horizontal(new_leaf, old, 0.5),
                DropZone::Right => DockTree::horizontal(old, new_leaf, 0.5),
                DropZone::Top => DockTree::vertical(new_leaf, old, 0.5),
                DropZone::Bottom => DockTree::vertical(old, new_leaf, 0.5),
                DropZone::Center => {
                    *leaf = old;
                    return false;
                }
            };
            true
        } else {
            false
        }
    }
}

/// Where a dragged panel will land on a leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    /// Add as a tab in the target leaf.
    Center,
    Left,
    Right,
    Top,
    Bottom,
}
