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
        WorkspaceLayout::builtin("Particles", particles_layout()),
        WorkspaceLayout::builtin("Pixels", pixels_layout()),
        WorkspaceLayout::builtin("Shaders", shaders_layout()),
        WorkspaceLayout::builtin("Physics", physics_layout()),
    ]
}

/// Scene layout: Hierarchy+Assets | Viewport | ShapeLibrary+Inspector
///
/// ```text
/// ┌──────────┬──────────────────────┬───────────┐
/// │          │                      │  Shape    │
/// │Hierarchy │      Viewport        │  Library  │
/// │          │                      │           │
/// ├──────────┤                      ├───────────┤
/// │          │                      │           │
/// │ Assets   │                      │ Inspector │
/// └──────────┴──────────────────────┴───────────┘
/// ```
pub fn default_layout() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf(PanelId::Hierarchy),
            DockTree::leaf(PanelId::Assets),
            0.55,
        ),
        DockTree::horizontal(
            DockTree::Leaf {
                tabs: vec![PanelId::Viewport, PanelId::NodeExplorer],
                active_tab: 0,
            },
            DockTree::vertical(
                DockTree::leaf(PanelId::ShapeLibrary),
                DockTree::Leaf {
                    tabs: vec![PanelId::Inspector, PanelId::History],
                    active_tab: 0,
                },
                0.4,
            ),
            0.82,
        ),
        0.14,
    )
}

/// Scripting layout: Hierarchy+Assets | CodeEditor+Console | Inspector+ScriptVariables
///
/// ```text
/// ┌─────────┬──────────────────────┬──────────────┐
/// │         │                      │  Inspector   │
/// │Hierarchy│    Script Editor     │              │
/// │         │                      ├──────────────┤
/// ├─────────┼──────────────────────┤   Script     │
/// │ Assets  │      Console         │  Variables   │
/// └─────────┴──────────────────────┴──────────────┘
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
                DockTree::leaf(PanelId::CodeEditor),
                DockTree::leaf(PanelId::Console),
                0.7,
            ),
            DockTree::vertical(
                DockTree::Leaf {
                    tabs: vec![PanelId::Inspector, PanelId::History],
                    active_tab: 0,
                },
                DockTree::leaf(PanelId::ScriptVariables),
                0.5,
            ),
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

/// Debug layout: Hierarchy+Performance | Viewport+Debug tabs | Inspector/Gamepad+EcsStats
///
/// ```text
/// ┌──────────┬─────┬───────┬──────┬────────┬────────┬──────────────────┐
/// │          │                                      │ Inspector|Gamepad │
/// │Hierarchy │              Viewport                │                  │
/// │          │                                      ├──────────────────┤
/// ├──────────┼─────┬───────┬──────┬────────┬────────┤                  │
/// │          │ Sys │Render │      │Physics │Camera  │    ECS Stats     │
/// │Performanc│Prof │Stats  │Memory│ Debug  │ Debug  │                  │
/// └──────────┴─────┴───────┴──────┴────────┴────────┴──────────────────┘
/// ```
pub fn debug_layout() -> DockTree {
    DockTree::horizontal(
        // Left column: Hierarchy on top, Performance on bottom
        DockTree::vertical(
            DockTree::leaf(PanelId::Hierarchy),
            DockTree::leaf(PanelId::Performance),
            0.6,
        ),
        DockTree::horizontal(
            // Center column: Viewport on top, 5 debug panels side-by-side on bottom
            DockTree::vertical(
                DockTree::leaf(PanelId::Viewport),
                DockTree::horizontal(
                    DockTree::horizontal(
                        DockTree::leaf(PanelId::SystemProfiler),
                        DockTree::Leaf {
                            tabs: vec![PanelId::RenderStats, PanelId::RenderPipeline],
                            active_tab: 0,
                        },
                        0.5,
                    ),
                    DockTree::horizontal(
                        DockTree::leaf(PanelId::MemoryProfiler),
                        DockTree::horizontal(
                            DockTree::leaf(PanelId::PhysicsDebug),
                            DockTree::leaf(PanelId::CameraDebug),
                            0.5,
                        ),
                        0.33,
                    ),
                    0.4,
                ),
                0.65,
            ),
            // Right column: Inspector/Gamepad on top, ECS Stats on bottom
            DockTree::vertical(
                DockTree::Leaf {
                    tabs: vec![PanelId::Inspector, PanelId::Gamepad],
                    active_tab: 0,
                },
                DockTree::leaf(PanelId::EcsStats),
                0.5,
            ),
            0.75,
        ),
        0.15,
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
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf(PanelId::Hierarchy),
            DockTree::leaf(PanelId::Assets),
            0.55,
        ),
        DockTree::horizontal(
            DockTree::leaf(PanelId::Viewport),
            DockTree::vertical(
                DockTree::leaf(PanelId::ShapeLibrary),
                DockTree::leaf(PanelId::Inspector),
                0.4,
            ),
            0.82,
        ),
        0.14,
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

/// Particles layout: Particle Preview on left, Particle Editor on right
///
/// ```text
/// ┌──────────────────────┬───────────────────────┐
/// │                      │                       │
/// │   Particle Preview   │    Particle Editor    │
/// │                      │                       │
/// │                      │                       │
/// └──────────────────────┴───────────────────────┘
/// ```
pub fn particles_layout() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf(PanelId::ParticlePreview),
        DockTree::leaf(PanelId::ParticleEditor),
        0.8,
    )
}

