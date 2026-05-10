//! Phase 2 V1 voxel radiance cache.
//!
//! Single-cascade 64³ Rgba16Float grid, camera-centered, voxel-snapped.
//! The full plan calls for 4 cascades + EMA + cascade scrolling — this
//! is the smallest useful slice that produces a debug-visible cache:
//!
//!   - Allocation: one 3D texture per app, sized 64³.
//!   - Snap origin: each frame, `floor(camera_pos / voxel_size) * voxel_size`
//!     so the grid doesn't flicker on sub-voxel motion.
//!   - Inject: a compute pass walks the post-tonemap scene texture,
//!     reads each pixel's world position via the depth prepass, and
//!     atomically accumulates that pixel's color into the voxel
//!     containing it. Many pixels per voxel = average via the per-voxel
//!     count.
//!   - Clear: full clear at start of each frame. Simplest correctness
//!     story; once we add 4 cascades + EMA, this becomes scroll-clear.
//!
//! Geometry voxelization (rasterize meshes into the grid) is deferred
//! — without it, the cache only contains visible-surface lighting, so
//! later off-screen tracing won't see anything in voxels behind walls.
//! That's a Phase 2.5 / Phase 3 scope item.

use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::core_pipeline::prepass::ViewPrepassTextures;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_graph::{
    NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
};
use bevy::render::render_resource::binding_types::{
    sampler, storage_buffer_sized, texture_2d, texture_depth_2d, texture_storage_3d, uniform_buffer,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::view::{ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms};
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::utils::default;
use bytemuck::{Pod, Zeroable};

use crate::{LumenDebug, LumenLighting, LumenQuality};

pub const VOXEL_RES: u32 = 64;
pub const VOXEL_SIZE: f32 = 0.5;

/// Per-camera component carried into the render world by extraction.
/// Drives whether the voxel inject + debug passes execute for this view.
#[derive(Component, Clone, Copy, Debug)]
pub struct VoxelCacheView {
    pub inject_active: bool,
    pub debug_active: bool,
}

impl ExtractComponent for VoxelCacheView {
    type QueryData = &'static VoxelCacheView;
    type QueryFilter = ();
    type Out = VoxelCacheView;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(*item)
    }
}

/// Update per-camera `VoxelCacheView` markers based on the active
/// `LumenLighting` settings on each routed camera. Voxel injection runs
/// for `SdfLow` and above; debug view runs whenever `LumenDebug::VoxelCache`
/// is selected (independent of quality so you can preview the cache
/// even when GI is otherwise off).
pub fn sync_voxel_cache_views(
    mut commands: Commands,
    cameras: Query<(Entity, Option<&LumenLighting>), With<Camera3d>>,
) {
    for (entity, settings) in &cameras {
        let (inject_active, debug_active) = match settings {
            Some(s) => {
                let inject = matches!(
                    s.quality,
                    LumenQuality::SdfLow | LumenQuality::SdfHigh | LumenQuality::Hwrt
                );
                let debug = matches!(s.debug, LumenDebug::VoxelCache);
                (inject, debug)
            }
            None => (false, false),
        };
        commands.entity(entity).insert(VoxelCacheView {
            inject_active,
            debug_active,
        });
    }
}

/// World-space origin + voxel size for the current frame's grid. Sent
/// to GPU as the only uniform the voxel shaders need (camera/view
/// matrices come from the standard `ViewUniform`).
#[derive(Clone, Copy, Debug, Pod, Zeroable, ShaderType)]
#[repr(C)]
pub struct VoxelGridUniform {
    pub origin: Vec3,
    pub voxel_size: f32,
    pub resolution: u32,
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
}

#[derive(Resource)]
pub struct VoxelCachePipelines {
    pub inject_layout: BindGroupLayoutDescriptor,
    pub inject_pipeline: CachedComputePipelineId,
    pub clear_layout: BindGroupLayoutDescriptor,
    pub clear_pipeline: CachedComputePipelineId,
    pub resolve_layout: BindGroupLayoutDescriptor,
    pub resolve_pipeline: CachedComputePipelineId,
    pub debug_layout: BindGroupLayoutDescriptor,
    pub debug_pipeline: CachedRenderPipelineId,
    pub sampler: Sampler,
}

