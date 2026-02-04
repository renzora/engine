//! Layout management for the docking system
//!
//! Handles saving, loading, and built-in layout presets.

use super::dock_tree::{DockTree, PanelId};
use serde::{Deserialize, Serialize};

/// A saved workspace layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceLayout {
    /// Name of the layout
    pub name: String,
    /// The dock tree structure
    pub dock_tree: DockTree,
    /// Whether this is a built-in layout (cannot be deleted)
    pub is_builtin: bool,
}

impl WorkspaceLayout {
    #[allow(dead_code)]
    pub fn new(name: impl Into<String>, dock_tree: DockTree) -> Self {
        Self {
            name: name.into(),
            dock_tree,
            is_builtin: false,
        }
    }

    pub fn builtin(name: impl Into<String>, dock_tree: DockTree) -> Self {
        Self {
            name: name.into(),
            dock_tree,
            is_builtin: true,
        }
    }
}

/// Get all built-in layouts
pub fn builtin_layouts() -> Vec<WorkspaceLayout> {
    vec![
        WorkspaceLayout::builtin("Scene", default_layout()),
        WorkspaceLayout::builtin("Scripting", scripting_layout()),
        WorkspaceLayout::builtin("Animation", animation_layout()),
        WorkspaceLayout::builtin("Debug", debug_layout()),
        WorkspaceLayout::builtin("Blueprints", blueprints_layout()),
        WorkspaceLayout::builtin("Level Design", level_design_layout()),
        WorkspaceLayout::builtin("Terrain", terrain_layout()),
        WorkspaceLayout::builtin("Image Preview", image_preview_layout()),
    ]
}

/// Scene layout: Hierarchy | Viewport/NodeExplorer | Inspector with Assets/Console/Animation at bottom
///
/// ```text
/// ┌─────────┬──────────────────────┬──────────┐
/// │         │                      │          │
/// │Hierarchy│ Viewport|NodeExplorer│ Inspector│
/// │         │                      │          │
/// ├─────────┴──────────────────────┴──────────┤
/// │       Assets | Console | Animation        │
/// └───────────────────────────────────────────┘
/// ```
pub fn default_layout() -> DockTree {
    DockTree::vertical(
        DockTree::horizontal(
            DockTree::leaf(PanelId::Hierarchy),
            DockTree::horizontal(
                DockTree::Leaf {
                    tabs: vec![PanelId::Viewport, PanelId::NodeExplorer],
                    active_tab: 0,
                },
                DockTree::Leaf {
                    tabs: vec![PanelId::Inspector, PanelId::History],
                    active_tab: 0,
                },
                0.78,
            ),
            0.18,
        ),
        DockTree::Leaf {
            tabs: vec![PanelId::Assets, PanelId::Console, PanelId::Animation],
            active_tab: 0,
        },
        0.72,
    )
}

/// Scripting layout: Hierarchy+Assets | ScriptEditor+Console | Inspector
///
/// ```text
/// ┌─────────┬──────────────────────┬──────────┐
/// │         │                      │          │
/// │Hierarchy│    Script Editor     │ Inspector│
/// │         │                      │          │
/// ├─────────┼──────────────────────┤          │
/// │ Assets  │      Console         │          │
/// └─────────┴──────────────────────┴──────────┘
/// ```
pub fn scripting_layout() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf(PanelId::Hierarchy),
            DockTree::leaf(PanelId::Assets),
            0.6,
        ),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf(PanelId::ScriptEditor),
                DockTree::leaf(PanelId::Console),
                0.7,
            ),
            DockTree::Leaf {
                tabs: vec![PanelId::Inspector, PanelId::History],
                active_tab: 0,
            },
            0.78,
        ),
        0.18,
    )
}

