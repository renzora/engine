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
            WorkspaceLayout { name: "Sandbox".into(), tree: layout_sandbox() },
            WorkspaceLayout { name: "Terrain".into(), tree: layout_terrain() },
            WorkspaceLayout { name: "Particles".into(), tree: layout_particles() },
            WorkspaceLayout { name: "Shaders".into(), tree: layout_shaders() },
            WorkspaceLayout { name: "UI".into(), tree: layout_ui() },
            WorkspaceLayout { name: "Physics".into(), tree: layout_physics() },
            WorkspaceLayout { name: "Audio".into(), tree: layout_audio() },
            WorkspaceLayout { name: "Lifecycle".into(), tree: layout_lifecycle() },
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

/// Scene: Hierarchy+Shapes | Viewport+BottomTabs | Inspector+History
pub fn scene_layout() -> DockTree {
    DockTree::horizontal(
        // Left column: hierarchy on top, shape library below
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::leaf("shape_library"),
            0.6,
        ),
        DockTree::horizontal(
            // Center: viewport on top, assets/console/properties/mixer tabbed below
            DockTree::vertical(
                DockTree::Leaf {
                    tabs: vec!["viewport".into(), "code_editor".into(), "node_explorer".into()],
                    active_tab: 0,
                },
                DockTree::Leaf {
                    tabs: vec!["assets".into(), "console".into(), "properties".into(), "mixer".into()],
                    active_tab: 0,
                },
                0.72,
            ),
            // Right column: inspector with history tab
            DockTree::Leaf {
                tabs: vec!["inspector".into(), "history".into()],
                active_tab: 0,
            },
            0.78,
        ),
        0.15,
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

/// Animation: Hierarchy | StudioPreview | Properties | Timeline
fn layout_animation() -> DockTree {
    DockTree::vertical(
        DockTree::horizontal(
            DockTree::leaf("hierarchy"),
            DockTree::horizontal(
                DockTree::leaf("studio_preview"),
                DockTree::leaf("animation"),
                0.78,
            ),
            0.15,
        ),
        DockTree::leaf("timeline"),
        0.60,
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

/// Materials: Hierarchy | MaterialGraph + (Assets+Console) | Preview + Properties
fn layout_materials() -> DockTree {
    DockTree::horizontal(
        // Left column: hierarchy (full height)
        DockTree::leaf("hierarchy"),
        DockTree::horizontal(
            // Center: graph on top, assets+console tabbed below
            DockTree::vertical(
                DockTree::leaf("material_graph"),
                DockTree::Leaf {
                    tabs: vec!["assets".into(), "console".into()],
                    active_tab: 0,
                },
                0.7,
            ),
            // Right column: preview on top, properties below
            DockTree::vertical(
                DockTree::leaf("material_preview"),
                DockTree::leaf("material_inspector"),
                0.5,
            ),
            0.75,
        ),
        0.15,
    )
}

/// Sandbox: Hierarchy+Assets | Viewport | ShapeLibrary+Inspector
fn layout_sandbox() -> DockTree {
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

/// Shaders: ShaderEditor+CompilerLog | Preview+Properties
fn layout_shaders() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("shader_editor"),
            DockTree::leaf("shader_compiler_log"),
            0.5,
        ),
        DockTree::vertical(
            DockTree::leaf("shader_preview"),
            DockTree::leaf("shader_properties"),
            0.5,
        ),
        0.55,
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

/// Lifecycle: Hierarchy+NetworkEntities | LifecycleGraph+Monitor+Console | Properties+Settings
fn layout_lifecycle() -> DockTree {
    DockTree::horizontal(
        // Left: hierarchy + network entities
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::leaf("network_entities"),
            0.5,
        ),
        DockTree::horizontal(
            // Center: lifecycle graph on top, monitor + console below
            DockTree::vertical(
                DockTree::leaf("lifecycle_graph"),
                DockTree::Leaf {
                    tabs: vec!["lifecycle_monitor".into(), "console".into()],
                    active_tab: 0,
                },
                0.7,
            ),
            // Right: node properties + lifecycle settings
            DockTree::vertical(
                DockTree::leaf("lifecycle_properties"),
                DockTree::leaf("lifecycle_settings"),
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

