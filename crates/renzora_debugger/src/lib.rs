//! Debugging panels and profiling support for the Renzora editor.
//!
//! Panels: System Profiler, Memory Profiler, Performance, Render Stats, ECS
//! Stats, Camera Debug, Culling Debug, Material Resolver, Lumen, Scripting.
//! All panels are bevy-native (ember); their content lives in [`native`] and
//! reads the per-frame snapshot resources kept current by the backend-agnostic
//! `update_*` systems in [`state`] (plus the lumen/scripting diag updaters
//! below). For Tracy profiler integration see the `renzora_tracy` crate.

pub mod native;
pub mod panels;
pub mod state;

use bevy::diagnostic::{
    EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
};
use bevy::prelude::*;

use state::*;

// ============================================================================
// Diagnostic snapshot updaters (lumen + scripting)
// ============================================================================

fn update_lumen_diag_state(
    mut state: ResMut<panels::lumen::LumenDiagState>,
    bake_stats: Option<Res<renzora_lumen::LumenBakeStats>>,
    cameras: Query<(Option<&Name>, &renzora_lumen::VoxelCacheView)>,
    sample_entities: Query<(), With<renzora_lumen::MeshVoxelSamples>>,
    // LumenSkyCubemap is a Component on the editor camera (extracted to
    // the render world each frame). Any entity with it = at least one
    // camera has an IBL/sky source bound.
    sky_cubemaps: Query<(), With<renzora_lumen::LumenSkyCubemap>>,
) {
    state.cameras.clear();
    state.cameras.extend(cameras.iter().map(|(name, view)| {
        panels::lumen::LumenCameraEntry {
            camera_name: name
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|| "<unnamed>".into()),
            inject_active: view.inject_active,
            debug_active: view.debug_active,
        }
    }));
    state.mesh_voxel_samples_entities = sample_entities.iter().count();
    state.has_sky_cubemap = !sky_cubemaps.is_empty();
    state.bake = bake_stats.map(|s| (*s).clone()).unwrap_or_default();
}

fn update_scripting_diag_state(
    mut state: ResMut<panels::scripting::ScriptingDiagState>,
    engine: Option<Res<renzora_scripting::ScriptEngine>>,
    perf: Option<Res<renzora_scripting::perf::ScriptPerfStats>>,
    components: Query<&renzora_scripting::ScriptComponent>,
) {
    // Entity-level inventory (cheap, no allocations beyond the count).
    let mut entities = 0usize;
    let mut attachments = 0usize;
    for comp in components.iter() {
        entities += 1;
        attachments += comp.scripts.len();
    }
    state.entities_with_script = entities;
    state.total_script_attachments = attachments;

    if let Some(engine) = engine {
        state.backend_count = engine.backend_count();
        state.scripts_folder = engine
            .scripts_folder()
            .map(|p| p.to_string_lossy().to_string());
    }

    if let Some(perf) = perf {
        state.totals = perf.totals();
        state.per_script = perf.snapshot();
        state.current_frame = perf.frame;
    } else {
        state.totals = Default::default();
        state.per_script.clear();
        state.current_frame = 0;
    }
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct DebuggerPlugin;

impl Plugin for DebuggerPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] DebuggerPlugin");
        // Add Bevy diagnostic plugins
        app.add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
            SystemInformationDiagnosticsPlugin,
        ));

        // Init resources
        app.init_resource::<DiagnosticsState>()
            .init_resource::<RenderStats>()
            .init_resource::<SystemTimingState>()
            .init_resource::<MemoryProfilerState>()
            .init_resource::<CameraDebugState>()
            .init_resource::<CullingDebugState>()
            .init_resource::<EcsStatsState>()
            .init_resource::<panels::lumen::LumenDiagState>()
            .init_resource::<panels::scripting::ScriptingDiagState>();

        // Update systems
        use renzora::SplashState;
        app.add_systems(
            Update,
            (
                update_diagnostics_state,
                update_render_stats,
                update_memory_profiler,
                update_system_timing,
                update_camera_debug_state,
                update_culling_debug_state,
            )
                .run_if(in_state(SplashState::Editor)),
        );
        app.add_systems(
            Update,
            update_ecs_stats.run_if(in_state(SplashState::Editor)),
        );
        app.add_systems(
            Update,
            (update_lumen_diag_state, update_scripting_diag_state)
                .run_if(in_state(SplashState::Editor)),
        );

        // bevy-native (ember) content for every debug panel.
        native::register_native_debug(app);
    }
}

renzora::add!(DebuggerPlugin, Editor);