#[derive(Resource)]
pub struct VoxelCacheResources {
    /// Persistent storage texture holding the resolved per-voxel
    /// radiance. Read by the debug view + later phases' tracers.
    pub radiance: TextureView,
    /// Per-frame uniform with the snapped origin.
    pub uniform_buffer: Buffer,
    /// Per-frame accumulation buffer: 4 u32s per voxel (sum_r, sum_g,
    /// sum_b in fixed-point ×256, plus contributor count). Cleared
    /// before inject, drained by resolve.
    pub accum_buffer: Buffer,
}

impl FromWorld for VoxelCachePipelines {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let asset_server = world.resource::<AssetServer>();

        // Accumulation buffer is 64³ × 4 u32s = 4 MB. Bind size matches.
        let accum_size = (VOXEL_RES * VOXEL_RES * VOXEL_RES * 4 * 4) as u64;
        let accum_size_nz = std::num::NonZeroU64::new(accum_size).unwrap();

        // ── inject ───────────────────────────────────────────────────
        let inject_layout = BindGroupLayoutDescriptor::new(
            "voxel_inject_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    // 0: scene HDR (post-tonemap fine since we just want lit color)
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // 1: depth prepass
                    texture_depth_2d(),
                    // 2: per-frame accumulation buffer (atomic adds)
                    storage_buffer_sized(false, Some(accum_size_nz)),
                    // 3: view uniform (dynamic offset)
                    uniform_buffer::<ViewUniform>(true),
                    // 4: voxel grid uniform
                    uniform_buffer::<VoxelGridUniform>(false),
                ),
            ),
        );
        let inject_shader = asset_server.load("embedded://renzora_lumen/voxel_inject.wgsl");
        let pipeline_cache = world.resource::<PipelineCache>();
        let inject_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("voxel_inject_pipeline".into()),
            layout: vec![inject_layout.clone()],
            shader: inject_shader,
            shader_defs: vec![],
            entry_point: Some("inject".into()),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        });

        // ── clear (accumulation buffer) ──────────────────────────────
        let clear_layout = BindGroupLayoutDescriptor::new(
            "voxel_clear_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (storage_buffer_sized(false, Some(accum_size_nz)),),
            ),
        );
        let clear_shader = asset_server.load("embedded://renzora_lumen/voxel_clear.wgsl");
        let clear_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("voxel_clear_pipeline".into()),
            layout: vec![clear_layout.clone()],
            shader: clear_shader,
            shader_defs: vec![],
            entry_point: Some("clear".into()),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        });

        // ── resolve (drain accumulation → radiance texture) ──────────
        let resolve_layout = BindGroupLayoutDescriptor::new(
            "voxel_resolve_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    storage_buffer_sized(false, Some(accum_size_nz)),
                    texture_storage_3d(TextureFormat::Rgba16Float, StorageTextureAccess::ReadWrite),
                ),
            ),
        );
        let resolve_shader = asset_server.load("embedded://renzora_lumen/voxel_resolve.wgsl");
        let resolve_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("voxel_resolve_pipeline".into()),
            layout: vec![resolve_layout.clone()],
            shader: resolve_shader,
            shader_defs: vec![],
            entry_point: Some("resolve".into()),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        });

        // ── debug view ────────────────────────────────────────────────
        let debug_layout = BindGroupLayoutDescriptor::new(
            "voxel_debug_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    // 0: voxel radiance (sampled)
                    bevy::render::render_resource::binding_types::texture_3d(
                        TextureSampleType::Float { filterable: true },
                    ),
                    // 1: linear sampler
                    sampler(SamplerBindingType::Filtering),
                    // 2: depth prepass — used to find each pixel's world pos
                    texture_depth_2d(),
                    // 3: scene tex (passthrough fallback)
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // 4: view uniform
                    uniform_buffer::<ViewUniform>(true),
                    // 5: voxel grid uniform
                    uniform_buffer::<VoxelGridUniform>(false),
                ),
            ),
        );
        let debug_shader = asset_server.load("embedded://renzora_lumen/voxel_debug.wgsl");
        let fullscreen = world.resource::<bevy::core_pipeline::FullscreenShader>();
        let debug_pipeline = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("voxel_debug_pipeline".into()),
            layout: vec![debug_layout.clone()],
            vertex: fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: debug_shader,
                shader_defs: vec![],
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: ViewTarget::TEXTURE_FORMAT_HDR,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        });

        let sampler = render_device.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..default()
        });

        Self {
            inject_layout,
            inject_pipeline,
            clear_layout,
            clear_pipeline,
            resolve_layout,
            resolve_pipeline,
            debug_layout,
            debug_pipeline,
            sampler,
        }
    }
}

