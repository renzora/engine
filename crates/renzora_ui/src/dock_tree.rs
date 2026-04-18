//! Dock tree data structure
//!
//! Binary tree representing the editor panel layout. Each node is a Split (dividing
//! space between two children) or a Leaf (containing string-based tab IDs).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Direction of a split in the dock tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDirection {
    /// Children are side-by-side (left / right).
    Horizontal,
    /// Children are stacked (top / bottom).
    Vertical,
}

/// A node in the dock tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DockTree {
    Split {
        direction: SplitDirection,
        /// Fraction of space given to the first child (0.0–1.0).
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

    /// Find a mutable reference to the leaf that contains `panel`.
    pub fn find_leaf_mut(&mut self, panel: &str) -> Option<&mut DockTree> {
        match self {
            DockTree::Split { first, second, .. } => {
                first.find_leaf_mut(panel).or_else(|| second.find_leaf_mut(panel))
            }
            DockTree::Leaf { tabs, .. } => {
                if tabs.iter().any(|t| t == panel) {
                    Some(self)
                } else {
                    None
                }
            }
            DockTree::Empty => None,
        }
    }

    /// Remove a panel tab from the tree, cleaning up empty leaves.
    pub fn remove_panel(&mut self, panel: &str) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                if first.remove_panel(panel) || second.remove_panel(panel) {
                    self.cleanup_empty();
                    true
                } else {
                    false
                }
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
        }
    }

    /// Set the active tab for the leaf that contains `panel`.
    pub fn set_active_tab(&mut self, panel: &str) {
        if let Some(leaf) = self.find_leaf_mut(panel) {
            if let DockTree::Leaf { tabs, active_tab } = leaf {
                if let Some(idx) = tabs.iter().position(|t| t == panel) {
                    *active_tab = idx;
                }
            }
        }
    }

    /// Update the split ratio at the given tree path.
    pub fn update_ratio(&mut self, path: &[bool], new_ratio: f32) {
        if path.is_empty() {
            if let DockTree::Split { ratio, .. } = self {
                *ratio = new_ratio.clamp(0.1, 0.9);
            }
            return;
        }
        if let DockTree::Split { first, second, .. } = self {
            if path[0] {
                second.update_ratio(&path[1..], new_ratio);
            } else {
                first.update_ratio(&path[1..], new_ratio);
            }
        }
    }

    /// Does the tree contain a panel with this ID?
    pub fn contains_panel(&self, panel: &str) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                first.contains_panel(panel) || second.contains_panel(panel)
            }
            DockTree::Leaf { tabs, .. } => tabs.iter().any(|t| t == panel),
            DockTree::Empty => false,
        }
    }

    /// Collapse empty leaves and single-child splits.
    fn cleanup_empty(&mut self) {
        match self {
            DockTree::Split { first, second, .. } => {
                first.cleanup_empty();
                second.cleanup_empty();

                let first_empty = matches!(first.as_ref(), DockTree::Empty)
                    || matches!(first.as_ref(), DockTree::Leaf { tabs, .. } if tabs.is_empty());
                let second_empty = matches!(second.as_ref(), DockTree::Empty)
                    || matches!(second.as_ref(), DockTree::Leaf { tabs, .. } if tabs.is_empty());

                if first_empty {
                    *self = std::mem::replace(second.as_mut(), DockTree::Empty);
                } else if second_empty {
                    *self = std::mem::replace(first.as_mut(), DockTree::Empty);
                }
            }
            DockTree::Leaf { tabs, .. } => {
                if tabs.is_empty() {
                    *self = DockTree::Empty;
                }
            }
            DockTree::Empty => {}
        }
    }

    /// Add a tab to the leaf containing `sibling`, at the end.
    pub fn add_tab(&mut self, sibling: &str, new_panel: String) -> bool {
        if let Some(leaf) = self.find_leaf_mut(sibling) {
            if let DockTree::Leaf { tabs, active_tab } = leaf {
                tabs.push(new_panel);
                *active_tab = tabs.len() - 1;
                return true;
            }
        }
        false
    }

    /// Add a tab at a specific index within its leaf.
    pub fn add_tab_at(&mut self, sibling: &str, new_panel: String, index: usize) -> bool {
        if let Some(leaf) = self.find_leaf_mut(sibling) {
            if let DockTree::Leaf { tabs, active_tab } = leaf {
                let idx = index.min(tabs.len());
                tabs.insert(idx, new_panel);
                *active_tab = idx;
                return true;
            }
        }
        false
    }

    /// Split the leaf containing `target` and place `new_panel` in the given direction.
    pub fn split_at(&mut self, target: &str, new_panel: String, zone: DropZone) -> bool {
        if let Some(leaf) = self.find_leaf_mut(target) {
            let old = std::mem::replace(leaf, DockTree::Empty);
            let new_leaf = DockTree::leaf(new_panel);
            *leaf = match zone {
                DropZone::Left => DockTree::horizontal(new_leaf, old, 0.5),
                DropZone::Right => DockTree::horizontal(old, new_leaf, 0.5),
                DropZone::Top => DockTree::vertical(new_leaf, old, 0.5),
                DropZone::Bottom => DockTree::vertical(old, new_leaf, 0.5),
                // Center and Tab should use add_tab instead
                _ => {
                    *leaf = old;
                    return false;
                }
            };
            return true;
        }
        false
    }

    /// Reorder a tab within its leaf (same leaf, different index).
    pub fn reorder_tab(&mut self, panel: &str, new_index: usize) -> bool {
        if let Some(leaf) = self.find_leaf_mut(panel) {
            if let DockTree::Leaf { tabs, active_tab } = leaf {
                if let Some(old_idx) = tabs.iter().position(|t| t == panel) {
                    let panel_id = tabs.remove(old_idx);
                    let idx = new_index.min(tabs.len());
                    tabs.insert(idx, panel_id);
                    *active_tab = idx;
                    return true;
                }
            }
        }
        false
    }

    /// Collect all panel IDs in the tree.
    pub fn all_panels(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_panels(&mut out);
        out
    }

    fn collect_panels(&self, out: &mut Vec<String>) {
        match self {
            DockTree::Split { first, second, .. } => {
                first.collect_panels(out);
                second.collect_panels(out);
            }
            DockTree::Leaf { tabs, .. } => {
                out.extend(tabs.iter().cloned());
            }
            DockTree::Empty => {}
        }
    }
}