/// Pixels layout: Pixel art editor workspace
///
/// ```text
/// ┌──────────┬──────────────────────┬──────────┐
/// │PixelTools│                      │PixelLayer│
/// │          │                      │          │
/// ├──────────┤    PixelCanvas       ├──────────┤
/// │PixelBrush│                      │PixelPalet│
/// │ Settings │                      │          │
/// ├──────────┴──────────────────────┴──────────┤
/// │              PixelTimeline                  │
/// └─────────────────────────────────────────────┘
/// ```
pub fn pixels_layout() -> DockTree {
    DockTree::vertical(
        DockTree::horizontal(
            DockTree::leaf(PanelId::PixelCanvas),
            DockTree::vertical(
                DockTree::leaf(PanelId::PixelLayers),
                DockTree::vertical(
                    DockTree::leaf(PanelId::PixelBrushSettings),
                    DockTree::leaf(PanelId::PixelPalette),
                    0.45,
                ),
                0.35,
            ),
            0.78,
        ),
        DockTree::leaf(PanelId::PixelTimeline),
        0.78,
    )
}

/// Shaders layout: Code Editor with Shader Preview side by side
///
/// ```text
/// ┌───────────┬──────────────────────┬──────────────┐
/// │           │                      │              │
/// │  Assets   │    Code Editor       │   Shader     │
/// │           │                      │   Preview    │
/// ├───────────┼──────────────────────┤              │
/// │  Console  │                      │              │
/// └───────────┴──────────────────────┴──────────────┘
/// ```
pub fn shaders_layout() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf(PanelId::Assets),
            DockTree::leaf(PanelId::Console),
            0.6,
        ),
        DockTree::horizontal(
            DockTree::leaf(PanelId::CodeEditor),
            DockTree::leaf(PanelId::ShaderPreview),
            0.6,
        ),
        0.18,
    )
}

