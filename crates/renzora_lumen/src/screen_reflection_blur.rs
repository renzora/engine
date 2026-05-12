//! Stage 2 of the Godot-style reflection filter pipeline: build the
//! blurred mip pyramid that `lumen_trace` samples per-pixel based on
//! the trace stage's stored `mip_level`.
//!
//! For each mip level `N` (1..REFLECTION_MIP_COUNT), runs a separable
//! 7-tap Gaussian: source = mip `N-1` (the previous, sharper level),
//! destination = mip `N`. Two dispatches per level (horizontal then
//! vertical), so 8 total dispatches for a 5-mip pyramid.
//!
//! Why a pyramid instead of a single variable-radius blur:
//! A single-pass variable-radius blur needs ~64 taps per pixel for
//! the widest cones (rough surfaces at long ranges). The pyramid
//! approach uses 7 taps per pixel per level and lets later stages
//! sample the right pre-blurred level for the desired roughness —
//! the standard tradeoff for shipping reflections at real-time rates.
//!
//! Two pipelines: one for the horizontal pass, one for the vertical.
//! Both run the same shader, differentiated by a shader_def. The
//! horizontal pass writes into a separate `temp` buffer; the vertical
//! pass reads from `temp` and writes into mip `N` of the colour
//! pyramid. Standard separable-Gaussian dance.

use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::render_graph::{
    NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
};
use bevy::render::render_resource::binding_types::{
    sampler, texture_2d, texture_storage_2d,
};
use bevy::render::render_resource::{
    AddressMode, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
    CachedComputePipelineId, ComputePassDescriptor, ComputePipelineDescriptor, Extent3d,
    FilterMode, PipelineCache, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
    StorageTextureAccess, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor,
};
use bevy::shader::ShaderDefVal;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::utils::default;

use crate::lumen_trace::LumenTraceLabel;
use crate::screen_reflection::{
    ScreenReflectionResources, ScreenReflectionTraceLabel, REFLECTION_MIP_COUNT,
};
use crate::voxel_cache::VoxelCacheView;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct ScreenReflectionBlurLabel;

#[derive(Resource)]
pub struct ScreenReflectionBlurPipeline {
    pub layout: BindGroupLayoutDescriptor,
    pub horizontal_pipeline: CachedComputePipelineId,
    pub vertical_pipeline: CachedComputePipelineId,
    pub linear_sampler: Sampler,
}

#[derive(Component)]
pub struct ScreenReflectionBlurResources {
    /// Per-mip temporary buffers used as the intermediate destination
    /// of the horizontal blur pass. Each entry matches the resolution
    /// of the corresponding mip in the colour pyramid (so writing
    /// into a 1×1 temp for mip 4 doesn't clobber the larger temps).
    pub temp_views: Vec<TextureView>,
    /// Cached extent so resize detection matches the trace pass.
    pub size: Extent3d,
}

impl Clone for ScreenReflectionBlurResources {
    fn clone(&self) -> Self {
        Self {
            temp_views: self.temp_views.clone(),
            size: self.size,
        }
    }
}

impl FromWorld for ScreenReflectionBlurPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = BindGroupLayoutDescriptor::new(
            "screen_reflection_blur_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    // 0: source — previous mip (sampled, filterable).
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // 1: linear sampler for the source.
                    sampler(SamplerBindingType::Filtering),
                    // 2: destination storage texture (current mip).
                    texture_storage_2d(
                        TextureFormat::Rgba16Float,
                        StorageTextureAccess::WriteOnly,
                    ),
                ),
            ),
        );

        let linear_sampler = render_device.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            ..default()
        });

        let asset_server = world.resource::<AssetServer>();
        let shader = asset_server.load("embedded://renzora_lumen/screen_reflection_blur.wgsl");

        let pipeline_cache = world.resource::<PipelineCache>();
        let horizontal_pipeline =
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some("screen_reflection_blur_h".into()),
                layout: vec![layout.clone()],
                shader: shader.clone(),
                shader_defs: vec![ShaderDefVal::Bool("HORIZONTAL".into(), true)],
                entry_point: Some("blur".into()),
                push_constant_ranges: vec![],
                zero_initialize_workgroup_memory: false,
            });
        let vertical_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("screen_reflection_blur_v".into()),
            layout: vec![layout.clone()],
            shader,
            shader_defs: vec![],
            entry_point: Some("blur".into()),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        });

        Self {
            layout,
            horizontal_pipeline,
            vertical_pipeline,
            linear_sampler,
        }
    }
}

