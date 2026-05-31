//! Workspace layout presets and layout manager.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::dock_tree::{DockTree, DockingState};

/// A named workspace layout.
#[derive(Clone, Serialize, Deserialize)]
pub struct WorkspaceLayout {
    pub name: String,
    pub tree: DockTree,
    /// Hidden layouts don't show up in the title-bar layout switcher. Used
    /// for asset-mode variants (e.g. `Materials-Asset`) which the editor
    /// switches to automatically when the user opens a single asset file.
    #[serde(default)]
    pub hidden: bool,
}

impl WorkspaceLayout {
    fn scene(name: &str, tree: DockTree) -> Self {
        Self {
            name: name.into(),
            tree,
            hidden: false,
        }
    }

    fn asset(name: &str, tree: DockTree) -> Self {
        Self {
            name: name.into(),
            tree,
            hidden: true,
        }
    }
}

/// Resource managing available workspace layouts.
#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct LayoutManager {
    pub layouts: Vec<WorkspaceLayout>,
    pub active_index: usize,
    /// Index of the last *non-hidden* (scene-mode) layout the user explicitly
    /// chose from the title bar. When the editor leaves Asset mode (user
    /// closes the asset tab or clicks back to a scene tab), this is the
    /// layout we restore.
    #[serde(default)]
    pub last_scene_index: usize,
}

