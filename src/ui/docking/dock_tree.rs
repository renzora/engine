//! Core dock tree data structures
//!
//! The dock tree represents the hierarchical layout of panels using a binary tree structure.
//! Each node is either a Split (dividing space between two children) or a Leaf (containing tabs).

use serde::{Deserialize, Serialize};
use egui_phosphor::regular::{
    TREE_STRUCTURE, SLIDERS_HORIZONTAL, FOLDER_OPEN, TERMINAL,
    MONITOR, FILM_STRIP, CODE, CLOCK_COUNTER_CLOCKWISE, PUZZLE_PIECE,
    GRAPH, LIST_BULLETS, GEAR, CUBE, GAME_CONTROLLER, CHART_LINE, CPU,
    STACK, CHART_BAR, ATOM, VIDEO_CAMERA, TIMER,
};

/// Direction of a split in the dock tree
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDirection {
    Horizontal, // Children are side by side (left/right)
    Vertical,   // Children are stacked (top/bottom)
}

/// Identifies a panel type in the editor
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelId {
    Hierarchy,
    Inspector,
    Assets,
    Console,
    Viewport,
    Animation,
    ScriptEditor,
    History,
    Blueprint,
    NodeLibrary,
    MaterialPreview,
    Settings,
    Gamepad,
    Performance,
    RenderStats,
    EcsStats,
    MemoryProfiler,
    PhysicsDebug,
    CameraDebug,
    SystemProfiler,
    /// Custom plugin-provided panel
    Plugin(String),
}

impl PanelId {
    /// Get the display title for this panel
    pub fn title(&self) -> &str {
        match self {
            PanelId::Hierarchy => "Hierarchy",
            PanelId::Inspector => "Inspector",
            PanelId::Assets => "Assets",
            PanelId::Console => "Console",
            PanelId::Viewport => "Viewport",
            PanelId::Animation => "Animation",
            PanelId::ScriptEditor => "Script Editor",
            PanelId::History => "History",
            PanelId::Blueprint => "Blueprint",
            PanelId::NodeLibrary => "Node Library",
            PanelId::MaterialPreview => "Material Preview",
            PanelId::Settings => "Settings",
            PanelId::Gamepad => "Gamepad",
            PanelId::Performance => "Performance",
            PanelId::RenderStats => "Render Stats",
            PanelId::EcsStats => "ECS Stats",
            PanelId::MemoryProfiler => "Memory",
            PanelId::PhysicsDebug => "Physics Debug",
            PanelId::CameraDebug => "Camera Debug",
            PanelId::SystemProfiler => "System Profiler",
            PanelId::Plugin(name) => name,
        }
    }

    /// Get the icon for this panel (Phosphor icons)
    pub fn icon(&self) -> &'static str {
        match self {
            PanelId::Hierarchy => TREE_STRUCTURE,
            PanelId::Inspector => SLIDERS_HORIZONTAL,
            PanelId::Assets => FOLDER_OPEN,
            PanelId::Console => TERMINAL,
            PanelId::Viewport => MONITOR,
            PanelId::Animation => FILM_STRIP,
            PanelId::ScriptEditor => CODE,
            PanelId::History => CLOCK_COUNTER_CLOCKWISE,
            PanelId::Blueprint => GRAPH,
            PanelId::NodeLibrary => LIST_BULLETS,
            PanelId::MaterialPreview => CUBE,
            PanelId::Settings => GEAR,
            PanelId::Gamepad => GAME_CONTROLLER,
            PanelId::Performance => CHART_LINE,
            PanelId::RenderStats => CPU,
            PanelId::EcsStats => STACK,
            PanelId::MemoryProfiler => CHART_BAR,
            PanelId::PhysicsDebug => ATOM,
            PanelId::CameraDebug => VIDEO_CAMERA,
            PanelId::SystemProfiler => TIMER,
            PanelId::Plugin(_) => PUZZLE_PIECE,
        }
    }

    /// Check if this panel can be closed (some panels like Viewport shouldn't be closeable)
    pub fn can_close(&self) -> bool {
        !matches!(self, PanelId::Viewport)
    }
}

/// A node in the dock tree - either a split or a leaf containing tabs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DockTree {
    /// A split node dividing space between two children
    Split {
        direction: SplitDirection,
        /// Ratio of first child's size (0.0 to 1.0)
        ratio: f32,
        /// First child (left or top)
        first: Box<DockTree>,
        /// Second child (right or bottom)
        second: Box<DockTree>,
    },
    /// A leaf node containing tabbed panels
    Leaf {
        /// List of panels in this tab group
        tabs: Vec<PanelId>,
        /// Index of the currently active tab
        active_tab: usize,
    },
    /// An empty node (placeholder during drag operations)
    Empty,
}

