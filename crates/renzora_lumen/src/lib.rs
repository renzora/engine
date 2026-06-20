//! Renzora Lumen — the GI distribution plugin.
//!
//! Ships as a `cdylib` dlopen plugin (in `plugins/`) like the postprocess
//! effects. `LumenPlugin` installs the Lumen voxel/trace passes AND its
//! screen-space backend `renzora_rt::RtPlugin` (Lumen's `ScreenSpace` tier) —
//! both must live in one dll so `RtLighting` has a single definition across the
//! main/render worlds. Under the `editor` feature it also registers the Lumen +
//! RT inspectors and the diagnostics snapshot the debugger's Lumen panel reads.
//!
//! The settings components (`LumenLighting`, `RtLighting`, …) live in the shared
//! `renzora` contract so the editor inspectors, `renzora_level_presets`, and the
//! debugger all share one `TypeId` across the dlopen boundary.
//!
//! Phase 1 implements only `Off` and `ScreenSpace`. Higher tiers
//! (`SdfLow`/`SdfHigh`/`Hwrt`) parse but currently render the same as `Off`;
//! Phases 2-6 of `docs/renzora_lumen_plan.md` fill them in.

use bevy::core_pipeline::{Core3d, Core3dSystems};
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponentPlugin;
use bevy::render::RenderApp;
use renzora::{
    LumenDebug, LumenLighting, LumenQuality, RtDebugMode, RtLighting, RtLightingExternallyManaged,
};

/// Bevy 0.19: the render graph became systems, so the Lumen GI pipeline's
/// node ordering (formerly render-graph edges across the sub-modules) is now
/// expressed as this shared `SystemSet` chain in the `Core3d` schedule. Each
/// pass system runs `.in_set(LumenSystems::…)`; `configure_lumen_sets` wires the
/// dependencies. All run in `EarlyPostProcess` (after the main pass, before
/// tonemapping) except `VoxelDebug`, which runs after tonemapping.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LumenSystems {
    VoxelClear,
    VoxelInject,
    GeometryInject,
    VoxelResolve,
    VoxelDownsample,
    ScreenReflectionTrace,
    ScreenReflectionBlur,
    ScreenReflectionResolve,
    LumenTrace,
    VoxelDebug,
}

/// Encode the GI pipeline ordering (the old render-graph edges) on the render
/// app's `Core3d` schedule. Called once from `LumenPlugin::build`.
fn configure_lumen_sets(render_app: &mut SubApp) {
    use Core3dSystems::{EarlyPostProcess, PostProcess};
    use LumenSystems::*;
    // EndMainPass → Clear → Inject → GeometryInject → Resolve (chained).
    render_app.configure_sets(
        Core3d,
        (VoxelClear, VoxelInject, GeometryInject, VoxelResolve)
            .chain()
            .in_set(EarlyPostProcess),
    );
    // Resolve → Downsample (mip pyramid).
    render_app.configure_sets(
        Core3d,
        VoxelDownsample.after(VoxelResolve).in_set(EarlyPostProcess),
    );
    // Resolve → SR-Trace → SR-Blur → SR-Resolve → LumenTrace (chained).
    // LumenTrace lives in EarlyPostProcess so it precedes Tonemapping (PostProcess).
    render_app.configure_sets(
        Core3d,
        (
            ScreenReflectionTrace,
            ScreenReflectionBlur,
            ScreenReflectionResolve,
            LumenTrace,
        )
            .chain()
            .after(VoxelResolve)
            .in_set(EarlyPostProcess),
    );
    // VoxelDebug runs after tonemapping.
    render_app.configure_sets(Core3d, VoxelDebug.in_set(PostProcess));
}

mod geometry_voxelize;
mod lumen_trace;
mod screen_reflection;
mod screen_reflection_blur;
mod screen_reflection_resolve;
mod voxel_cache;
mod voxel_downsample;
pub use geometry_voxelize::{GeometryVoxelizePlugin, LumenBakeStats, MeshVoxelSamples};
pub use lumen_trace::{LumenSkyCubemap, LumenTracePlugin};
pub use screen_reflection::ScreenReflectionPlugin;
pub use screen_reflection_blur::ScreenReflectionBlurPlugin;
pub use screen_reflection_resolve::ScreenReflectionResolvePlugin;
pub use voxel_cache::{VoxelCachePlugin, VoxelCacheView};
pub use voxel_downsample::VoxelDownsamplePlugin;

#[cfg(feature = "editor")]
mod editor;

#[derive(Default)]
pub struct LumenPlugin;

