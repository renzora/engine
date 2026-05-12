//! Half-resolution dedicated screen-space reflection trace pass.
//!
//! Stage 1 of the Godot-style SSR-meets-Lumen reflection pipeline:
//! marches reflection rays in world space at half view resolution
//! and writes the hits into a dedicated `Rgba16Float` buffer. The
//! eventual stages will sit on top of this buffer:
//!
//!   Stage 1 (this file): half-res trace → color + validity
//!   Stage 2:              separable Gaussian blur of the buffer
//!   Stage 3:              promote blur to mip pyramid
//!   Stage 4:              bilateral upsample resolve in `lumen_trace`
//!
//! Why half resolution: convolving full-res reflections is expensive
//! for what's perceptually a low-frequency signal — Godot, UE5,
//! Frostbite, and others all decimate to half res, blur, then
//! bilaterally upscale to preserve edges. The 4× pixel-count saving
//! pays for the extra blur/resolve passes with a budget left over.
//!
//! Runs as a render-graph view node between `VoxelResolveLabel` and
//! `LumenTraceLabel` so the resolved buffer is available when the
//! main composite pass samples it.

use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::core_pipeline::prepass::ViewPrepassTextures;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::render_graph::{
    NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
};
use bevy::render::render_resource::binding_types::{
    sampler, texture_2d, texture_depth_2d, texture_storage_2d, uniform_buffer,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::view::{ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms};
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::utils::default;

use crate::lumen_trace::LumenTraceLabel;
use crate::voxel_cache::{VoxelCacheView, VoxelResolveLabel};

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct ScreenReflectionTraceLabel;

#[derive(Resource)]
pub struct ScreenReflectionPipeline {
    pub layout: BindGroupLayoutDescriptor,
    pub pipeline_id: CachedComputePipelineId,
    pub scene_sampler: Sampler,
}

/// Number of mip levels in the half-res reflection pyramid.
/// Level 0 is the raw trace at half-res; each subsequent level is
/// the previous level blurred + downsampled. Stage 2's blur pass
/// fills levels 1..MIP_COUNT, then `lumen_trace` samples the
/// pyramid using each pixel's stored `mip_level` (computed from
/// roughness in the trace stage). 5 levels gives ~32× decimation
/// at the coarsest — covers the perceptual roughness range from
/// mirror-sharp at level 0 to fully-rough-looking at level 4.
pub const REFLECTION_MIP_COUNT: u32 = 5;

#[derive(Component)]
pub struct ScreenReflectionResources {
    /// Half-res reflection pyramid. RGB = traced colour, A = validity
    /// (0.0 = miss / off-screen / sky, 1.0 = solid screen-space hit
    /// well inside the screen margin). Mip 0 = raw trace, mips 1..N
    /// are progressively blurrier (filled by `screen_reflection_blur`).
    /// `lumen_trace` samples the pyramid as a single texture and uses
    /// the stored per-pixel `mip_level` to pick how blurry to read.
    pub color: Texture,
    /// View over all mip levels — `lumen_trace` samples this with
    /// `textureSampleLevel(..., uv, mip_level)` to pick blurriness.
    pub color_view_all_mips: TextureView,
    /// One view per mip level. Storage views the blur pass uses as
    /// its source (mip N-1) / destination (mip N) for each downsample
    /// step. Index 0 is also where the trace writes the raw color.
    pub color_view_per_mip: Vec<TextureView>,
    /// Half-res scalar buffer holding the per-pixel mip level
    /// selected from roughness in the trace stage. Sampled bilinearly
    /// at full res in `lumen_trace`. Encoded as a float in [0, N]
    /// where N = REFLECTION_MIP_COUNT - 1.
    pub mip_level_view: TextureView,
    /// Cached half-res size so we can detect view-target resizes and
    /// recreate the buffer.
    pub size: Extent3d,
}

impl Clone for ScreenReflectionResources {
    fn clone(&self) -> Self {
        Self {
            color: self.color.clone(),
            color_view_all_mips: self.color_view_all_mips.clone(),
            color_view_per_mip: self.color_view_per_mip.clone(),
            mip_level_view: self.mip_level_view.clone(),
            size: self.size,
        }
    }
}

impl FromWorld for ScreenReflectionPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = BindGroupLayoutDescriptor::new(
            "screen_reflection_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    // 0: scene HDR (full-res input — we sample at the
                    // half-res ray hit point with linear filtering)
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // 1: scene sampler
                    sampler(SamplerBindingType::Filtering),
                    // 2: depth prepass (full-res — the ray march
                    // tests against this every step)
                    texture_depth_2d(),
                    // 3: normal prepass (we read the surface normal
                    // at the representative full-res pixel for each
                    // half-res output)
                    texture_2d(TextureSampleType::Float { filterable: false }),
                    // 4: view uniform — clip_from_world, view_from_world,
                    // world_position. Dynamic offset so we can
                    // share the binding across multi-view setups.
                    uniform_buffer::<ViewUniform>(true),
                    // 5: deferred G-buffer. R channel packs
                    // `(base_color_srgb, perceptual_roughness)` as
                    // unorm4x8 — we unpack roughness to drive the
                    // mip_level computation (Godot's cone-of-confusion
                    // formula: rougher surfaces or longer rays get a
                    // larger blur radius → higher mip level).
                    texture_2d(TextureSampleType::Uint),
                    // 6: half-res reflection color output (mip 0 of
                    // the pyramid). The blur pass fills mips 1..N
                    // by repeatedly blurring and downsampling.
                    texture_storage_2d(
                        TextureFormat::Rgba16Float,
                        StorageTextureAccess::WriteOnly,
                    ),
                    // 7: half-res scalar mip_level output. `lumen_trace`
                    // samples this with bilinear filtering and reads
                    // it as the LOD into the reflection pyramid.
                    // R32Float (not R16Float) — wgpu only guarantees
                    // *write* storage for the wide-format tier; R16
                    // is opt-in via STORAGE_TEXTURE_FORMAT_R16_FLOAT
                    // feature, which isn't enabled here. R32 costs
                    // ~2KB extra at 1080p half-res — negligible.
                    texture_storage_2d(
                        TextureFormat::R32Float,
                        StorageTextureAccess::WriteOnly,
                    ),
                ),
            ),
        );

        let scene_sampler = render_device.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            ..default()
        });

        let asset_server = world.resource::<AssetServer>();
        let shader = asset_server.load("embedded://renzora_lumen/screen_reflection.wgsl");

        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline_id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("screen_reflection_pipeline".into()),
            layout: vec![layout.clone()],
            shader,
            shader_defs: vec![],
            entry_point: Some("trace".into()),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        });

        Self {
            layout,
            pipeline_id,
            scene_sampler,
        }
    }
}

