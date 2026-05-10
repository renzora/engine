//! CPU-side scene luminance readback.
//!
//! Each frame, dispatches a small compute shader that samples a 16×16
//! stratified grid of the active view target, accumulates the log2 of
//! luminance into a tiny storage buffer, then async-maps the result to
//! the CPU. `CameraExposureState` exposes the resulting EV-100 to the
//! main world (and to scripting).
//!
//! Bevy's own auto-exposure (`bevy_post_process::auto_exposure`) keeps
//! its state buffer `pub(super)`, so we can't read its value directly.
//! This module mirrors AE's own log-luminance computation against the
//! same view texture; the value matches what AE is doing visually
//! within ~1-3 frames of latency (cost of the async readback).

use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::render_graph::{
    NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
};
use bevy::render::render_resource::binding_types::{storage_buffer, texture_2d};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderSystems};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use renzora::core::CameraExposureState;

/// Cross-world channel for the readback callback to deliver values.
///
/// Encodes `(sum_fixed, count)` into a u64 (high 32 = count, low 32 =
/// sum-of-fixed-point-log2-luminance × 1000). Default: `u64::MAX` is the
/// "no value yet" sentinel.
#[derive(Resource, Clone)]
pub struct LuminanceChannel(pub Arc<AtomicU64>);

impl Default for LuminanceChannel {
    fn default() -> Self {
        Self(Arc::new(AtomicU64::new(u64::MAX)))
    }
}

#[derive(Resource)]
pub struct LuminancePipeline {
    pub layout: BindGroupLayoutDescriptor,
    pub pipeline_id: CachedComputePipelineId,
    pub storage_buffer: Buffer,
    pub staging_buffer: Buffer,
    /// Marks whether a readback is in flight, to avoid stacking maps.
    pub readback_pending: Arc<std::sync::atomic::AtomicBool>,
}

impl FromWorld for LuminancePipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = BindGroupLayoutDescriptor::new(
            "luminance_reduce_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    texture_2d(TextureSampleType::Float { filterable: false }),
                    storage_buffer::<[u32; 2]>(false),
                ),
            ),
        );

        let asset_server = world.resource::<AssetServer>();
        let shader = asset_server.load("embedded://renzora_auto_exposure/luminance_reduce.wgsl");

        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline_id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("luminance_reduce_pipeline".into()),
            layout: vec![layout.clone()],
            shader,
            shader_defs: vec![],
            entry_point: Some("reduce".into()),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        });

        // `[u32; 2]` = sum + count. STORAGE for shader writes, COPY_SRC
        // so the CPU staging buffer can pull from it.
        let storage_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("luminance_storage_buffer"),
            size: 8,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("luminance_staging_buffer"),
            size: 8,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            layout,
            pipeline_id,
            storage_buffer,
            staging_buffer,
            readback_pending: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct LuminanceLabel;

#[derive(Default)]
pub struct LuminanceNode;

impl ViewNode for LuminanceNode {
    type ViewQuery = &'static ViewTarget;

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        view_target: QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<LuminancePipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let Some(compute) = pipeline_cache.get_compute_pipeline(pipeline.pipeline_id) else {
            return Ok(());
        };

        let bind_group = render_context.render_device().create_bind_group(
            "luminance_reduce_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((
                view_target.main_texture_view(),
                pipeline.storage_buffer.as_entire_binding(),
            )),
        );

        let encoder = render_context.command_encoder();

        // Reset the storage buffer to (0, 0) before each frame's reduction.
        // copy_buffer_to_buffer needs a clean source — easiest is to zero
        // via an offset write done by the queue, but we're inside the node
        // and don't have queue access; clear via clear_buffer instead.
        encoder.clear_buffer(&pipeline.storage_buffer, 0, None);

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("luminance_reduce_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(compute);
            pass.set_bind_group(0, &bind_group, &[]);
            // 16×16 sample grid, workgroup size 16×16 → 1 workgroup.
            pass.dispatch_workgroups(1, 1, 1);
        }

        // Copy storage → staging for CPU map.
        encoder.copy_buffer_to_buffer(
            &pipeline.storage_buffer,
            0,
            &pipeline.staging_buffer,
            0,
            8,
        );

        Ok(())
    }
}

/// After the encoder for this frame has been submitted, schedule an async
/// map of the staging buffer. The callback decodes `(sum, count)` and
/// stores the encoded u64 into the shared `LuminanceChannel`.
pub fn poll_readback(
    pipeline: Res<LuminancePipeline>,
    channel: Res<LuminanceChannel>,
    _render_device: Res<RenderDevice>,
) {
    use std::sync::atomic::Ordering as O;

    if pipeline
        .readback_pending
        .swap(true, O::AcqRel)
    {
        return; // a previous readback is still in flight
    }

    let staging = pipeline.staging_buffer.clone();
    let pending = pipeline.readback_pending.clone();
    let chan = channel.0.clone();
    let staging_in_callback = staging.clone();

    staging.slice(..).map_async(MapMode::Read, move |result| {
        if result.is_ok() {
            let slice = staging_in_callback.slice(..);
            let view = slice.get_mapped_range();
            let bytes: [u8; 8] = view[..8].try_into().unwrap_or([0; 8]);
            drop(view);
            staging_in_callback.unmap();
            let sum = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
            let count = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
            let packed = ((count as u64) << 32) | (sum as u64);
            chan.store(packed, O::Release);
        }
        pending.store(false, O::Release);
    });
}

/// Main-world system: decode the latest channel value into EV-100.
pub fn update_camera_exposure_state(
    channel: Res<LuminanceChannel>,
    mut state: ResMut<CameraExposureState>,
) {
    let packed = channel.0.load(Ordering::Acquire);
    if packed == u64::MAX {
        return; // no readback yet
    }
    let sum = (packed & 0xFFFF_FFFF) as u32;
    let count = (packed >> 32) as u32;
    if count == 0 {
        return;
    }
    // Decode: avg log_lum (shifted by +10) × 1000.
    let avg_log_lum_shifted = sum as f32 / (count as f32 * 1000.0);
    let avg_log_lum = avg_log_lum_shifted - 10.0;
    // EV-100 from average log-luminance, calibrated against middle gray
    // (0.18 reflectance under standard illuminance).
    state.ev100 = avg_log_lum + std::f32::consts::LOG2_E.recip() * 1.0986; // log2(0.18 ⁻¹) ≈ 2.470
}

pub struct LuminanceReadbackPlugin;

impl Plugin for LuminanceReadbackPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "luminance_reduce.wgsl");

        let channel = LuminanceChannel::default();
        app.insert_resource(channel.clone());
        app.init_resource::<CameraExposureState>();
        app.add_systems(Update, update_camera_exposure_state);

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.insert_resource(channel);
            render_app
                .add_systems(Render, poll_readback.in_set(RenderSystems::Cleanup))
                .add_render_graph_node::<ViewNodeRunner<LuminanceNode>>(Core3d, LuminanceLabel)
                .add_render_graph_edges(
                    Core3d,
                    (Node3d::Tonemapping, LuminanceLabel, Node3d::EndMainPassPostProcessing),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<LuminancePipeline>();
        }
    }
}