impl Default for DockTree {
    fn default() -> Self {
        // Default layout: Hierarchy | Viewport+Assets | Inspector
        DockTree::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.15,
            first: Box::new(DockTree::Leaf {
                tabs: vec![PanelId::Hierarchy],
                active_tab: 0,
            }),
            second: Box::new(DockTree::Split {
                direction: SplitDirection::Horizontal,
                ratio: 0.75, // Center takes 75% of remaining space
                first: Box::new(DockTree::Split {
                    direction: SplitDirection::Vertical,
                    ratio: 0.7,
                    first: Box::new(DockTree::Leaf {
                        tabs: vec![PanelId::Viewport],
                        active_tab: 0,
                    }),
                    second: Box::new(DockTree::Leaf {
                        tabs: vec![PanelId::Assets, PanelId::Console, PanelId::Animation],
                        active_tab: 0,
                    }),
                }),
                second: Box::new(DockTree::Leaf {
                    tabs: vec![PanelId::Inspector, PanelId::History],
                    active_tab: 0,
                }),
            }),
        }
    }
}

impl DockTree {
    /// Create a leaf with a single panel
    pub fn leaf(panel: PanelId) -> Self {
        DockTree::Leaf {
            tabs: vec![panel],
            active_tab: 0,
        }
    }

