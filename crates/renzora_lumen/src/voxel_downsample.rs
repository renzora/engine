//! Voxel-cache mipmap-pyramid generation. Runs after `voxel_resolve`
//! fills mip 0 of the radiance texture, builds mips 1..N by repeated
//! 2× box-filter downsample of the previous mip.
//!
//! Cone trace consumes the pyramid via `textureLoad(voxels, coords, mip)` —
//! a widening cone picks a coarser mip per step, which means each
//! distant tap pulls a pre-averaged 2³ neighbourhood instead of a
//! single voxel. Replaces the `DISTANCE_FALLOFF` magic constant
//! `lumen_trace.wgsl` used to fake this attenuation.
//!
//! Cascade boundary safety: voxels are stacked along Z so cascade C
//! at mip M occupies Z ∈ [C × (RES >> M), (C+1) × (RES >> M)). Since
//! the cascade size at every mip is a power of two (64, 32, 16, 8),
//! a plain 2× downsample never reads across a cascade boundary —
//! pairs `(2k, 2k+1)` always land in the same cascade's Z range. No
//! cascade-aware indexing needed in the shader.

use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::render_graph::{
    NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
};
use bevy::render::render_resource::binding_types::{
    sampler, texture_3d, texture_storage_3d,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::RenderApp;
use bevy::utils::default;

use crate::voxel_cache::{
    VoxelCacheResources, VoxelCacheView, VoxelResolveLabel, VOXEL_MIP_COUNT, VOXEL_RES,
};

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct VoxelDownsampleLabel;

#[derive(Resource)]
pub struct VoxelDownsamplePipeline {
    pub layout: BindGroupLayoutDescriptor,
    pub pipeline_id: CachedComputePipelineId,
    pub linear_sampler: Sampler,
}

impl FromWorld for VoxelDownsamplePipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = BindGroupLayoutDescriptor::new(
            "voxel_downsample_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    // 0: source mip (sampled). Linear filter handles
                    // the 2³ box average for us in hardware.
                    texture_3d(TextureSampleType::Float { filterable: true }),
                    // 1: linear sampler.
                    sampler(SamplerBindingType::Filtering),
                    // 2: destination mip (storage write).
                    texture_storage_3d(
                        TextureFormat::Rgba16Float,
                        StorageTextureAccess::WriteOnly,
                    ),
                ),
            ),
        );

        let linear_sampler = render_device.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            // Clamp to edge so the source sample at a cascade Z
            // boundary doesn't wrap or repeat. (Even though the
            // 2× pair lands inside the cascade, near-edge linear
            // filter taps could pull from adjacent texels — clamp
            // keeps them on the same side.)
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            ..default()
        });

        let asset_server = world.resource::<AssetServer>();
        let shader = asset_server.load("embedded://renzora_lumen/voxel_downsample.wgsl");

        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline_id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("voxel_downsample_pipeline".into()),
            layout: vec![layout.clone()],
            shader,
            shader_defs: vec![],
            entry_point: Some("downsample".into()),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        });

        Self {
            layout,
            pipeline_id,
            linear_sampler,
        }
    }
}

#[derive(Default)]
pub struct VoxelDownsampleNode;

impl ViewNode for VoxelDownsampleNode {
    type ViewQuery = &'static VoxelCacheView;

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        view: QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        if !view.inject_active {
            return Ok(());
        }
        let _span = info_span!("voxel.downsample").entered();

        let pipeline = world.resource::<VoxelDownsamplePipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(compute) = pipeline_cache.get_compute_pipeline(pipeline.pipeline_id) else {
            return Ok(());
        };
        let Some(resources) = world.get_resource::<VoxelCacheResources>() else {
            return Ok(());
        };

        let layout = pipeline_cache.get_bind_group_layout(&pipeline.layout);

        // One bind group + dispatch per destination mip. Source is the
        // previous mip's all-levels view (sampled), dest is the
        // current mip's single-level view (storage).
        for dst_mip in 1..VOXEL_MIP_COUNT {
            let src_mip = dst_mip - 1;
            let bg = render_context.render_device().create_bind_group(
                "voxel_downsample_bg",
                &layout,
                &BindGroupEntries::sequential((
                    &resources.radiance_mip_views[src_mip as usize],
                    &pipeline.linear_sampler,
                    &resources.radiance_mip_views[dst_mip as usize],
                )),
            );

            let mut pass = render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: Some("voxel_downsample"),
                    timestamp_writes: None,
                });
            pass.set_pipeline(compute);
            pass.set_bind_group(0, &bg, &[]);
            // Destination mip size per axis. Workgroup 4×4×4 → ceil
            // divide. Z dimension covers all stacked cascades at this
            // mip level.
            let dst_res = (VOXEL_RES >> dst_mip).max(1);
            let dst_z = (super::voxel_cache::VOXEL_RES_Z >> dst_mip).max(1);
            let dispatch_xy = dst_res.div_ceil(4);
            let dispatch_z = dst_z.div_ceil(4);
            pass.dispatch_workgroups(dispatch_xy, dispatch_xy, dispatch_z);
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct VoxelDownsamplePlugin;

impl Plugin for VoxelDownsamplePlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "voxel_downsample.wgsl");

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_render_graph_node::<ViewNodeRunner<VoxelDownsampleNode>>(
                    Core3d,
                    VoxelDownsampleLabel,
                )
                // Slot right after the resolve fills mip 0. Other
                // consumers (screen-space reflection trace, lumen
                // trace) come later in the graph and read the
                // finished pyramid.
                .add_render_graph_edges(
                    Core3d,
                    (VoxelResolveLabel, VoxelDownsampleLabel),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<VoxelDownsamplePipeline>();
        }
    }
}