impl Plugin for LumenPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] LumenPlugin (GI: Lumen + RT backend)");

        // The screen-space backend (Lumen's ScreenSpace tier) ships inside this
        // GI plugin, so `RtLighting` is defined once on both sides of the dlopen
        // boundary. RtPlugin owns its own render-graph node (`RtLabel`),
        // independent of the Lumen labels below.
        app.add_plugins(renzora_rt::RtPlugin);

        app.register_type::<LumenLighting>();
        app.add_systems(Update, (sync_lumen_lighting, cleanup_lumen_lighting));
        app.add_plugins(ExtractComponentPlugin::<LumenLighting>::default());
        app.add_plugins(VoxelCachePlugin);
        // Mipmap pyramid generation for the voxel radiance texture.
        // Slots after `VoxelResolveLabel` (defined by VoxelCachePlugin)
        // so the resolved mip 0 is ready when we downsample mips 1..N.
        app.add_plugins(VoxelDownsamplePlugin);
        app.add_plugins(GeometryVoxelizePlugin);
        // LumenTracePlugin must register *before* ScreenReflectionPlugin
        // — ScreenReflectionPlugin's render-graph edge references
        // `LumenTraceLabel`, and Bevy resolves labels at edge-add
        // time (no lazy lookup). With this order, `LumenTraceLabel`
        // exists in the graph by the time ScreenReflection asks for
        // it. The reverse order panics with "node LumenTraceLabel
        // does not exist".
        app.add_plugins(LumenTracePlugin);
        app.add_plugins(ScreenReflectionPlugin);
        // Blur plugin slots its render-graph node between the trace
        // and `LumenTraceLabel`, so it must register after both
        // labels exist.
        app.add_plugins(ScreenReflectionBlurPlugin);
        // Resolve sits between blur and lumen_trace: blur fills the
        // pyramid, resolve bilateral-upsamples it to full res, trace
        // reads the resolved buffer.
        app.add_plugins(ScreenReflectionResolvePlugin);

        // Wire the GI pass ordering (replaces the old render-graph edges).
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            configure_lumen_sets(render_app);
        }

        // Editor-only: the inspectors (Lumen + RT) and the diagnostics snapshot
        // the debugger's Lumen panel reads. This plugin loads at Runtime scope
        // (so it runs in the editor viewport too); these registrations are
        // harmless no-ops in a shipped game with no editor present.
        #[cfg(feature = "editor")]
        {
            app.init_resource::<renzora::LumenDiagState>();
            app.add_systems(Update, editor::update_lumen_diag_state);
            editor::register_inspectors(app);
        }
    }
}

/// Route `LumenLighting` from source entities onto target cameras and
/// translate quality into the matching engine-level component.
fn sync_lumen_lighting(
    mut commands: Commands,
    sources: Query<(Entity, Ref<LumenLighting>)>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() {
                    found = true;
                    break;
                }
                apply_quality(&mut commands, *target, &settings);
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<(LumenLighting, RtLighting, RtLightingExternallyManaged)>();
            }
        }
    }
}

fn apply_quality(commands: &mut Commands, target: Entity, settings: &LumenLighting) {
    // Always mirror the component to the camera so the inspector reflects
    // what's active. The `RtLightingExternallyManaged` marker tells
    // `renzora_rt`'s sync system to leave RtLighting alone — without it,
    // RT would clobber what we set every frame because the authored source
    // entity has `LumenLighting`, not `RtLighting`.
    // try_insert: `target` may despawn before these deferred commands apply
    // (e.g. opening a document/asset tab tears down the scene camera).
    commands
        .entity(target)
        .try_insert((settings.clone(), RtLightingExternallyManaged));

    match settings.quality {
        LumenQuality::ScreenSpace => {
            let rt_debug = match settings.debug {
                LumenDebug::IndirectOnly => RtDebugMode::IndirectOnly,
                // VoxelCache is a Lumen-only debug view; SSGI keeps
                // composite output and the voxel debug pass overlays
                // on top.
                LumenDebug::None | LumenDebug::VoxelCache => RtDebugMode::Composite,
            };
            commands.entity(target).try_insert(RtLighting {
                enabled: true,
                intensity: settings.intensity,
                debug: rt_debug,
            });
        }
        LumenQuality::Off | LumenQuality::SdfLow | LumenQuality::SdfHigh | LumenQuality::Hwrt => {
            // SdfLow / SdfHigh are handled by the Lumen voxel-cache trace
            // pipeline (`LumenTracePlugin`); it reads quality off the
            // mirrored `LumenLighting` directly. RtLighting (SSGI) must be
            // stripped here so the two GI paths don't double-apply.
            commands.entity(target).remove::<RtLighting>();
        }
    }
}

fn cleanup_lumen_lighting(
    mut commands: Commands,
    mut removed: RemovedComponents<LumenLighting>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<(LumenLighting, RtLighting, RtLightingExternallyManaged)>();
            }
        }
    }
}

renzora::add!(LumenPlugin);