    /// Create a horizontal split (left/right)
    pub fn horizontal(first: DockTree, second: DockTree, ratio: f32) -> Self {
        DockTree::Split {
            direction: SplitDirection::Horizontal,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Create a vertical split (top/bottom)
    pub fn vertical(first: DockTree, second: DockTree, ratio: f32) -> Self {
        DockTree::Split {
            direction: SplitDirection::Vertical,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Find a leaf containing the given panel and return a mutable reference
    pub fn find_leaf_mut(&mut self, panel: &PanelId) -> Option<&mut DockTree> {
        match self {
            DockTree::Split { first, second, .. } => {
                first.find_leaf_mut(panel).or_else(|| second.find_leaf_mut(panel))
            }
            DockTree::Leaf { tabs, .. } => {
                if tabs.contains(panel) {
                    Some(self)
                } else {
                    None
                }
            }
            DockTree::Empty => None,
        }
    }

    /// Find a leaf containing the given panel
    #[allow(dead_code)]
    pub fn find_leaf(&self, panel: &PanelId) -> Option<&DockTree> {
        match self {
            DockTree::Split { first, second, .. } => {
                first.find_leaf(panel).or_else(|| second.find_leaf(panel))
            }
            DockTree::Leaf { tabs, .. } => {
                if tabs.contains(panel) {
                    Some(self)
                } else {
                    None
                }
            }
            DockTree::Empty => None,
        }
    }

    /// Remove a panel from the tree, cleaning up empty leaves
    pub fn remove_panel(&mut self, panel: &PanelId) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                // Try to remove from children
                if first.remove_panel(panel) || second.remove_panel(panel) {
                    // Clean up empty nodes
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

    /// Add a panel as a tab to the leaf containing target_panel
    pub fn add_tab(&mut self, target_panel: &PanelId, new_panel: PanelId) -> bool {
        if let Some(leaf) = self.find_leaf_mut(target_panel) {
            if let DockTree::Leaf { tabs, active_tab } = leaf {
                tabs.push(new_panel);
                *active_tab = tabs.len() - 1;
                return true;
            }
        }
        false
    }

    /// Split a leaf containing target_panel and add new_panel in the specified direction
    pub fn split_at(&mut self, target_panel: &PanelId, new_panel: PanelId, direction: SplitDirection, insert_first: bool) -> bool {
        self.split_at_recursive(target_panel, new_panel, direction, insert_first)
    }

    fn split_at_recursive(&mut self, target_panel: &PanelId, new_panel: PanelId, direction: SplitDirection, insert_first: bool) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                // Check if target is directly in first or second child
                let in_first = first.contains_panel(target_panel);
                let in_second = second.contains_panel(target_panel);

                if in_first {
                    if let DockTree::Leaf { .. } = first.as_ref() {
                        // Replace first with a split
                        let old_first = std::mem::replace(first.as_mut(), DockTree::Empty);
                        let new_leaf = DockTree::leaf(new_panel);
                        *first = Box::new(if insert_first {
                            DockTree::Split {
                                direction,
                                ratio: 0.5,
                                first: Box::new(new_leaf),
                                second: Box::new(old_first),
                            }
                        } else {
                            DockTree::Split {
                                direction,
                                ratio: 0.5,
                                first: Box::new(old_first),
                                second: Box::new(new_leaf),
                            }
                        });
                        return true;
                    } else {
                        return first.split_at_recursive(target_panel, new_panel, direction, insert_first);
                    }
                }

                if in_second {
                    if let DockTree::Leaf { .. } = second.as_ref() {
                        // Replace second with a split
                        let old_second = std::mem::replace(second.as_mut(), DockTree::Empty);
                        let new_leaf = DockTree::leaf(new_panel);
                        *second = Box::new(if insert_first {
                            DockTree::Split {
                                direction,
                                ratio: 0.5,
                                first: Box::new(new_leaf),
                                second: Box::new(old_second),
                            }
                        } else {
                            DockTree::Split {
                                direction,
                                ratio: 0.5,
                                first: Box::new(old_second),
                                second: Box::new(new_leaf),
                            }
                        });
                        return true;
                    } else {
                        return second.split_at_recursive(target_panel, new_panel, direction, insert_first);
                    }
                }

                false
            }
            DockTree::Leaf { .. } => {
                // This is the target leaf - replace self with a split
                let old_self = std::mem::replace(self, DockTree::Empty);
                let new_leaf = DockTree::leaf(new_panel);
                *self = if insert_first {
                    DockTree::Split {
                        direction,
                        ratio: 0.5,
                        first: Box::new(new_leaf),
                        second: Box::new(old_self),
                    }
                } else {
                    DockTree::Split {
                        direction,
                        ratio: 0.5,
                        first: Box::new(old_self),
                        second: Box::new(new_leaf),
                    }
                };
                true
            }
            DockTree::Empty => false,
        }
    }

    /// Check if this tree contains the given panel
    pub fn contains_panel(&self, panel: &PanelId) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                first.contains_panel(panel) || second.contains_panel(panel)
            }
            DockTree::Leaf { tabs, .. } => tabs.contains(panel),
            DockTree::Empty => false,
        }
    }

    /// Get all panels in the tree
    #[allow(dead_code)]
    pub fn all_panels(&self) -> Vec<PanelId> {
        let mut panels = Vec::new();
        self.collect_panels(&mut panels);
        panels
    }

    #[allow(dead_code)]
    fn collect_panels(&self, panels: &mut Vec<PanelId>) {
        match self {
            DockTree::Split { first, second, .. } => {
                first.collect_panels(panels);
                second.collect_panels(panels);
            }
            DockTree::Leaf { tabs, .. } => {
                panels.extend(tabs.iter().cloned());
            }
            DockTree::Empty => {}
        }
    }

    /// Clean up empty leaves and collapse single-child splits
    fn cleanup_empty(&mut self) {
        match self {
            DockTree::Split { first, second, .. } => {
                // Recursively clean children
                first.cleanup_empty();
                second.cleanup_empty();

                // If first is empty, replace self with second
                if matches!(first.as_ref(), DockTree::Empty) ||
                   matches!(first.as_ref(), DockTree::Leaf { tabs, .. } if tabs.is_empty()) {
                    let second_val = std::mem::replace(second.as_mut(), DockTree::Empty);
                    *self = second_val;
                }
                // If second is empty, replace self with first
                else if matches!(second.as_ref(), DockTree::Empty) ||
                        matches!(second.as_ref(), DockTree::Leaf { tabs, .. } if tabs.is_empty()) {
                    let first_val = std::mem::replace(first.as_mut(), DockTree::Empty);
                    *self = first_val;
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

    /// Update the split ratio for a split at the given path
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

    /// Set the active tab for a leaf containing the given panel
    pub fn set_active_tab(&mut self, panel: &PanelId) {
        if let Some(leaf) = self.find_leaf_mut(panel) {
            if let DockTree::Leaf { tabs, active_tab } = leaf {
                if let Some(idx) = tabs.iter().position(|t| t == panel) {
                    *active_tab = idx;
                }
            }
        }
    }

    /// Count total number of leaves (tab groups) in the tree
    #[allow(dead_code)]
    pub fn leaf_count(&self) -> usize {
        match self {
            DockTree::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
            DockTree::Leaf { .. } => 1,
            DockTree::Empty => 0,
        }
    }
}

/// Represents where a drop will occur
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    /// Add as a new tab to existing leaf
    Tab,
    /// Split and place on the left
    Left,
    /// Split and place on the right
    Right,
    /// Split and place on top
    Top,
    /// Split and place on bottom
    Bottom,
}

impl DropZone {
    /// Convert to split direction and whether to insert first
    #[allow(dead_code)]
    pub fn to_split_params(self) -> Option<(SplitDirection, bool)> {
        match self {
            DropZone::Tab => None,
            DropZone::Left => Some((SplitDirection::Horizontal, true)),
            DropZone::Right => Some((SplitDirection::Horizontal, false)),
            DropZone::Top => Some((SplitDirection::Vertical, true)),
            DropZone::Bottom => Some((SplitDirection::Vertical, false)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_layout() {
        let tree = DockTree::default();
        assert!(tree.contains_panel(&PanelId::Hierarchy));
        assert!(tree.contains_panel(&PanelId::Viewport));
        assert!(tree.contains_panel(&PanelId::Inspector));
        assert!(tree.contains_panel(&PanelId::Assets));
    }

    #[test]
    fn test_remove_panel() {
        let mut tree = DockTree::default();
        assert!(tree.contains_panel(&PanelId::Console));
        tree.remove_panel(&PanelId::Console);
        assert!(!tree.contains_panel(&PanelId::Console));
    }

    #[test]
    fn test_add_tab() {
        let mut tree = DockTree::leaf(PanelId::Viewport);
        tree.add_tab(&PanelId::Viewport, PanelId::Assets);

        if let DockTree::Leaf { tabs, active_tab } = tree {
            assert_eq!(tabs.len(), 2);
            assert_eq!(active_tab, 1); // New tab is active
        } else {
            panic!("Expected leaf");
        }
    }
}