/// Where a dragged tab will be dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    /// Insert as a tab at the given index.
    Tab(usize),
    Left,
    Right,
    Top,
    Bottom,
    /// Add as tab at end.
    Center,
}

/// Default layout:
/// ```text
/// Hierarchy (15%) | Viewport (70% top)   | Inspector (remaining)
///                 | Assets / Console (30%)|
/// ```
pub fn default_layout() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("hierarchy"),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("viewport"),
                DockTree::Leaf {
                    tabs: vec!["assets".into(), "console".into()],
                    active_tab: 0,
                },
                0.7,
            ),
            DockTree::leaf("inspector"),
            0.75,
        ),
        0.15,
    )
}

/// Bevy resource holding the current docking layout.
#[derive(Resource)]
pub struct DockingState {
    pub tree: DockTree,
}

impl Default for DockingState {
    fn default() -> Self {
        // Use the Scene layout from LayoutManager so they start in sync.
        Self {
            tree: crate::layouts::scene_layout(),
        }
    }
}

// ── Layout persistence ─────────────────────────────────────────────────────
//
// The user's in-progress dock layout is auto-saved whenever panels are moved
// or added, and restored on next launch. Storage is a single TOML file in
// the user config dir — not per-project, so layouts follow the user across
// projects.

/// Path to the persisted layout file. Returns `None` if the user config
/// directory is unavailable (rare — sandboxed environments).
#[cfg(not(target_arch = "wasm32"))]
pub fn layout_config_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("renzora").join("layout.toml"))
}

#[cfg(target_arch = "wasm32")]
pub fn layout_config_path() -> Option<std::path::PathBuf> {
    None
}

/// Load the last-saved workspace (all layouts + active index) from disk.
/// Returns `None` if nothing is saved, the file is corrupt, or we're on
/// a platform without filesystem access.
pub fn load_saved_workspace() -> Option<crate::layouts::LayoutManager> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = layout_config_path()?;
        let content = std::fs::read_to_string(&path).ok()?;
        toml::from_str::<crate::layouts::LayoutManager>(&content).ok()
    }
    #[cfg(target_arch = "wasm32")]
    { None }
}

/// Persist the workspace (all layouts + active index) to the user config
/// file. Errors are logged but not propagated.
pub fn save_workspace(manager: &crate::layouts::LayoutManager) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = layout_config_path() else { return };
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                warn!("[dock] couldn't create layout config dir: {e}");
                return;
            }
        }
        match toml::to_string_pretty(manager) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    warn!("[dock] couldn't save workspace: {e}");
                }
            }
            Err(e) => warn!("[dock] couldn't serialise workspace: {e}"),
        }
    }
    #[cfg(target_arch = "wasm32")]
    { let _ = manager; }
}

/// Delete the saved workspace file (used by "Reset Layout").
pub fn delete_saved_workspace() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(path) = layout_config_path() {
            let _ = std::fs::remove_file(path);
        }
    }
}