/// Allocate the per-mip temp buffers used between the horizontal
/// and vertical blur passes. One buffer per pyramid level, each at
/// the matching mip resolution. We don't reuse a single large temp
/// because storage textures require matching sizes for `textureStore`
/// (writing past the bound view's extent is UB).
pub fn prepare_blur_resources(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    views: Query<
        (
            Entity,
            &ScreenReflectionResources,
            Option<&ScreenReflectionBlurResources>,
        ),
        With<VoxelCacheView>,
    >,
) {
    for (entity, refl, existing) in &views {
        let needs_alloc = match existing {
            Some(res) => res.size != refl.size,
            None => true,
        };
        if !needs_alloc {
            continue;
        }

        let temp_views = (0..REFLECTION_MIP_COUNT)
            .map(|mip| {
                let mip_size = Extent3d {
                    width: (refl.size.width >> mip).max(1),
                    height: (refl.size.height >> mip).max(1),
                    depth_or_array_layers: 1,
                };
                let tex = render_device.create_texture(&TextureDescriptor {
                    label: Some("screen_reflection_blur_temp_mip"),
                    size: mip_size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::Rgba16Float,
                    usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                tex.create_view(&TextureViewDescriptor::default())
            })
            .collect::<Vec<_>>();

        commands.entity(entity).insert(ScreenReflectionBlurResources {
            temp_views,
            size: refl.size,
        });
    }
}

#[derive(Default)]
pub struct ScreenReflectionBlurNode;

impl ViewNode for ScreenReflectionBlurNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static ScreenReflectionResources,
        &'static ScreenReflectionBlurResources,
        &'static VoxelCacheView,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (_view_target, refl, blur, view): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        if !view.inject_active {
            return Ok(());
        }

        let pipeline = world.resource::<ScreenReflectionBlurPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let Some(h_pipeline) =
            pipeline_cache.get_compute_pipeline(pipeline.horizontal_pipeline)
        else {
            return Ok(());
        };
        let Some(v_pipeline) = pipeline_cache.get_compute_pipeline(pipeline.vertical_pipeline)
        else {
            return Ok(());
        };
        let layout = pipeline_cache.get_bind_group_layout(&pipeline.layout);

        // Build pyramid: mip N gets the blurred + downsampled view of
        // mip N-1. We iterate from 1 upward — mip 0 is the raw trace
        // output, untouched by this pass.
        for mip in 1..REFLECTION_MIP_COUNT {
            let mip_w = (refl.size.width >> mip).max(1);
            let mip_h = (refl.size.height >> mip).max(1);
            let dispatch_x = (mip_w + 7) / 8;
            let dispatch_y = (mip_h + 7) / 8;

            // Horizontal pass: source = previous mip of pyramid,
            // destination = temp buffer at the current mip's size.
            let source_view = &refl.color_view_per_mip[(mip - 1) as usize];
            let temp_view = &blur.temp_views[mip as usize];
            let h_bg = render_context.render_device().create_bind_group(
                "screen_reflection_blur_h_bg",
                &layout,
                &BindGroupEntries::sequential((
                    source_view,
                    &pipeline.linear_sampler,
                    temp_view,
                )),
            );

            // Vertical pass: source = temp buffer, destination = current
            // mip of pyramid.
            let dest_view = &refl.color_view_per_mip[mip as usize];
            let v_bg = render_context.render_device().create_bind_group(
                "screen_reflection_blur_v_bg",
                &layout,
                &BindGroupEntries::sequential((
                    temp_view,
                    &pipeline.linear_sampler,
                    dest_view,
                )),
            );

            let mut pass = render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: Some("screen_reflection_blur"),
                    timestamp_writes: None,
                });
            pass.set_pipeline(h_pipeline);
            pass.set_bind_group(0, &h_bg, &[]);
            pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
            pass.set_pipeline(v_pipeline);
            pass.set_bind_group(0, &v_bg, &[]);
            pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct ScreenReflectionBlurPlugin;

impl Plugin for ScreenReflectionBlurPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "screen_reflection_blur.wgsl");

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(
                    Render,
                    prepare_blur_resources.in_set(RenderSystems::PrepareResources),
                )
                .add_render_graph_node::<ViewNodeRunner<ScreenReflectionBlurNode>>(
                    Core3d,
                    ScreenReflectionBlurLabel,
                )
                // Slot between the trace (so we have raw mip 0 to
                // pyramid-ify) and the main lumen trace (which samples
                // the pyramid).
                .add_render_graph_edges(
                    Core3d,
                    (
                        ScreenReflectionTraceLabel,
                        ScreenReflectionBlurLabel,
                        LumenTraceLabel,
                    ),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<ScreenReflectionBlurPipeline>();
        }
    }
}