/// Allocate the voxel texture + uniform buffer once and refresh the
/// per-frame snapped origin.
pub fn prepare_voxel_resources(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    existing: Option<Res<VoxelCacheResources>>,
    cameras: Query<&bevy::render::view::ExtractedView, With<VoxelCacheView>>,
) {
    let camera_pos = cameras
        .iter()
        .next()
        .map(|v| v.world_from_view.translation())
        .unwrap_or(Vec3::ZERO);

    // Snap origin to voxel-size increments so half-voxel camera motion
    // is invisible. Centered on the camera (origin = corner of grid).
    let snapped_center = (camera_pos / VOXEL_SIZE).floor() * VOXEL_SIZE;
    let half_extent = (VOXEL_RES as f32 * VOXEL_SIZE) * 0.5;
    let origin = snapped_center - Vec3::splat(half_extent);

    let uniform = VoxelGridUniform {
        origin,
        voxel_size: VOXEL_SIZE,
        resolution: VOXEL_RES,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    };

    if let Some(res) = existing {
        render_queue.write_buffer(&res.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
        return;
    }

    let texture = render_device.create_texture(&TextureDescriptor {
        label: Some("voxel_radiance"),
        size: Extent3d {
            width: VOXEL_RES,
            height: VOXEL_RES,
            depth_or_array_layers: VOXEL_RES,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D3,
        format: TextureFormat::Rgba16Float,
        usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let uniform_buffer = render_device.create_buffer(&BufferDescriptor {
        label: Some("voxel_grid_uniform"),
        size: std::mem::size_of::<VoxelGridUniform>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    render_queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&uniform));

    let accum_size = (VOXEL_RES * VOXEL_RES * VOXEL_RES * 4 * 4) as u64;
    let accum_buffer = render_device.create_buffer(&BufferDescriptor {
        label: Some("voxel_accum"),
        size: accum_size,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    commands.insert_resource(VoxelCacheResources {
        radiance: texture.create_view(&TextureViewDescriptor::default()),
        uniform_buffer,
        accum_buffer,
    });
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct VoxelInjectLabel;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct VoxelDebugLabel;

#[derive(Default)]
pub struct VoxelInjectNode;

impl ViewNode for VoxelInjectNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static ViewUniformOffset,
        &'static ViewPrepassTextures,
        &'static VoxelCacheView,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, view_offset, prepass, view): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        if !view.inject_active {
            return Ok(());
        }

        let pipelines = world.resource::<VoxelCachePipelines>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let view_uniforms = world.resource::<ViewUniforms>();

        let Some(resources) = world.get_resource::<VoxelCacheResources>() else {
            return Ok(());
        };
        let Some(view_binding) = view_uniforms.uniforms.binding() else {
            return Ok(());
        };
        let Some(depth_view) = prepass.depth_view() else { return Ok(()); };

        // ── Pass 1: clear accumulation buffer ────────────────────────
        if let Some(clear_pl) = pipeline_cache.get_compute_pipeline(pipelines.clear_pipeline) {
            let clear_bg = render_context.render_device().create_bind_group(
                "voxel_clear_bg",
                &pipeline_cache.get_bind_group_layout(&pipelines.clear_layout),
                &BindGroupEntries::sequential((resources.accum_buffer.as_entire_binding(),)),
            );
            let mut pass = render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: Some("voxel_clear_accum"),
                    timestamp_writes: None,
                });
            pass.set_pipeline(clear_pl);
            pass.set_bind_group(0, &clear_bg, &[]);
            // 64³ × 4 = 1M u32 entries; workgroup_size 64 → 16384 groups.
            let entries = (VOXEL_RES * VOXEL_RES * VOXEL_RES * 4) / 64;
            pass.dispatch_workgroups(entries, 1, 1);
        }

        // ── Pass 2: inject visible-surface contributions ─────────────
        if let Some(inject_pl) = pipeline_cache.get_compute_pipeline(pipelines.inject_pipeline) {
            let inject_bg = render_context.render_device().create_bind_group(
                "voxel_inject_bg",
                &pipeline_cache.get_bind_group_layout(&pipelines.inject_layout),
                &BindGroupEntries::sequential((
                    view_target.main_texture_view(),
                    depth_view,
                    resources.accum_buffer.as_entire_binding(),
                    view_binding.clone(),
                    resources.uniform_buffer.as_entire_binding(),
                )),
            );
            let mut pass = render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: Some("voxel_inject"),
                    timestamp_writes: None,
                });
            pass.set_pipeline(inject_pl);
            pass.set_bind_group(0, &inject_bg, &[view_offset.offset]);
            let size = view_target.main_texture().size();
            pass.dispatch_workgroups((size.width + 7) / 8, (size.height + 7) / 8, 1);
        }

        // ── Pass 3: resolve sums into the radiance texture ───────────
        if let Some(resolve_pl) = pipeline_cache.get_compute_pipeline(pipelines.resolve_pipeline) {
            let resolve_bg = render_context.render_device().create_bind_group(
                "voxel_resolve_bg",
                &pipeline_cache.get_bind_group_layout(&pipelines.resolve_layout),
                &BindGroupEntries::sequential((
                    resources.accum_buffer.as_entire_binding(),
                    &resources.radiance,
                )),
            );
            let mut pass = render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: Some("voxel_resolve"),
                    timestamp_writes: None,
                });
            pass.set_pipeline(resolve_pl);
            pass.set_bind_group(0, &resolve_bg, &[]);
            pass.dispatch_workgroups(VOXEL_RES / 4, VOXEL_RES / 4, VOXEL_RES / 4);
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct VoxelDebugNode;

impl ViewNode for VoxelDebugNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static ViewUniformOffset,
        &'static ViewPrepassTextures,
        &'static VoxelCacheView,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, view_offset, prepass, view): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        if !view.debug_active {
            return Ok(());
        }

        let pipelines = world.resource::<VoxelCachePipelines>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let view_uniforms = world.resource::<ViewUniforms>();

        let Some(resources) = world.get_resource::<VoxelCacheResources>() else {
            return Ok(());
        };
        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipelines.debug_pipeline) else {
            return Ok(());
        };
        let Some(view_binding) = view_uniforms.uniforms.binding() else {
            return Ok(());
        };
        let Some(depth_view) = prepass.depth_view() else { return Ok(()); };

        let post_process = view_target.post_process_write();
        let bind_group = render_context.render_device().create_bind_group(
            "voxel_debug_bg",
            &pipeline_cache.get_bind_group_layout(&pipelines.debug_layout),
            &BindGroupEntries::sequential((
                &resources.radiance,
                &pipelines.sampler,
                depth_view,
                post_process.source,
                view_binding.clone(),
                resources.uniform_buffer.as_entire_binding(),
            )),
        );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("voxel_debug_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &bind_group, &[view_offset.offset]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

#[derive(Default)]
pub struct VoxelCachePlugin;

impl Plugin for VoxelCachePlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "voxel_inject.wgsl");
        bevy::asset::embedded_asset!(app, "voxel_clear.wgsl");
        bevy::asset::embedded_asset!(app, "voxel_resolve.wgsl");
        bevy::asset::embedded_asset!(app, "voxel_debug.wgsl");

        app.add_systems(Update, sync_voxel_cache_views);
        app.add_plugins(ExtractComponentPlugin::<VoxelCacheView>::default());

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(
                    Render,
                    prepare_voxel_resources.in_set(RenderSystems::PrepareResources),
                )
                .add_render_graph_node::<ViewNodeRunner<VoxelInjectNode>>(Core3d, VoxelInjectLabel)
                .add_render_graph_node::<ViewNodeRunner<VoxelDebugNode>>(Core3d, VoxelDebugLabel)
                // Inject runs after EndMainPass so we have a lit scene
                // texture to read from. Debug runs after Tonemapping so
                // it bypasses tonemap/AE just like the normals/depth viz.
                .add_render_graph_edges(
                    Core3d,
                    (Node3d::EndMainPass, VoxelInjectLabel, Node3d::Tonemapping),
                )
                .add_render_graph_edges(
                    Core3d,
                    (
                        Node3d::Tonemapping,
                        VoxelDebugLabel,
                        Node3d::EndMainPassPostProcessing,
                    ),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<VoxelCachePipelines>();
        }
    }
}

