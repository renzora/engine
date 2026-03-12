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
        let layouts = vec![
            WorkspaceLayout { name: "Scene".into(), tree: scene_layout() },
            WorkspaceLayout { name: "Blueprints".into(), tree: layout_blueprints() },
            WorkspaceLayout { name: "Scripting".into(), tree: layout_scripting() },
            WorkspaceLayout { name: "Animation".into(), tree: layout_animation() },
            WorkspaceLayout { name: "Materials".into(), tree: layout_materials() },
            WorkspaceLayout { name: "Level Design".into(), tree: layout_level_design() },
            WorkspaceLayout { name: "Terrain".into(), tree: layout_terrain() },
            WorkspaceLayout { name: "Particles".into(), tree: layout_particles() },
            WorkspaceLayout { name: "Shaders".into(), tree: layout_shaders() },
            WorkspaceLayout { name: "UI".into(), tree: layout_ui() },
            WorkspaceLayout { name: "Physics".into(), tree: layout_physics() },
            WorkspaceLayout { name: "Audio".into(), tree: layout_audio() },
            WorkspaceLayout { name: "Networking".into(), tree: layout_networking() },
            WorkspaceLayout { name: "Debug".into(), tree: layout_debug() },
        ];


        Self {
            layouts,
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

/// Scene: Viewport+bottom strip | Hierarchy(top)+Inspector(bottom)
pub fn scene_layout() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::Leaf {
                tabs: vec!["viewport".into(), "code_editor".into(), "node_explorer".into()],
                active_tab: 0,
            },
            DockTree::Leaf {
                tabs: vec!["assets".into(), "console".into(), "animation".into(), "mixer".into()],
                active_tab: 0,
            },
            0.72,
        ),
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::Leaf {
                tabs: vec!["inspector".into(), "history".into()],
                active_tab: 0,
            },
            0.25,
        ),
        0.82,
    )
}

/// Blueprints: Hierarchy+NodeProperties | BlueprintGraph+Console | Inspector
fn layout_blueprints() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::leaf("blueprint_properties"),
            0.5,
        ),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("blueprint_graph"),
                DockTree::leaf("console"),
                0.75,
            ),
            DockTree::leaf("inspector"),
            0.78,
        ),
        0.18,
    )
}

/// Scripting: Hierarchy+Assets | CodeEditor+Console | Inspector+ScriptVariables
fn layout_scripting() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::leaf("assets"),
            0.6,
        ),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("code_editor"),
                DockTree::leaf("console"),
                0.7,
            ),
            DockTree::vertical(
                DockTree::Leaf {
                    tabs: vec!["inspector".into(), "history".into()],
                    active_tab: 0,
                },
                DockTree::leaf("script_variables"),
                0.5,
            ),
            0.78,
        ),
        0.18,
    )
}

/// Animation: Hierarchy | StudioPreview+AnimationControls | Timeline
fn layout_animation() -> DockTree {
    DockTree::vertical(
        DockTree::horizontal(
            DockTree::leaf("hierarchy"),
            DockTree::horizontal(
                DockTree::leaf("studio_preview"),
                DockTree::leaf("animation"),
                0.75,
            ),
            0.15,
        ),
        DockTree::leaf("timeline"),
        0.65,
    )
}

/// Debug: Hierarchy+Performance | Viewport+debug panels | Inspector+EcsStats
fn layout_debug() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::leaf("performance"),
            0.6,
        ),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("viewport"),
                DockTree::horizontal(
                    DockTree::horizontal(
                        DockTree::leaf("system_profiler"),
                        DockTree::Leaf {
                            tabs: vec!["render_stats".into(), "render_pipeline".into()],
                            active_tab: 0,
                        },
                        0.5,
                    ),
                    DockTree::horizontal(
                        DockTree::leaf("memory_profiler"),
                        DockTree::horizontal(
                            DockTree::leaf("physics_debug"),
                            DockTree::leaf("camera_debug"),
                            0.5,
                        ),
                        0.33,
                    ),
                    0.4,
                ),
                0.65,
            ),
            DockTree::vertical(
                DockTree::Leaf {
                    tabs: vec!["inspector".into(), "gamepad".into()],
                    active_tab: 0,
                },
                DockTree::leaf("ecs_stats"),
                0.5,
            ),
            0.75,
        ),
        0.15,
    )
}

/// Materials: [Preview / Properties] | [MaterialGraph / (Assets+Console tabs)]
fn layout_materials() -> DockTree {
    DockTree::horizontal(
        // Left column: preview on top, properties below
        DockTree::vertical(
            DockTree::leaf("material_preview"),
            DockTree::leaf("material_inspector"),
            0.4,
        ),
        // Right column: graph on top, assets+console tabbed below
        DockTree::vertical(
            DockTree::leaf("material_graph"),
            DockTree::Leaf {
                tabs: vec!["assets".into(), "console".into()],
                active_tab: 0,
            },
            0.7,
        ),
        0.25,
    )
}