impl Default for LayoutManager {
    fn default() -> Self {
        let layouts = vec![
            // ── Scene-mode layouts (visible in title bar) ────────────────
            WorkspaceLayout::scene("Scene", scene_layout()),
            WorkspaceLayout::scene("Blueprints", layout_blueprints()),
            WorkspaceLayout::scene("Scripting", layout_scripting()),
            WorkspaceLayout::scene("Animation", layout_animation()),
            WorkspaceLayout::scene("Materials", layout_materials()),
            WorkspaceLayout::scene("Particles", layout_particles()),
            WorkspaceLayout::scene("Video", layout_video()),
            WorkspaceLayout::scene("Audio", layout_audio()),
            WorkspaceLayout::scene("Debug", layout_debug()),
            // ── Asset-mode layouts (hidden, auto-activated when an asset
            // doc tab is focused). Add new variants here as panels for
            // those kinds learn to render from `EditorContext`.
            WorkspaceLayout::asset("Materials-Asset", layout_materials_asset()),
            WorkspaceLayout::asset("Scripting-Asset", layout_scripting_asset()),
            WorkspaceLayout::asset("Blueprints-Asset", layout_blueprints_asset()),
            WorkspaceLayout::asset("Particles-Asset", layout_particles_asset()),
        ];

        Self {
            layouts,
            active_index: 0,
            last_scene_index: 0,
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

    /// Ensure every layout in the factory default exists in this manager,
    /// preserving any user-customised trees. Called after loading from disk
    /// so older saved workspaces pick up newly-added layouts (e.g. the
    /// asset-mode variants) without a manual reset.
    pub fn merge_missing_defaults(&mut self) {
        let defaults = Self::default();
        for default_layout in &defaults.layouts {
            let exists = self.layouts.iter().any(|l| l.name == default_layout.name);
            if !exists {
                self.layouts.push(default_layout.clone());
            } else if let Some(existing) = self
                .layouts
                .iter_mut()
                .find(|l| l.name == default_layout.name)
            {
                // Always re-stamp the hidden flag from the factory definition
                // so a workspace saved before `hidden` existed still hides
                // the asset-mode variants from the title bar.
                existing.hidden = default_layout.hidden;
            }
        }
    }

    /// Iterate visible (title-bar-eligible) layouts with their original index.
    pub fn visible_layouts(&self) -> impl Iterator<Item = (usize, &WorkspaceLayout)> {
        self.layouts.iter().enumerate().filter(|(_, l)| !l.hidden)
    }

    /// Add a new user-created visible layout and switch to it. The new
    /// layout starts with an empty dock tree so the user can choose which
    /// panels to populate it with from a clean slate. Returns the new
    /// layout's index.
    pub fn add_layout(&mut self, name: String, docking: &mut DockingState) -> usize {
        // Snapshot current dock so the active slot stays in sync (mirrors `switch`).
        if let Some(current) = self.layouts.get_mut(self.active_index) {
            current.tree = docking.tree.clone();
        }
        let layout = WorkspaceLayout {
            name,
            tree: DockTree::Empty,
            hidden: false,
        };
        self.layouts.push(layout);
        let new_idx = self.layouts.len() - 1;
        self.active_index = new_idx;
        self.last_scene_index = new_idx;
        docking.tree = DockTree::Empty;
        new_idx
    }

    /// Rename the layout at `index`. Returns `true` if the rename happened.
    /// No-op when the index is out of range or when another layout already
    /// uses the requested name (case-insensitive). Trims surrounding
    /// whitespace before storing.
    pub fn rename_layout(&mut self, index: usize, new_name: String) -> bool {
        let trimmed = new_name.trim().to_string();
        if trimmed.is_empty() || index >= self.layouts.len() {
            return false;
        }
        let conflict = self
            .layouts
            .iter()
            .enumerate()
            .any(|(i, l)| i != index && l.name.eq_ignore_ascii_case(&trimmed));
        if conflict {
            return false;
        }
        if let Some(slot) = self.layouts.get_mut(index) {
            slot.name = trimmed;
            return true;
        }
        false
    }

    /// Delete the layout at `index`. Refuses to delete the last visible
    /// layout (so the title bar always has at least one tab to switch to).
    /// If the deleted layout was active, switches to the previous visible
    /// neighbour. Returns `true` if the layout was deleted.
    pub fn delete_layout(&mut self, index: usize, docking: &mut DockingState) -> bool {
        if index >= self.layouts.len() {
            return false;
        }
        if self.layouts[index].hidden {
            return false;
        }
        let visible_count = self.layouts.iter().filter(|l| !l.hidden).count();
        if visible_count <= 1 {
            return false;
        }
        // Snapshot current dock so the active slot stays in sync before we
        // potentially delete a different slot.
        if let Some(current) = self.layouts.get_mut(self.active_index) {
            current.tree = docking.tree.clone();
        }

        self.layouts.remove(index);

        // Re-target indices that pointed at or past the removed slot.
        let remap = |idx: usize| -> usize {
            if idx > index {
                idx - 1
            } else {
                idx
            }
        };
        if self.active_index == index {
            // Pick the nearest remaining visible layout.
            let new_idx = self
                .layouts
                .iter()
                .enumerate()
                .filter(|(_, l)| !l.hidden)
                .map(|(i, _)| i)
                .min_by_key(|i| (*i as isize - index as isize).abs())
                .unwrap_or(0);
            self.active_index = new_idx;
            if let Some(layout) = self.layouts.get(new_idx) {
                docking.tree = layout.tree.clone();
            }
        } else {
            self.active_index = remap(self.active_index);
        }
        if self.last_scene_index == index {
            self.last_scene_index = self.active_index;
        } else {
            self.last_scene_index = remap(self.last_scene_index);
        }
        true
    }

    /// Move the layout at `from` to position `to`, shifting layouts in
    /// between. Updates `active_index` and `last_scene_index` so the user's
    /// active layout follows the move. No-op when indices are equal or
    /// out of range.
    pub fn move_layout(&mut self, from: usize, to: usize) {
        if from == to || from >= self.layouts.len() || to >= self.layouts.len() {
            return;
        }
        let item = self.layouts.remove(from);
        self.layouts.insert(to, item);

        let remap = |idx: usize| -> usize {
            if idx == from {
                to
            } else if from < to && idx > from && idx <= to {
                idx - 1
            } else if to < from && idx >= to && idx < from {
                idx + 1
            } else {
                idx
            }
        };
        self.active_index = remap(self.active_index);
        self.last_scene_index = remap(self.last_scene_index);
    }

    /// Reset the active layout's tree to its hardcoded factory default.
    /// Other layouts are untouched.
    pub fn reset_active(&mut self, docking: &mut DockingState) {
        let defaults = Self::default();
        let Some(active) = self.layouts.get(self.active_index) else {
            return;
        };
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

/// Scene: Viewport+BottomTabs | (Hierarchy/Scenes/Shapes) over (Inspector/Gamepad/History)
///
/// No left column — main area gets the full width minus the right
/// column. Right column stacks hierarchy/scenes/shape_library tabs on
/// top of inspector/gamepad/history tabs.
pub fn scene_layout() -> DockTree {
    DockTree::horizontal(
        // Main area: viewport on top, assets/console/etc tabbed below
        DockTree::vertical(
            DockTree::Leaf {
                tabs: vec![
                    "viewport".into(),
                    "render_pipeline".into(),
                    "code_editor".into(),
                ],
                active_tab: 0,
            },
            DockTree::Leaf {
                tabs: vec![
                    "assets".into(),
                    "hub_store".into(),
                    "console".into(),
                    "mixer".into(),
                    "sequencer".into(),
                    "timeline".into(),
                    "record".into(),
                ],
                active_tab: 0,
            },
            0.72,
        ),
        // Right column: hierarchy/scenes/shapes tabs on top, inspector/gamepad/history below
        DockTree::vertical(
            DockTree::Leaf {
                tabs: vec![
                    "hierarchy".into(),
                    "scenes".into(),
                    "shape_library".into(),
                ],
                active_tab: 0,
            },
            DockTree::Leaf {
                tabs: vec!["inspector".into(), "gamepad".into(), "history".into()],
                active_tab: 0,
            },
            0.4,
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
    // Left column:   Hierarchy / Scripts / Assets               (~16%)
    // Center column: Code editor / (Console+Problems tabbed)    (~59%)
    // Right column:  Viewport / Outline / Script Variables      (~25%)
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::vertical(
                DockTree::leaf("scripts_on_entity"),
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
                DockTree::vertical(
                    DockTree::leaf("outline"),
                    DockTree::leaf("script_variables"),
                    0.4,
                ),
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

/// Debug: Hierarchy+Performance | Viewport+debug panels | Inspector+ECS / SceneDiagnostics+(subsystem diag tabs)
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
                    tabs: vec!["inspector".into(), "gamepad".into(), "ecs_stats".into()],
                    active_tab: 0,
                },
                // The MOT — Scene Diagnostics on top, with the subsystem
                // diagnostic panels stacked as tabs so they all share
                // one slot the user can flip through.
                DockTree::Leaf {
                    tabs: vec![
                        "scene_diagnostics".into(),
                        "material_resolver_diag".into(),
                        "lumen_diag".into(),
                        "scripting_diag".into(),
                    ],
                    active_tab: 0,
                },
                0.5,
            ),
            0.75,
        ),
        0.15,
    )
}

/// Materials: Preview + Properties | MaterialGraph
fn layout_materials() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("material_preview"),
            DockTree::leaf("material_inspector"),
            0.5,
        ),
        DockTree::leaf("material_graph"),
        0.25,
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

// ── Asset-mode layouts ──────────────────────────────────────────────────────
//
// These activate when the user opens a single asset file (double-click in the
// asset browser → opens a doc tab → editor enters Asset mode). They drop the
// hierarchy/outline panels because there's no entity context — the panels in
// these layouts read the file path from `EditorContext` directly.

/// Materials (asset mode): Preview + Properties | MaterialGraph
/// Same shape as the scene-mode layout but explicitly without hierarchy —
/// makes it obvious the user is editing a file, not an entity's material.
fn layout_materials_asset() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("material_preview"),
            DockTree::leaf("material_inspector"),
            0.5,
        ),
        DockTree::leaf("material_graph"),
        0.25,
    )
}

