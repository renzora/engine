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

#[derive(Component)]
pub struct ScreenReflectionResources {
    /// Half-res reflection output. RGB = traced color, A = validity
    /// (0.0 = miss / off-screen / sky, 1.0 = solid screen-space hit
    /// well inside the screen margin). Stage 2's blur will read this
    /// and produce a softened version; stage 4's bilateral resolve
    /// will weight upsampled samples by validity so invalid pixels
    /// don't smear into valid ones.
    pub color: TextureView,
    /// Cached half-res size so we can detect view-target resizes and
    /// recreate the buffer.
    pub size: Extent3d,
}

impl Clone for ScreenReflectionResources {
    fn clone(&self) -> Self {
        Self {
            color: self.color.clone(),
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
                    // 5: half-res reflection output. Storage texture
                    // we write into; the lumen_trace pass binds the
                    // matching sampled view at slot 14 to read it.
                    texture_storage_2d(
                        TextureFormat::Rgba16Float,
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
            label: Some("screen_reflection_color"),
            size: half_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        commands.entity(entity).insert(ScreenReflectionResources {
            color: color_tex.create_view(&TextureViewDescriptor::default()),
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

        // Sample the scene at its current state — opaque pass and
        // voxel-resolve have both finished by the time this node
        // runs, so reflections can pick up direct lighting plus the
        // baked indirect from the voxel cache.
        let scene_view = view_target.main_texture_view();

        let bind_group = render_context.render_device().create_bind_group(
            "screen_reflection_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((
                scene_view,
                &pipeline.scene_sampler,
                depth_view,
                normal_view,
                view_binding.clone(),
                &resources.color,
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
