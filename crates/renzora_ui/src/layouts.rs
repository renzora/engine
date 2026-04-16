//! Workspace layout presets and layout manager.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::dock_tree::{DockTree, DockingState};

/// A named workspace layout.
#[derive(Clone, Serialize, Deserialize)]
pub struct WorkspaceLayout {
    pub name: String,
    pub tree: DockTree,
}

/// Resource managing available workspace layouts.
#[derive(Resource, Clone, Serialize, Deserialize)]
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
            WorkspaceLayout { name: "Particles".into(), tree: layout_particles() },
            WorkspaceLayout { name: "Shaders".into(), tree: layout_shaders() },
            WorkspaceLayout { name: "UI".into(), tree: layout_ui() },
            WorkspaceLayout { name: "Physics".into(), tree: layout_physics() },
            WorkspaceLayout { name: "Audio".into(), tree: layout_audio() },
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

    /// Switch to a layout by index. The previous layout's current state is
    /// saved back into its slot first so user edits persist across
    /// switches, then the new layout's tree becomes the active dock.
    pub fn switch(&mut self, index: usize, docking: &mut DockingState) {
        if let Some(current) = self.layouts.get_mut(self.active_index) {
            current.tree = docking.tree.clone();
        }
        if let Some(layout) = self.layouts.get(index) {
            docking.tree = layout.tree.clone();
            self.active_index = index;
        }
    }

    /// Reset the active layout's tree to its hardcoded factory default.
    /// Other layouts are untouched.
    pub fn reset_active(&mut self, docking: &mut DockingState) {
        let defaults = Self::default();
        let Some(active) = self.layouts.get(self.active_index) else { return };
        let Some(default) = defaults
            .layouts
            .iter()
            .find(|l| l.name == active.name)
            .map(|l| l.tree.clone())
        else {
            return;
        };
        docking.tree = default.clone();
        if let Some(active) = self.layouts.get_mut(self.active_index) {
            active.tree = default;
        }
    }
}

/// Scene: Hierarchy+Shapes | Viewport+BottomTabs | Inspector+History
pub fn scene_layout() -> DockTree {
    DockTree::horizontal(
        // Left column: hierarchy+scenes tabbed on top, tool settings + shape library tabbed below
        DockTree::vertical(
            DockTree::Leaf {
                tabs: vec!["hierarchy".into(), "scenes".into()],
                active_tab: 0,
            },
            DockTree::Leaf {
                tabs: vec!["tool_settings".into(), "shape_library".into()],
                active_tab: 0,
            },
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
                    tabs: vec!["assets".into(), "hub_store".into(), "console".into(), "mixer".into()],
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
    // Left column: Hierarchy / (Scripts+Outline tabbed) / Assets   (~16%)
    // Center column: Code editor over (Console+Problems tabbed)    (~59%)
    // Right column: Viewport over Script Variables                 (~25%)
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::vertical(
                DockTree::Leaf {
                    tabs: vec!["scripts_on_entity".into(), "outline".into()],
                    active_tab: 0,
                },
                DockTree::leaf("assets"),
                0.4,
            ),
            0.4,
        ),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("code_editor"),
                DockTree::Leaf {
                    tabs: vec!["console".into(), "problems".into()],
                    active_tab: 0,
                },
                0.7,
            ),
            DockTree::vertical(
                DockTree::leaf("viewport"),
                DockTree::leaf("script_variables"),
                0.6,
            ),
            0.7,
        ),
        0.16,
    )
}

/// Animation: Hierarchy | (StudioPreview + StateMachine) | (Properties + Params) | Timeline
fn layout_animation() -> DockTree {
    DockTree::vertical(
        DockTree::horizontal(
            DockTree::leaf("hierarchy"),
            DockTree::horizontal(
                DockTree::vertical(
                    DockTree::leaf("studio_preview"),
                    DockTree::leaf("animator_state_machine"),
                    0.55,
                ),
                DockTree::vertical(
                    DockTree::leaf("animation"),
                    DockTree::leaf("animator_params"),
                    0.55,
                ),
                0.72,
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
/// UI: Hierarchy | Assets | WidgetLibrary (left)  |  UI Canvas (center)  |  UiInspector+Inspector (right)
fn layout_ui() -> DockTree {
    DockTree::horizontal(
        // Left: hierarchy on top, assets below. Width matches Scene layout.
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::leaf("assets"),
            0.6,
        ),
        DockTree::horizontal(
            // Center: UI canvas fills the full column.
            DockTree::leaf("ui_canvas"),
            // Right: UI inspector + widget palette tabbed together.
            DockTree::Leaf {
                tabs: vec!["ui_inspector".into(), "widget_library".into()],
                active_tab: 0,
            },
            0.82,
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