/// Scripting (asset mode): CodeEditor + Console+Problems
/// No hierarchy, no scripts_on_entity, no viewport — you're editing one file.
fn layout_scripting_asset() -> DockTree {
    DockTree::vertical(
        DockTree::leaf("code_editor"),
        DockTree::Leaf {
            tabs: vec!["console".into(), "problems".into()],
            active_tab: 0,
        },
        0.75,
    )
}

/// Blueprints (asset mode): BlueprintGraph | NodeProperties
/// No hierarchy — the graph being edited comes from a `.blueprint` file,
/// not from a scene entity.
fn layout_blueprints_asset() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("blueprint_graph"),
        DockTree::leaf("blueprint_properties"),
        0.78,
    )
}

/// Particles (asset mode): ParticlePreview | ParticleEditor
/// Same shape as scene-mode particles layout — particle editor is already
/// file-driven, so no hierarchy is needed even in scene mode.
fn layout_particles_asset() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("particle_preview"),
        DockTree::leaf("particle_editor"),
        0.7,
    )
}

/// Video: Premiere-style cinematics workspace.
///
/// Top row: Hierarchy | Viewport (preview) | Inspector
/// Bottom row: Sequencer with Mixer + Assets tabbed alongside it.
fn layout_video() -> DockTree {
    DockTree::vertical(
        // Top: hierarchy on the left, viewport (preview) center, inspector right.
        DockTree::horizontal(
            DockTree::leaf("hierarchy"),
            DockTree::horizontal(
                DockTree::leaf("viewport"),
                DockTree::leaf("inspector"),
                0.78,
            ),
            0.15,
        ),
        // Bottom: sequencer is the main work surface; mixer + assets tab in
        // beside it so audio levels and clip sources are one click away.
        DockTree::Leaf {
            tabs: vec!["sequencer".into(), "mixer".into(), "assets".into()],
            active_tab: 0,
        },
        0.55,
    )
}

/// Audio: DAW timeline + Mixer + Assets
fn layout_audio() -> DockTree {
    DockTree::vertical(
        // Top: hierarchy | DAW timeline | inspector
        DockTree::horizontal(
            DockTree::leaf("hierarchy"),
            DockTree::horizontal(DockTree::leaf("daw"), DockTree::leaf("inspector"), 0.78),
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
