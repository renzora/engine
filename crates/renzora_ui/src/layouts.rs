//! Workspace layout presets and layout manager.

use bevy::prelude::*;

use crate::dock_tree::{DockTree, DockingState};

/// A named workspace layout.
#[derive(Clone)]
pub struct WorkspaceLayout {
    pub name: String,
    pub tree: DockTree,
}

/// Resource managing available workspace layouts.
#[derive(Resource, Clone)]
pub struct LayoutManager {
    pub layouts: Vec<WorkspaceLayout>,
    pub active_index: usize,
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self {
            layouts: vec![
                WorkspaceLayout {
                    name: "Default".into(),
                    tree: layout_default(),
                },
                WorkspaceLayout {
                    name: "Scripting".into(),
                    tree: layout_scripting(),
                },
                WorkspaceLayout {
                    name: "Debug".into(),
                    tree: layout_debug(),
                },
                WorkspaceLayout {
                    name: "Minimal".into(),
                    tree: layout_minimal(),
                },
            ],
            active_index: 0,
        }
    }
}

impl LayoutManager {
    /// Name of the currently active layout.
    pub fn active_name(&self) -> &str {
        self.layouts
            .get(self.active_index)
            .map(|l| l.name.as_str())
            .unwrap_or("Default")
    }

    /// Switch to a layout by index, replacing the docking tree.
    pub fn switch(&mut self, index: usize, docking: &mut DockingState) {
        if let Some(layout) = self.layouts.get(index) {
            docking.tree = layout.tree.clone();
            self.active_index = index;
        }
    }
}

/// Default: Hierarchy (15%) | Viewport / Assets+Console (70%) | Inspector (remaining)
fn layout_default() -> DockTree {
    crate::dock_tree::default_layout()
}

/// Scripting: Hierarchy (15%) | Viewport / Console (50/50) | Code Editor + Inspector tabs (35%)
fn layout_scripting() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("hierarchy"),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("viewport"),
                DockTree::leaf("console"),
                0.5,
            ),
            DockTree::Leaf {
                tabs: vec!["code_editor".into(), "inspector".into()],
                active_tab: 0,
            },
            0.65,
        ),
        0.15,
    )
}

/// Debug: Hierarchy (12%) | Viewport / Console+Performance tabs (55%) | Inspector / Physics Debug (18%)
fn layout_debug() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("hierarchy"),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("viewport"),
                DockTree::Leaf {
                    tabs: vec!["console".into(), "performance".into()],
                    active_tab: 0,
                },
                0.55,
            ),
            DockTree::vertical(
                DockTree::leaf("inspector"),
                DockTree::leaf("physics_debug"),
                0.55,
            ),
            0.75,
        ),
        0.12,
    )
}

/// Minimal: Viewport only (full area).
fn layout_minimal() -> DockTree {
    DockTree::leaf("viewport")
}
