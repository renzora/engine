//! Editor-only half of `renzora_navmesh` (the dual-mode crate split).
//!
//! `renzora_navmesh` compiles lean (no `editor` cargo feature, no renzora_ui /
//! renzora_ember). The editor-only pieces that used to live behind
//! `#[cfg(feature = "editor")]` in the main crate moved here: the NavMesh
//! inspector entries, the spawn preset, the native (ember) panel UI, and the
//! bake-to-disk workflow.
//!
//! [`NavMeshEditorPlugin`] registers via `renzora::add!(.., Editor)`. The editor
//! bundle links this crate as an rlib and replays its Editor-scope registration
//! at dlopen; the lean runtime never links it. The runtime navmesh systems live
//! in `renzora_navmesh::NavMeshPlugin` (Runtime scope), which runs in both the
//! editor viewport and the shipped game.

use bevy::prelude::*;

mod editor_panel;
mod inspectors;
mod native;
pub mod persistence;

use renzora::AppEditorExt;
use renzora_navmesh::ShowAgentPathsOverride;

/// Editor-scope companion to `renzora_navmesh::NavMeshPlugin`. Adds the NavMesh
/// inspectors, the spawn preset, the native panel, and the bake workflow.
#[derive(Default)]
pub struct NavMeshEditorPlugin;

impl Plugin for NavMeshEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] NavMeshEditorPlugin");

        app.register_inspector(inspectors::inspector_entry());
        app.register_inspector(inspectors::obstacle_inspector_entry());
        app.register_inspector(inspectors::agent_inspector_entry());

        {
            use renzora::{EntityPreset, SpawnRegistry};
            let mut registry = app
                .world_mut()
                .get_resource_or_insert_with(SpawnRegistry::default);
            registry.register(EntityPreset {
                id: "navmesh_volume",
                display_name: "NavMesh Volume",
                icon: "polygon",
                category: "Navigation",
                spawn_fn: |world: &mut World| {
                    world
                        .spawn((
                            Name::new("NavMesh Volume"),
                            renzora_navmesh::NavMeshVolume::default(),
                            Transform::default(),
                        ))
                        .id()
                },
            });
        }

        app.init_resource::<editor_panel::NavMeshPanelState>();
        app.init_resource::<editor_panel::NavMeshPanelMirror>();
        app.init_resource::<editor_panel::NavMeshBakeRequest>();
        // The lean `draw_agent_paths` reads `ShowAgentPathsOverride` when present
        // (absent in a game → it falls back to per-volume debug_draw). The panel
        // owns the "Show Agent Paths" toggle (default on), so seed the override
        // and keep it in sync each frame.
        app.insert_resource(ShowAgentPathsOverride(true));

        // Native (ember) panel content.
        app.add_plugins(native::NativeNavmesh);
        app.add_systems(
            Update,
            (
                editor_panel::refresh_panel_mirror,
                editor_panel::drain_panel_actions,
                editor_panel::apply_auto_rebuild_setting,
                sync_show_agent_paths_override,
            ),
        );
        app.add_systems(Update, editor_panel::flush_bake_request);
    }
}

renzora::add!(NavMeshEditorPlugin, Editor);

/// Mirror the panel's "Show Agent Paths" toggle into the lean override resource
/// that `renzora_navmesh::draw_agent_paths` reads.
fn sync_show_agent_paths_override(
    panel: Res<editor_panel::NavMeshPanelState>,
    mut over: ResMut<ShowAgentPathsOverride>,
) {
    over.0 = panel.show_agent_paths();
}
