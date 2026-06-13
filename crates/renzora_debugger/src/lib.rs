//! Debugging panels and profiling support for the Renzora editor.
//!
//! Panels: System Profiler, Memory Profiler, Performance, Render Stats, ECS
//! Stats, Camera Debug, Culling Debug, Material Resolver, Lumen, Scripting.
//! All panels are bevy-native (ember); their content lives in [`native`] and
//! reads the per-frame snapshot resources kept current by the backend-agnostic
//! `update_*` systems in [`state`] (plus the scripting diag updater below).
//! The Lumen panel reads `renzora::LumenDiagState`, produced by the GI plugin.

pub mod native;
pub mod panels;
pub mod state;

use bevy::diagnostic::{
    EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
};
use bevy::prelude::*;

use state::*;

// ============================================================================
// Diagnostic snapshot updaters (scripting)
// ============================================================================
//
// The Lumen diagnostics snapshot (`renzora::LumenDiagState`) is produced by the
// GI plugin (`renzora_lumen`) under its `editor` feature, not here — the plugin
// is a cdylib and owns the internal voxel/bake types it reads. The native Lumen
// panel just reads the contract resource.

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

        // Real per-render-pass CPU/GPU timings (`render/<pass>/elapsed_{cpu,gpu}`).
        // This is the ONLY source of genuine GPU time; without it the render-stats
        // panel has nothing to read. On Vulkan/DX12 Bevy's default
        // `WgpuSettingsPriority::Functionality` already enables `TIMESTAMP_QUERY`,
        // so GPU timestamps populate automatically; on backends without it (GL,
        // some integrated adapters) only CPU spans exist and the panel shows "n/a"
        // for GPU rather than a fabricated number. Guarded because the (currently
        // unused) Tracy bridge can also add it, and a duplicate add panics.
        use bevy::render::diagnostic::RenderDiagnosticsPlugin;
        if !app.is_plugin_added::<RenderDiagnosticsPlugin>() {
            app.add_plugins(RenderDiagnosticsPlugin);
        }

        // Attribute the engine's built-in GPU passes to the components that drive
        // them, so the GPU Pass Breakdown shows *what* is paying for each pass.
        // Plugins that add their own render passes register the same way (via
        // `App::register_gpu_pass_source`) — nothing here is special-cased in the
        // panel. NOTE: the atmosphere environment map becomes a
        // `GeneratedEnvironmentMapLight` on the camera, so counting that catches
        // the realtime atmosphere IBL that drives the `lightprobe_*` passes.
        use bevy::light::{DirectionalLight, GeneratedEnvironmentMapLight, PointLight, SpotLight};
        use renzora::AppEditorExt;
        app.register_gpu_pass_source::<GeneratedEnvironmentMapLight>("lightprobe", "environment map")
            .register_gpu_pass_source::<DirectionalLight>(
                "shadow_directional_light",
                "directional light",
            )
            .register_gpu_pass_source::<PointLight>("shadow_point", "point light")
            .register_gpu_pass_source::<SpotLight>("shadow_spot", "spot light");

        // Init resources
        app.init_resource::<DiagnosticsState>()
            .init_resource::<RenderStats>()
            .init_resource::<SystemTimingState>()
            .init_resource::<MemoryProfilerState>()
            .init_resource::<CameraDebugState>()
            .init_resource::<CullingDebugState>()
            .init_resource::<EcsStatsState>()
            .init_resource::<panels::scripting::ScriptingDiagState>();

        // Update systems
        use renzora::SplashState;
        app.add_systems(
            Update,
            (
                update_diagnostics_state,
                update_render_stats,
                update_memory_profiler,
                update_camera_debug_state,
                update_culling_debug_state,
            )
                .run_if(in_state(SplashState::Editor)),
        );
        // Exclusive systems (need `&mut World`): ECS archetype stats, and the GPU
        // pass breakdown (scans archetypes to count the entities driving passes).
        app.add_systems(
            Update,
            update_ecs_stats.run_if(in_state(SplashState::Editor)),
        );
        app.add_systems(
            Update,
            update_system_timing.run_if(in_state(SplashState::Editor)),
        );
        app.add_systems(
            Update,
            update_scripting_diag_state.run_if(in_state(SplashState::Editor)),
        );

        // bevy-native (ember) content for every debug panel.
        native::register_native_debug(app);
    }
}

renzora::add!(DebuggerPlugin, Editor);