/// Level Design: Hierarchy+Assets | Viewport | ShapeLibrary+Inspector
fn layout_level_design() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::leaf("assets"),
            0.55,
        ),
        DockTree::horizontal(
            DockTree::leaf("viewport"),
            DockTree::vertical(
                DockTree::leaf("shape_library"),
                DockTree::leaf("inspector"),
                0.4,
            ),
            0.82,
        ),
        0.14,
    )
}

/// Terrain: TerrainTools | Viewport
fn layout_terrain() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("terrain_tools"),
        DockTree::leaf("viewport"),
        0.2,
    )
}

/// Particles: ParticlePreview | ParticleEditor
pub fn layout_particles() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("particle_preview"),
        DockTree::leaf("particle_editor"),
        0.8,
    )
}

/// Particles Advanced: ParticleGraph | Preview / Editor
pub fn layout_particles_advanced() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("particle_graph"),
        DockTree::vertical(
            DockTree::leaf("particle_preview"),
            DockTree::leaf("particle_editor"),
            0.5,
        ),
        0.75,
    )
}

/// Shaders: Assets+Console | CodeEditor | ShaderPreview
fn layout_shaders() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("assets"),
            DockTree::leaf("console"),
            0.6,
        ),
        DockTree::horizontal(
            DockTree::leaf("code_editor"),
            DockTree::leaf("shader_preview"),
            0.6,
        ),
        0.18,
    )
}

/// Physics: Hierarchy | Viewport+all physics tabs | Inspector+Shapes
/// UI: Hierarchy+WidgetLibrary | Canvas+Console | UiInspector+Assets
fn layout_ui() -> DockTree {
    DockTree::horizontal(
        // Left: hierarchy on top, widget palette below
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::leaf("widget_library"),
            0.5,
        ),
        DockTree::horizontal(
            // Center: UI canvas on top, assets+console below
            DockTree::vertical(
                DockTree::Leaf {
                    tabs: vec!["ui_canvas".into(), "viewport".into()],
                    active_tab: 0,
                },
                DockTree::Leaf {
                    tabs: vec!["assets".into(), "console".into()],
                    active_tab: 0,
                },
                0.75,
            ),
            // Right: UI inspector + scene inspector
            DockTree::Leaf {
                tabs: vec!["ui_inspector".into(), "inspector".into()],
                active_tab: 0,
            },
            0.75,
        ),
        0.15,
    )
}

/// Audio: DAW timeline + Mixer + Assets
fn layout_audio() -> DockTree {
    DockTree::vertical(
        // Top: hierarchy | DAW timeline | inspector
        DockTree::horizontal(
            DockTree::leaf("hierarchy"),
            DockTree::horizontal(
                DockTree::leaf("daw"),
                DockTree::leaf("inspector"),
                0.78,
            ),
            0.15,
        ),
        // Bottom: mixer | assets + console
        DockTree::horizontal(
            DockTree::leaf("mixer"),
            DockTree::Leaf {
                tabs: vec!["assets".into(), "console".into()],
                active_tab: 0,
            },
            0.6,
        ),
        0.6,
    )
}

/// Networking: Hierarchy | Viewport+NetworkMonitor | Inspector+NetworkEntities
fn layout_networking() -> DockTree {
    DockTree::horizontal(
        // Left: hierarchy + network entities
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::leaf("network_entities"),
            0.5,
        ),
        DockTree::horizontal(
            // Center: viewport on top, network monitor + console below
            DockTree::vertical(
                DockTree::leaf("viewport"),
                DockTree::Leaf {
                    tabs: vec!["network_monitor".into(), "console".into()],
                    active_tab: 0,
                },
                0.65,
            ),
            // Right: inspector + network settings
            DockTree::vertical(
                DockTree::leaf("inspector"),
                DockTree::leaf("network_settings"),
                0.5,
            ),
            0.75,
        ),
        0.18,
    )
}

fn layout_physics() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("hierarchy"),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("viewport"),
                DockTree::Leaf {
                    tabs: vec![
                        "physics_playground".into(),
                        "physics_scenarios".into(),
                        "arena_presets".into(),
                        "physics_forces".into(),
                        "physics_properties".into(),
                        "physics_debug".into(),
                        "physics_metrics".into(),
                        "console".into(),
                    ],
                    active_tab: 0,
                },
                0.72,
            ),
            DockTree::vertical(
                DockTree::leaf("inspector"),
                DockTree::leaf("shape_library"),
                0.5,
            ),
            0.75,
        ),
        0.15,
    )
}