/// Animation layout: Full animation editing workspace
///
/// ```text
/// ┌─────────────┬──────────────────────────────┬─────────────────┐
/// │             │                              │                 │
/// │             │                              │   Animation     │
/// │  Hierarchy  │       Studio Preview         │   Controls      │
/// │  (full ht)  │      (studio lighting)       │   (full ht)     │
/// │             │                              │                 │
/// │             │                              │   - Clip list   │
/// │             │                              │   - Properties  │
/// ├─────────────┴──────────────────────────────┴─────────────────┤
/// │                           Timeline                           │
/// │  [<<][<][▶][■][>][>>] | 00:01.234 | [🔁]  ────▼────────────  │
/// │  ▶ Position                                                  │
/// │    Pos.X [M] ────◆────────◆──────────────◆─────────────────  │
/// └──────────────────────────────────────────────────────────────┘
/// ```
pub fn animation_layout() -> DockTree {
    DockTree::vertical(
        // Top section: Hierarchy | Studio Preview | Animation controls
        DockTree::horizontal(
            DockTree::leaf(PanelId::Hierarchy),
            DockTree::horizontal(
                DockTree::leaf(PanelId::StudioPreview), // Isolated studio preview with lighting
                DockTree::leaf(PanelId::Animation), // Animation controls panel (right)
                0.75,
            ),
            0.15,
        ),
        // Bottom section: Timeline (full width)
        DockTree::leaf(PanelId::Timeline),
        0.65, // 65% for top section, 35% for timeline
    )
}

/// Debug layout: Hierarchy+Console | Viewport | Inspector/Debug panels
///
/// ```text
/// ┌──────────────────┬─────────────┬─────────────┐
/// │                  │             │  Inspector  │
/// │    Hierarchy     │   Viewport  │   Gamepad   │
/// │                  │             ├─────────────┤
/// ├──────────────────┤             │ Performance │
/// │     Console      │             │ ECS Stats   │
/// │                  │             │ Memory, etc │
/// └──────────────────┴─────────────┴─────────────┘
/// ```
pub fn debug_layout() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf(PanelId::Hierarchy),
            DockTree::leaf(PanelId::Console),
            0.6,
        ),
        DockTree::horizontal(
            DockTree::leaf(PanelId::Viewport),
            DockTree::vertical(
                DockTree::Leaf {
                    tabs: vec![PanelId::Inspector, PanelId::Gamepad, PanelId::CameraDebug],
                    active_tab: 0,
                },
                DockTree::Leaf {
                    tabs: vec![
                        PanelId::Performance,
                        PanelId::RenderStats,
                        PanelId::EcsStats,
                        PanelId::MemoryProfiler,
                        PanelId::PhysicsDebug,
                        PanelId::SystemProfiler,
                    ],
                    active_tab: 0,
                },
                0.5,
            ),
            0.72,
        ),
        0.18,
    )
}

/// Blueprints layout: Visual scripting focused with large blueprint editor
///
/// ```text
/// ┌───────────┬──────────────────────┬───────────┐
/// │  Material │                      │           │
/// │  Preview  │   Blueprint Editor   │   Node    │
/// ├───────────┤                      │  Library  │
/// │           ├──────────────────────┤           │
/// │   Assets  │      Console         │           │
/// └───────────┴──────────────────────┴───────────┘
/// ```
pub fn blueprints_layout() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf(PanelId::MaterialPreview),
            DockTree::leaf(PanelId::Assets),
            0.4,
        ),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf(PanelId::Blueprint),
                DockTree::leaf(PanelId::Console),
                0.7,
            ),
            DockTree::leaf(PanelId::NodeLibrary),
            0.8,
        ),
        0.15,
    )
}

/// Level Design layout: Maximized viewport with Level Tools panel and quick access to hierarchy
///
/// ```text
/// ┌─────────────────────────────────┬──────────┐
/// │                                 │  Level   │
/// │            Viewport             │  Tools   │
/// │                                 ├──────────┤
/// │                                 │ Hierarchy│
/// │                                 ├──────────┤
/// │                                 │  Assets  │
/// ├─────────────────────────────────┼──────────┤
/// │            Console              │ Inspector│
/// └─────────────────────────────────┴──────────┘
/// ```
pub fn level_design_layout() -> DockTree {
    DockTree::vertical(
        DockTree::horizontal(
            DockTree::leaf(PanelId::Viewport),
            DockTree::vertical(
                DockTree::leaf(PanelId::LevelTools),
                DockTree::vertical(
                    DockTree::leaf(PanelId::Hierarchy),
                    DockTree::leaf(PanelId::Assets),
                    0.5,
                ),
                0.4,
            ),
            0.8,
        ),
        DockTree::horizontal(
            DockTree::leaf(PanelId::Console),
            DockTree::leaf(PanelId::Inspector),
            0.75,
        ),
        0.75,
    )
}

