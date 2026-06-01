//! Editor workspace layouts (which panels go where, per ribbon workspace).
//!
//! The dock **model** (`DockTree`, mutations, `DropZone`) now lives in
//! [`renzora_ember::dock`] — it's the reusable, UI-framework half. This module
//! is the editor-specific part: it builds concrete `DockTree`s for the editor's
//! workspaces using that model. Re-exported here so the rest of the shell keeps
//! importing `dock::DockTree` etc. unchanged.

pub use renzora_ember::dock::{DockTree, DropZone, SplitDirection};

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
        ("Video".into(), layout_video()),
        ("Audio".into(), layout_audio()),
        ("Debug".into(), layout_debug()),
        ("Gallery".into(), layout_gallery()),
    ]
}

/// Gallery: a 2×3 showcase of the `renzora_ember` component categories, each in
/// its own panel — the living catalog of the bevy_ui widget set.
fn layout_gallery() -> DockTree {
    let top = DockTree::horizontal(
        DockTree::leaf("gallery_typography"),
        DockTree::horizontal(
            DockTree::leaf("gallery_buttons"),
            DockTree::leaf("gallery_inputs"),
            0.5,
        ),
        0.34,
    );
    let bottom = DockTree::horizontal(
        DockTree::leaf("gallery_selection"),
        DockTree::horizontal(
            DockTree::leaf("gallery_feedback"),
            DockTree::leaf("gallery_colors"),
            0.5,
        ),
        0.34,
    );
    DockTree::vertical(top, bottom, 0.5)
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

/// Video: Hierarchy | Viewport | Inspector  /  Sequencer+Mixer+Assets
fn layout_video() -> DockTree {
    DockTree::vertical(
        DockTree::horizontal(
            DockTree::leaf("hierarchy"),
            DockTree::horizontal(DockTree::leaf("viewport"), DockTree::leaf("inspector"), 0.78),
            0.15,
        ),
        DockTree::tabs(&["sequencer", "mixer", "assets"]),
        0.55,
    )
}

/// Audio: Hierarchy | DAW | Inspector  /  Mixer | Assets+Console
fn layout_audio() -> DockTree {
    DockTree::vertical(
        DockTree::horizontal(
            DockTree::leaf("hierarchy"),
            DockTree::horizontal(DockTree::leaf("daw"), DockTree::leaf("inspector"), 0.78),
            0.15,
        ),
        DockTree::horizontal(
            DockTree::leaf("mixer"),
            DockTree::tabs(&["assets", "console"]),
            0.6,
        ),
        0.6,
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
                "record",
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