/// Allocate (or re-allocate on resize) the half-res reflection
/// buffer for every camera that's running Lumen GI.
pub fn prepare_screen_reflection_resources(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    views: Query<
        (Entity, &ViewTarget, Option<&ScreenReflectionResources>),
        With<VoxelCacheView>,
    >,
) {
    for (entity, view_target, existing) in &views {
        let target_size = view_target.main_texture().size();
        // Half-res — ceil-divide so a 1px-wide view still gets one
        // output pixel (avoids zero-sized texture allocation).
        let half_size = Extent3d {
            width: (target_size.width + 1) / 2,
            height: (target_size.height + 1) / 2,
            depth_or_array_layers: 1,
        };

        let needs_alloc = match existing {
            Some(res) => res.size != half_size,
            None => true,
        };
        if !needs_alloc {
            continue;
        }

        let color_tex = render_device.create_texture(&TextureDescriptor {
            label: Some("screen_reflection_color_pyramid"),
            size: half_size,
            mip_level_count: REFLECTION_MIP_COUNT,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        // View covering all mips — `lumen_trace` samples this with
        // `textureSampleLevel(..., uv, mip_level)` to pick blurriness.
        let color_view_all_mips = color_tex.create_view(&TextureViewDescriptor {
            label: Some("screen_reflection_color_all_mips"),
            base_mip_level: 0,
            mip_level_count: Some(REFLECTION_MIP_COUNT),
            ..default()
        });

        // Per-mip single-level views — the blur pass binds these as
        // storage for write at mip N and as a sampled texture for
        // read at mip N-1.
        let color_view_per_mip = (0..REFLECTION_MIP_COUNT)
            .map(|mip| {
                color_tex.create_view(&TextureViewDescriptor {
                    label: Some("screen_reflection_color_mip"),
                    base_mip_level: mip,
                    mip_level_count: Some(1),
                    ..default()
                })
            })
            .collect::<Vec<_>>();

        // Mip-level scalar buffer — half-res, single channel,
        // bilinear-sampled by `lumen_trace`. R32Float so the storage
        // binding is in wgpu's guaranteed format tier (R16Float is
        // optional via a feature flag).
        let mip_level_tex = render_device.create_texture(&TextureDescriptor {
            label: Some("screen_reflection_mip_level"),
            size: half_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R32Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        commands.entity(entity).insert(ScreenReflectionResources {
            color: color_tex,
            color_view_all_mips,
            color_view_per_mip,
            mip_level_view: mip_level_tex.create_view(&TextureViewDescriptor::default()),
            size: half_size,
        });
    }
}

#[derive(Default)]
pub struct ScreenReflectionTraceNode;

impl ViewNode for ScreenReflectionTraceNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static ViewUniformOffset,
        &'static ViewPrepassTextures,
        &'static ScreenReflectionResources,
        &'static VoxelCacheView,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, view_offset, prepass, resources, view): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        // Same gate as the lumen_trace pass — only fire when GI is
        // actually active. Avoids wasting GPU time when the user
        // toggles Lumen quality to Off mid-frame.
        if !view.inject_active {
            return Ok(());
        }

        let pipeline = world.resource::<ScreenReflectionPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let view_uniforms = world.resource::<ViewUniforms>();

        let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(pipeline.pipeline_id)
        else {
            return Ok(());
        };
        let Some(view_binding) = view_uniforms.uniforms.binding() else {
            return Ok(());
        };
        let Some(depth_view) = prepass.depth_view() else {
            return Ok(());
        };
        let Some(normal_view) = prepass.normal_view() else {
            return Ok(());
        };

        // Deferred G-buffer — same source as `lumen_trace`. Roughness
        // packed in R channel alpha drives the per-pixel mip selection.
        // Forward views without a deferred prepass fall back to the
        // shared dummy texture from `LumenTracePipeline`, with the
        // shader gating itself off via `use_albedo_modulation == 0`.
        let lumen_pipeline = world.resource::<crate::lumen_trace::LumenTracePipeline>();
        let gbuffer_view = prepass
            .deferred_view()
            .unwrap_or(&lumen_pipeline.gbuffer_fallback);

        // Sample the scene at its current state — opaque pass and
        // voxel-resolve have both finished by the time this node
        // runs, so reflections can pick up direct lighting plus the
        // baked indirect from the voxel cache.
        let scene_view = view_target.main_texture_view();

        // Mip 0 of the colour pyramid — raw trace output. The blur
        // pass takes it from here.
        let color_mip0 = &resources.color_view_per_mip[0];

        let bind_group = render_context.render_device().create_bind_group(
            "screen_reflection_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((
                scene_view,
                &pipeline.scene_sampler,
                depth_view,
                normal_view,
                view_binding.clone(),
                gbuffer_view,
                color_mip0,
                &resources.mip_level_view,
            )),
        );

        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {
                label: Some("screen_reflection_trace"),
                timestamp_writes: None,
            });
        pass.set_pipeline(compute_pipeline);
        pass.set_bind_group(0, &bind_group, &[view_offset.offset]);
        // Workgroup 8x8 — covers a tile of half-res output pixels.
        // `(size + 7) / 8` round-up so the right/bottom edges don't
        // get clipped when the half-res size isn't a multiple of 8.
        let dispatch_x = (resources.size.width + 7) / 8;
        let dispatch_y = (resources.size.height + 7) / 8;
        pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);

        Ok(())
    }
}

#[derive(Default)]
pub struct ScreenReflectionPlugin;

impl Plugin for ScreenReflectionPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "screen_reflection.wgsl");

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(
                    Render,
                    prepare_screen_reflection_resources.in_set(RenderSystems::PrepareResources),
                )
                .add_render_graph_node::<ViewNodeRunner<ScreenReflectionTraceNode>>(
                    Core3d,
                    ScreenReflectionTraceLabel,
                )
                // Slot between the voxel cache resolve (so the cache
                // has fresh radiance for any fallback paths the
                // reflection blend might use) and the main lumen
                // trace (which reads our output buffer).
                .add_render_graph_edges(
                    Core3d,
                    (
                        VoxelResolveLabel,
                        ScreenReflectionTraceLabel,
                        LumenTraceLabel,
                    ),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<ScreenReflectionPipeline>();
        }
    }
}