/// Terrain layout: Level Tools on left, Viewport on right
///
/// ```text
/// ┌──────────┬───────────────────────────────────┐
/// │  Level   │                                   │
/// │  Tools   │            Viewport               │
/// │          │                                   │
/// └──────────┴───────────────────────────────────┘
/// ```
pub fn terrain_layout() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf(PanelId::LevelTools),
        DockTree::leaf(PanelId::Viewport),
        0.2,
    )
}

/// Image Preview layout: Image viewer with assets for browsing
///
/// ```text
/// ┌───────────┬──────────────────────┬───────────┐
/// │           │                      │           │
/// │ Hierarchy │    Image Preview     │ Inspector │
/// │           │                      │           │
/// ├───────────┴──────────────────────┴───────────┤
/// │              Assets | Console                │
/// └──────────────────────────────────────────────┘
/// ```
pub fn image_preview_layout() -> DockTree {
    DockTree::vertical(
        DockTree::horizontal(
            DockTree::leaf(PanelId::Hierarchy),
            DockTree::horizontal(
                DockTree::leaf(PanelId::ImagePreview),
                DockTree::leaf(PanelId::Inspector),
                0.78,
            ),
            0.18,
        ),
        DockTree::Leaf {
            tabs: vec![PanelId::Assets, PanelId::Console],
            active_tab: 0,
        },
        0.72,
    )
}

/// Minimal layout: Just viewport
#[allow(dead_code)]
pub fn minimal_layout() -> DockTree {
    DockTree::leaf(PanelId::Viewport)
}

/// Layout configuration for serialization
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DockingLayoutConfig {
    /// Current layout name (references saved layouts)
    pub active_layout: String,
    /// Custom saved layouts
    pub custom_layouts: Vec<WorkspaceLayout>,
    /// Current dock tree (for restoring exact state)
    pub current_tree: Option<DockTree>,
}

impl DockingLayoutConfig {
    /// Get a layout by name (checks custom first, then builtins)
    #[allow(dead_code)]
    pub fn get_layout(&self, name: &str) -> Option<&DockTree> {
        // Check custom layouts first
        for layout in &self.custom_layouts {
            if layout.name == name {
                return Some(&layout.dock_tree);
            }
        }

        // Check builtins
        for layout in builtin_layouts() {
            if layout.name == name {
                // Return a reference by matching - we need to handle this differently
                return None; // Caller should use builtin_layouts() directly
            }
        }

        None
    }

    /// Save a custom layout
    #[allow(dead_code)]
    pub fn save_custom_layout(&mut self, name: String, tree: DockTree) {
        // Remove existing layout with same name if it exists
        self.custom_layouts.retain(|l| l.name != name);
        self.custom_layouts.push(WorkspaceLayout::new(name, tree));
    }

    /// Delete a custom layout
    #[allow(dead_code)]
    pub fn delete_layout(&mut self, name: &str) -> bool {
        let len_before = self.custom_layouts.len();
        self.custom_layouts.retain(|l| l.name != name);
        self.custom_layouts.len() < len_before
    }

    /// Get all available layouts (builtin + custom)
    #[allow(dead_code)]
    pub fn all_layouts(&self) -> Vec<String> {
        let mut names: Vec<String> = builtin_layouts().iter().map(|l| l.name.clone()).collect();
        names.extend(self.custom_layouts.iter().map(|l| l.name.clone()));
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_layouts_contain_required_panels() {
        let layouts = builtin_layouts();

        for layout in layouts {
            // All layouts should contain at least a viewport
            assert!(
                layout.dock_tree.contains_panel(&PanelId::Viewport) ||
                layout.dock_tree.contains_panel(&PanelId::ScriptEditor),
                "Layout '{}' should contain Viewport or ScriptEditor",
                layout.name
            );
        }
    }

    #[test]
    fn test_default_layout_structure() {
        let layout = default_layout();

        assert!(layout.contains_panel(&PanelId::Hierarchy));
        assert!(layout.contains_panel(&PanelId::Viewport));
        assert!(layout.contains_panel(&PanelId::Inspector));
        assert!(layout.contains_panel(&PanelId::Assets));
        assert!(layout.contains_panel(&PanelId::Console));
    }
}
