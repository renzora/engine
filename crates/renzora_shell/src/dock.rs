//! Editor workspace layouts (which panels go where, per ribbon workspace).
//!
//! The dock **model** (`DockTree`, mutations, `DropZone`) now lives in
//! [`renzora_ember::dock`] — it's the reusable, UI-framework half. This module
//! is the editor-specific part: it builds concrete `DockTree`s for the editor's
//! workspaces using that model. Re-exported here so the rest of the shell keeps
//! importing `dock::DockTree` etc. unchanged.

pub use renzora_ember::dock::{DockTree, DropZone, SplitDirection};

use serde::{Deserialize, Serialize};

// ── Persistence ────────────────────────────────────────────────────────────────
//
// Dock positions (split ratios, which panels sit where, active tabs) persist
// across sessions in a per-user file, mirroring the `~/.renzora/*.toml`
// convention used for the renderer/UI-scale preferences (see `renzora::core`).
// JSON, not TOML: the layout is a recursive tagged enum tree, which TOML renders
// as an unreadable pile of nested tables — JSON round-trips it cleanly. The set
// of workspaces is machine-local user state, not project state, so it lives next
// to the other per-user prefs rather than in `project.toml`.

/// One persisted workspace: its ribbon name + its dock tree.
#[derive(Serialize, Deserialize)]
struct PersistedWorkspace {
    name: String,
    tree: DockTree,
}

/// The on-disk dock layout file: every workspace plus the active index.
#[derive(Serialize, Deserialize)]
struct PersistedLayout {
    active: usize,
    workspaces: Vec<PersistedWorkspace>,
}

/// Path to the persisted dock layout: `~/.renzora/layout.json`. Resolves the
/// home dir via env vars (matching `renzora::core`'s pref paths) so this stays
/// dependency-light. `None` on wasm / when no home dir is resolvable.
#[cfg(not(target_arch = "wasm32"))]
fn layout_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)?;
    Some(home.join(".renzora").join("layout.json"))
}

/// Load the persisted workspaces + active index, or `None` when the file is
/// absent / unreadable / malformed (callers then fall back to the built-in
/// [`workspace_layouts`]).
pub fn load_dock_layouts() -> Option<(Vec<(String, DockTree)>, usize)> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let text = std::fs::read_to_string(layout_path()?).ok()?;
        let data: PersistedLayout = serde_json::from_str(&text).ok()?;
        if data.workspaces.is_empty() {
            return None;
        }
        let workspaces = data
            .workspaces
            .into_iter()
            .map(|w| (w.name, w.tree))
            .collect::<Vec<_>>();
        let active = data.active.min(workspaces.len() - 1);
        Some((workspaces, active))
    }
}

/// Serialize the workspaces + active index to the JSON we'd persist. Returns the
/// string so the caller can skip a redundant disk write when nothing changed
/// (the save system compares it against the last-written snapshot).
pub fn layout_json(workspaces: &[(String, DockTree)], active: usize) -> Option<String> {
    let data = PersistedLayout {
        active,
        workspaces: workspaces
            .iter()
            .map(|(name, tree)| PersistedWorkspace {
                name: name.clone(),
                tree: tree.clone(),
            })
            .collect(),
    };
    serde_json::to_string_pretty(&data).ok()
}

/// Write a pre-serialized layout JSON (from [`layout_json`]) to disk, creating
/// `~/.renzora/` if needed. No-op `Ok` on wasm.
#[allow(unused_variables)]
pub fn write_layout(json: &str) -> std::io::Result<()> {
    #[cfg(target_arch = "wasm32")]
    {
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = layout_path().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "could not resolve home directory for dock layout",
            )
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, json)
    }
}

/// The ribbon workspace layouts, in ribbon order (Scene … Debug). Ports
/// `renzora_ui::layouts` (the visible, non-asset layouts) into the shell's
/// egui-free dock model.
pub fn workspace_layouts() -> Vec<(String, DockTree)> {
    vec![
        ("Scene".into(), scene_layout()),
        ("Blueprints".into(), layout_blueprints()),
        ("Scripting".into(), layout_scripting()),
        ("Animation".into(), layout_animation()),
        ("Materials".into(), layout_materials()),
        ("Particles".into(), layout_particles()),
        ("Debug".into(), layout_debug()),
    ]
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

/// Scripting: Hierarchy/Scripts/Assets | CodeEditor+Console | Viewport/Outline/Vars
fn layout_scripting() -> DockTree {
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
                DockTree::tabs(&["console", "problems"]),
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

/// Animation: Hierarchy | (StudioPreview/StateMachine) | (Properties/Params) | Timeline
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
fn layout_particles() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("particle_preview"),
        DockTree::leaf("particle_editor"),
        0.8,
    )
}

/// Debug: Hierarchy/Performance | Viewport+diag panels | Inspector/ECS + diagnostics
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
                        DockTree::tabs(&["render_stats", "render_pipeline"]),
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
                DockTree::tabs(&["inspector", "gamepad", "ecs_stats"]),
                DockTree::tabs(&[
                    "scene_diagnostics",
                    "material_resolver_diag",
                    "lumen_diag",
                    "scripting_diag",
                ]),
                0.5,
            ),
            0.75,
        ),
        0.15,
    )
}

/// Scene workspace: main area (viewport + bottom tabs) on the left, a right
/// column stacking hierarchy/scenes/shapes over inspector/gamepad/history.
///
/// Mirrors `renzora_ui::layouts::scene_layout` so the bevy_ui shell renders the
/// same default the egui editor ships.
pub fn scene_layout() -> DockTree {
    DockTree::horizontal(
        // Main area: viewport on top, assets/console/etc tabbed below.
        DockTree::vertical(
            DockTree::tabs(&["viewport", "render_pipeline", "code_editor"]),
            DockTree::tabs(&[
                "assets",
                "hub_store",
                "console",
                "mixer",
                "sequencer",
                "timeline",
            ]),
            0.72,
        ),
        // Right column: hierarchy/scenes/shapes on top, inspector/gamepad/history below.
        DockTree::vertical(
            DockTree::tabs(&["hierarchy", "scenes", "shape_library"]),
            DockTree::tabs(&["inspector", "gamepad", "history"]),
            0.4,
        ),
        0.82,
    )
}
