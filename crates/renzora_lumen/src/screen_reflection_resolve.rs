//! Stage 4 of the Godot-style reflection filter pipeline: bilateral
//! upsample from the half-res blurred mip pyramid back to full res.
//!
//! For each full-res output pixel, samples the four nearest half-res
//! pyramid taps, weights each by `depth × normal × roughness`
//! similarity to the centre pixel, and combines. Bilateral weighting
//! is what prevents reflections from leaking across material/depth
//! boundaries during the upscale — e.g. a glass facade adjacent to
//! a brick wall won't smear reflections into the brick.
//!
//! Output: full-res `Rgba16Float` resolved reflection buffer that
//! `lumen_trace` samples directly with a single
//! `textureSampleLevel(resolved, uv, 0)` for the specular composite.

use bevy::core_pipeline::Core3d;
use bevy::core_pipeline::prepass::ViewPrepassTextures;
use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{
    sampler, texture_2d, texture_depth_2d, texture_storage_2d,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::RenderApp;
use bevy::utils::default;

use crate::lumen_trace::LumenTracePipeline;
use crate::screen_reflection::ScreenReflectionResources;
use crate::voxel_cache::VoxelCacheView;

#[derive(Resource)]
pub struct ScreenReflectionResolvePipeline {
    pub layout: BindGroupLayoutDescriptor,
    pub pipeline_id: CachedComputePipelineId,
    pub linear_sampler: Sampler,
}

impl FromWorld for ScreenReflectionResolvePipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = BindGroupLayoutDescriptor::new(
            "screen_reflection_resolve_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    // 0: half-res reflection pyramid (all mips) — sampled
                    // with linear filter at the chosen mip per pixel.
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // 1: linear sampler for the pyramid.
                    sampler(SamplerBindingType::Filtering),
                    // 2: half-res mip_level scalar — sampled bilinearly
                    // to upscale smoothly to full res.
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // 3: full-res depth prepass — used for bilateral
                    // weight (depth similarity test).
                    texture_depth_2d(),
                    // 4: full-res normal prepass — bilateral normal test.
                    texture_2d(TextureSampleType::Float { filterable: false }),
                    // 5: deferred G-buffer (Rgba32Uint) — roughness
                    // extracted from R channel for bilateral roughness
                    // test. Same fallback dummy from `LumenTracePipeline`
                    // when no DeferredPrepass is attached.
                    texture_2d(TextureSampleType::Uint),
                    // 6: full-res resolved output (Rgba16Float).
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
        let shader = asset_server.load("embedded://renzora_lumen/screen_reflection_resolve.wgsl");

        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline_id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("screen_reflection_resolve_pipeline".into()),
            layout: vec![layout.clone()],
            shader,
            shader_defs: vec![],
            entry_point: Some("resolve".into()),
            immediate_size: 0,
            zero_initialize_workgroup_memory: false,
        });

        Self {
            layout,
            pipeline_id,
            linear_sampler,
        }
    }
}

pub fn screen_reflection_resolve_pass(
    world: &World,
    view: ViewQuery<(
        &'static ViewPrepassTextures,
        &'static ScreenReflectionResources,
        &'static VoxelCacheView,
    )>,
    mut render_context: RenderContext,
) {
    let (prepass, refl, view) = view.into_inner();
    {
        if !view.inject_active {
            return;
        }
        let _span = info_span!("lumen.specular_resolve").entered();

        let pipeline = world.resource::<ScreenReflectionResolvePipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let Some(compute) = pipeline_cache.get_compute_pipeline(pipeline.pipeline_id) else {
            return;
        };
        let Some(depth_view) = prepass.depth_view() else {
            return;
        };
        let Some(normal_view) = prepass.normal_view() else {
            return;
        };

        // Same G-buffer fallback used by lumen_trace + screen_reflection
        // when the view has no DeferredPrepass.
        let lumen_pipeline = world.resource::<LumenTracePipeline>();
        let gbuffer_view = prepass
            .deferred_view()
            .unwrap_or(&lumen_pipeline.gbuffer_fallback);

        let bind_group = render_context.render_device().create_bind_group(
            "screen_reflection_resolve_bg",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((
                &refl.color_view_all_mips,
                &pipeline.linear_sampler,
                &refl.mip_level_view,
                depth_view,
                normal_view,
                gbuffer_view,
                &refl.resolved_view,
            )),
        );

        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {
                label: Some("screen_reflection_resolve"),
                timestamp_writes: None,
            });
        pass.set_pipeline(compute);
        pass.set_bind_group(0, &bind_group, &[]);
        // 8×8 tiles cover the full-res output.
        let dispatch_x = refl.resolved_size.width.div_ceil(8);
        let dispatch_y = refl.resolved_size.height.div_ceil(8);
        pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);

    }
}

#[derive(Default)]
pub struct ScreenReflectionResolvePlugin;

impl Plugin for ScreenReflectionResolvePlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "screen_reflection_resolve.wgsl");

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(
                    Core3d,
                    screen_reflection_resolve_pass.in_set(crate::LumenSystems::ScreenReflectionResolve),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<ScreenReflectionResolvePipeline>();
        }
    }
}
