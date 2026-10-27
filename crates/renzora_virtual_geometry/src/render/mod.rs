//! The render-world half: extraction, the GPU scene, per-view resources and
//! the passes. See `passes.rs` for how a frame runs.

pub(crate) mod debug;
pub(crate) mod masked;
pub(crate) mod materials;
pub(crate) mod passes;
pub(crate) mod pipelines;
pub(crate) mod pool;
pub(crate) mod scene;
pub(crate) mod streaming;
pub(crate) mod transparent;
pub(crate) mod view;

use bevy::core_pipeline::core_3d::main_opaque_pass_3d;
use bevy::core_pipeline::prepass::node::early_prepass;
use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy::prelude::*;
use bevy::render::{ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems};

/// Install the renderer. False when the app has no render world.
pub(crate) fn build(app: &mut App) -> bool {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return false;
    };
    render_app
        .init_resource::<scene::ExtractedScene>()
        .init_resource::<scene::GpuScene>()
        .init_resource::<view::VgViewBuffers>()
        .init_resource::<debug::VgDebug>()
        .init_resource::<masked::MaskedDraws>()
        .init_resource::<scene::VgSkinBindGroup>()
        .init_resource::<transparent::TransparentPipelines>()
        .add_systems(RenderStartup, (pipelines::init, debug::init, transparent::init))
        .add_systems(
            ExtractSchedule,
            (scene::extract, view::extract_settings, debug::extract),
        )
        .add_systems(
            Render,
            (
                // Before the queue systems, not with Bevy's `PrepareResources`
                // (which runs after them): they read this frame's buckets and
                // materials, and must see the same ones the buffers and bind
                // groups are built from. Built after them, a model removed this
                // frame left a draw pointing past the end of the new buffers.
                // Only prepared assets are read here, and those are ready.
                (scene::prepare, view::prepare, debug::prepare)
                    .chain()
                    .in_set(RenderSystems::PrepareViews),
                (view::prepare_bind_groups, scene::prepare_skin_bind_group)
                    .in_set(RenderSystems::PrepareBindGroups),
                (materials::queue, masked::queue, transparent::queue).in_set(RenderSystems::QueueMeshes),
                scene::read_back.in_set(RenderSystems::Cleanup),
            ),
        )
        .add_systems(
            Core3d,
            (
                (passes::raster, passes::prepass)
                    .chain()
                    .before(early_prepass)
                    .in_set(Core3dSystems::Prepass),
                passes::main_opaque
                    .before(main_opaque_pass_3d)
                    .in_set(Core3dSystems::MainPass),
            ),
        );
    debug::add_pass(render_app);
    true
}