/// Physics layout: Comprehensive physics testing workspace
///
/// ```text
/// ┌──────────┬──────────────────────┬──────────────────────────┐
/// │          │                      │ Hierarchy|Scenarios      │
/// │          │      Viewport        │                          │
/// │          │                      ├──────────────────────────┤
/// │          │                      │ PhyDebug|PhyProps|Metrics│
/// │          ├──────────────────────┤                          │
/// │          │Console|Playground    ├──────────────────────────┤
/// │          │      |StressTest     │Forces|CollViz|Trails     │
/// │          │                      │      |Recorder           │
/// └──────────┴──────────────────────┴──────────────────────────┘
/// ```
pub fn physics_layout() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf(PanelId::Hierarchy),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf(PanelId::Viewport),
                DockTree::Leaf {
                    tabs: vec![PanelId::Console, PanelId::PhysicsPlayground, PanelId::StressTest, PanelId::ArenaPresets],
                    active_tab: 0,
                },
                0.72,
            ),
            DockTree::vertical(
                DockTree::Leaf {
                    tabs: vec![PanelId::Inspector, PanelId::PhysicsScenarios],
                    active_tab: 0,
                },
                DockTree::vertical(
                    DockTree::Leaf {
                        tabs: vec![PanelId::PhysicsDebug, PanelId::PhysicsProperties, PanelId::PhysicsMetrics],
                        active_tab: 0,
                    },
                    DockTree::Leaf {
                        tabs: vec![PanelId::PhysicsForces, PanelId::CollisionViz, PanelId::MovementTrails, PanelId::StateRecorder],
                        active_tab: 0,
                    },
                    0.5,
                ),
                0.4,
            ),
            0.68,
        ),
        0.15,
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
    use std::collections::HashSet;

    #[test]
    fn test_builtin_layouts_contain_required_panels() {
        let layouts = builtin_layouts();

        for layout in layouts {
            // All layouts should have at least one panel
            let panels = layout.dock_tree.all_panels();
            assert!(!panels.is_empty(),
                "Layout '{}' should contain at least one panel",
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

    #[test]
    fn test_builtin_layouts_count() {
        let layouts = builtin_layouts();
        assert_eq!(layouts.len(), 12, "Expected 12 built-in layouts");
    }

    #[test]
    fn test_all_builtin_layouts_are_builtin() {
        for layout in builtin_layouts() {
            assert!(layout.is_builtin, "Layout '{}' should be marked as builtin", layout.name);
        }
    }

    #[test]
    fn test_workspace_layout_new_is_not_builtin() {
        let layout = WorkspaceLayout::new("Custom", DockTree::leaf(PanelId::Viewport));
        assert!(!layout.is_builtin);
        assert_eq!(layout.name, "Custom");
    }

    #[test]
    fn test_builtin_layouts_have_unique_names() {
        let layouts = builtin_layouts();
        let mut names = HashSet::new();
        for layout in &layouts {
            assert!(names.insert(&layout.name), "Duplicate layout name: {}", layout.name);
        }
    }

    #[test]
    fn test_scripting_layout_has_code_editor() {
        let layout = scripting_layout();
        assert!(layout.contains_panel(&PanelId::CodeEditor),
            "Scripting layout should contain CodeEditor");
    }

    #[test]
    fn test_debug_layout_has_performance_panels() {
        let layout = debug_layout();
        assert!(layout.contains_panel(&PanelId::Performance),
            "Debug layout should contain Performance panel");
        assert!(layout.contains_panel(&PanelId::EcsStats),
            "Debug layout should contain EcsStats panel");
    }

    #[test]
    fn test_blueprints_layout_has_blueprint_editor() {
        let layout = blueprints_layout();
        assert!(layout.contains_panel(&PanelId::Blueprint),
            "Blueprints layout should contain Blueprint panel");
        assert!(layout.contains_panel(&PanelId::NodeLibrary),
            "Blueprints layout should contain NodeLibrary panel");
    }

    #[test]
    fn test_docking_layout_config_default() {
        let config = DockingLayoutConfig::default();
        assert!(config.custom_layouts.is_empty());
        assert!(config.current_tree.is_none());
        assert!(config.active_layout.is_empty());
    }

    #[test]
    fn test_docking_layout_config_save_and_delete() {
        let mut config = DockingLayoutConfig::default();
        config.save_custom_layout("Test".to_string(), DockTree::leaf(PanelId::Viewport));
        assert_eq!(config.custom_layouts.len(), 1);
        assert!(!config.custom_layouts[0].is_builtin);

        // Delete it
        assert!(config.delete_layout("Test"));
        assert!(config.custom_layouts.is_empty());

        // Delete non-existent
        assert!(!config.delete_layout("NonExistent"));
    }
}
