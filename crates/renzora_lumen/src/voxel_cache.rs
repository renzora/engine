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

use bevy::core_pipeline::Core3d;
use bevy::core_pipeline::prepass::ViewPrepassTextures;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_resource::binding_types::{
    sampler, storage_buffer_sized, texture_2d, texture_depth_2d, texture_storage_3d, uniform_buffer,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::view::{ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms};
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::utils::default;
use bytemuck::{Pod, Zeroable};

use renzora::{LumenDebug, LumenLighting, LumenQuality};

pub const VOXEL_RES: u32 = 64;
pub const VOXEL_SIZE_BASE: f32 = 0.5;
/// Number of clipmap cascades stacked in the voxel texture along Z.
/// Cascade N has voxel size `VOXEL_SIZE_BASE * 2^N`, giving extent
/// `VOXEL_RES * size` (32m / 64m / 128m / 256m for cascades 0-3).
/// Cone trace tests cascades inner-out and uses the first one
/// containing the sampled point, so smaller voxels win where they
/// overlap. Four cascades = ~128m radius of reach around the camera,
/// enough to fill mid-distance buildings without sky fallback. Memory
/// cost: 64³ × 4 × 8B (radiance Rgba16F) ≈ 8MB + 20MB accum.
pub const CASCADE_COUNT: u32 = 4;
/// Voxel cache mip levels. Mip 0 is the inject/resolve target (1×
/// base voxel size). Mips 1..MIP_COUNT are downsampled box-filter
/// averages — 2³ source voxels per destination voxel. The cone
/// trace picks a mip per step proportional to cone diameter at that
/// step, so a widening cone naturally samples coarser pre-averaged
/// content instead of doing many sharp taps. Replaces the previous
/// `DISTANCE_FALLOFF` hack with physically meaningful averaging.
///
/// Cascade stack-along-Z layout stays compatible: each cascade has
/// 64 voxels in Z at mip 0, 32 at mip 1, 16 at mip 2, 8 at mip 3 —
/// powers of two so the 2× boxfilter never reads across a cascade
/// boundary (`mip[N].cascade[C]` covers Z = [C*(64/2^N), (C+1)*(64/2^N))
/// and the source mip's `2k, 2k+1` neighbours stay within that range).
pub const VOXEL_MIP_COUNT: u32 = 4;
/// Total Z extent of the mega-texture: cascades stacked along Z.
pub const VOXEL_RES_Z: u32 = VOXEL_RES * CASCADE_COUNT;

/// Per-camera component carried into the render world by extraction.
/// Drives whether the voxel inject + debug passes execute for this view.
#[derive(Component, Clone, Copy, Debug)]
pub struct VoxelCacheView {
    pub inject_active: bool,
    pub debug_active: bool,
}

// Bevy 0.19: ExtractComponent requires SyncComponent.
impl bevy::render::sync_component::SyncComponent for VoxelCacheView {
    type Target = Self;
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
    cameras: Query<
        (
            Entity,
            Option<&LumenLighting>,
            Option<&renzora::core::ViewportCamera>,
        ),
        With<Camera3d>,
    >,
    viewports: Option<Res<renzora::core::viewport_types::Viewports>>,
) {
    for (entity, settings, viewport) in &cameras {
        // Editor viewport cameras only feed the voxel cache while their panel
        // is actually visible. The slot-0 camera stays active even when its
        // panel isn't docked (it hosts the atmosphere/IBL probe and only gets
        // a token-size target — see renzora_viewport); without this gate it
        // kept the full resolution-independent voxel pipeline (clear + inject
        // + downsample + per-frame CPU sample upload) running behind
        // viewport-less workspaces. Non-viewport cameras (game runtime) have
        // no `ViewportCamera` and are unaffected.
        let visible = match (viewport, viewports.as_ref()) {
            (Some(vc), Some(vp)) => vp.slots.get(vc.0).is_some_and(|s| s.docked),
            _ => true,
        };
        let (inject_active, debug_active) = match settings {
            Some(s) if visible => {
                let inject = matches!(
                    s.quality,
                    LumenQuality::SdfLow | LumenQuality::SdfHigh | LumenQuality::Hwrt
                );
                let debug = matches!(s.debug, LumenDebug::VoxelCache);
                (inject, debug)
            }
            _ => (false, false),
        };
        // `try_insert` (not `insert`): this command is applied at end-of-
        // schedule, and a camera can be despawned in between — e.g. opening a
        // document/asset tab tears down the scene camera. `insert` would panic
        // on the dead entity; `try_insert` safely no-ops.
        commands.entity(entity).try_insert(VoxelCacheView {
            inject_active,
            debug_active,
        });
    }
}

/// Per-cascade origin + voxel size. Padded to 16-byte alignment so
/// the WGSL `array<CascadeData, N>` layout matches the host.
#[derive(Clone, Copy, Debug, Pod, Zeroable, ShaderType)]
#[repr(C)]
pub struct VoxelCascadeData {
    pub origin: Vec3,
    pub voxel_size: f32,
}

/// World-space cascade origins + per-cascade voxel sizes for the
/// current frame's clipmap. Array size must match `CASCADE_COUNT` and
/// the matching WGSL `array<CascadeData, N>` in every shader file.
#[derive(Clone, Copy, Debug, Pod, Zeroable, ShaderType)]
#[repr(C)]
pub struct VoxelGridUniform {
    pub cascades: [VoxelCascadeData; 4],
    pub resolution: u32,
    pub cascade_count: u32,
    pub _pad0: u32,
    pub _pad1: u32,
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
    /// Persistent voxel radiance pyramid. Mip 0 is the inject/resolve
    /// destination; mips 1..VOXEL_MIP_COUNT are generated each frame
    /// by `voxel_downsample`. Standard mip view (covers all levels),
    /// what `lumen_trace` binds and samples per-step with `textureLoad(..., mip)`.
    pub radiance: TextureView,
    /// Per-mip single-level views — `voxel_downsample` binds these as
    /// storage for the destination mip. Index 0 is the mip 0 view
    /// (used by inject/resolve as the storage write target).
    pub radiance_mip_views: Vec<TextureView>,
    /// Per-frame uniform with the snapped origin.
    pub uniform_buffer: Buffer,
    /// Per-frame accumulation buffer: 5 u32s per voxel. Layout:
    ///   [0..3] sum_r, sum_g, sum_b (fixed-point ×256)
    ///   [3]    total_count (visible-surface + geometry inject)
    ///   [4]    geom_count  (geometry inject only — feeds occupancy)
    /// Cleared before each frame's inject pipeline, drained by resolve.
    pub accum_buffer: Buffer,
}

impl FromWorld for VoxelCachePipelines {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let asset_server = world.resource::<AssetServer>();

        // Accumulation buffer is 64³ × 5 u32s = 5 MB.
        //   [0..3] sum_r, sum_g, sum_b (fixed-point ×256)
        //   [3]    total_count (visible-surface + geometry inject)
        //   [4]    geom_count  (geometry inject only — feeds occupancy)
        let accum_size = (VOXEL_RES * VOXEL_RES * VOXEL_RES * CASCADE_COUNT * 5 * 4) as u64;
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
            immediate_size: 0,
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
            immediate_size: 0,
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
            immediate_size: 0,
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
                    format: TextureFormat::Rgba16Float, // 0.19: TEXTURE_FORMAT_HDR deprecated
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            immediate_size: 0,
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

    // Each cascade snaps its origin to its own voxel grid (size 2^N ×
    // base). The outer cascade has coarser snapping, so it shifts less
    // often as the camera moves; the inner cascade tracks the camera
    // more closely with finer voxel detail. Both centered on the camera.
    let mut cascades = [VoxelCascadeData {
        origin: Vec3::ZERO,
        voxel_size: 0.0,
    }; 4];
    for (c, cascade) in cascades.iter_mut().enumerate().take(CASCADE_COUNT as usize) {
        let voxel_size = VOXEL_SIZE_BASE * (1u32 << c as u32) as f32;
        let snapped_center = (camera_pos / voxel_size).floor() * voxel_size;
        let half_extent = (VOXEL_RES as f32 * voxel_size) * 0.5;
        *cascade = VoxelCascadeData {
            origin: snapped_center - Vec3::splat(half_extent),
            voxel_size,
        };
    }

    let uniform = VoxelGridUniform {
        cascades,
        resolution: VOXEL_RES,
        cascade_count: CASCADE_COUNT,
        _pad0: 0,
        _pad1: 0,
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
            // Cascades stacked along Z. Cascade C occupies
            // Z range [C * VOXEL_RES, (C+1) * VOXEL_RES).
            depth_or_array_layers: VOXEL_RES_Z,
        },
        mip_level_count: VOXEL_MIP_COUNT,
        sample_count: 1,
        dimension: TextureDimension::D3,
        format: TextureFormat::Rgba16Float,
        usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    // View covering all mips — what `lumen_trace`'s `voxels` binding
    // gets. WGSL `textureLoad(voxels, coords, mip)` picks per-step.
    let radiance = texture.create_view(&TextureViewDescriptor {
        label: Some("voxel_radiance_all_mips"),
        base_mip_level: 0,
        mip_level_count: Some(VOXEL_MIP_COUNT),
        ..default()
    });

    // Single-level views — `voxel_downsample` binds these as the
    // write storage target for each mip. Inject + resolve also use
    // mip 0 (index 0) as their write target.
    let radiance_mip_views: Vec<TextureView> = (0..VOXEL_MIP_COUNT)
        .map(|mip| {
            texture.create_view(&TextureViewDescriptor {
                label: Some("voxel_radiance_mip"),
                base_mip_level: mip,
                mip_level_count: Some(1),
                ..default()
            })
        })
        .collect();

    let uniform_buffer = render_device.create_buffer(&BufferDescriptor {
        label: Some("voxel_grid_uniform"),
        size: std::mem::size_of::<VoxelGridUniform>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    render_queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&uniform));

    let accum_size = (VOXEL_RES * VOXEL_RES * VOXEL_RES * CASCADE_COUNT * 5 * 4) as u64;
    let accum_buffer = render_device.create_buffer(&BufferDescriptor {
        label: Some("voxel_accum"),
        size: accum_size,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    commands.insert_resource(VoxelCacheResources {
        radiance,
        radiance_mip_views,
        uniform_buffer,
        accum_buffer,
    });
}

/// Clear pass — zeros the per-frame accumulation buffer. Split from
/// the visible-surface inject so other writers (e.g. geometry
/// voxelization) can land between clear and resolve. Bevy 0.19: was
/// `VoxelClearNode: ViewNode`; now a render system (see `LumenSystems`).
pub fn voxel_clear_pass(
    world: &World,
    view: ViewQuery<&'static VoxelCacheView>,
    mut render_context: RenderContext,
) {
    let view = view.into_inner();
    {
        if !view.inject_active {
            return;
        }
        let _span = info_span!("voxel.clear").entered();
        let pipelines = world.resource::<VoxelCachePipelines>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(resources) = world.get_resource::<VoxelCacheResources>() else {
            return;
        };
        let Some(clear_pl) = pipeline_cache.get_compute_pipeline(pipelines.clear_pipeline) else {
            return;
        };
        let bg = render_context.render_device().create_bind_group(
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
        pass.set_bind_group(0, &bg, &[]);
        // Divisor must match `voxel_clear.wgsl` workgroup_size(256,1,1).
        // ceil-div handles the (currently exact) edge case where future
        // resolution/cascade tweaks make the entry count non-multiple.
        let entries = VOXEL_RES * VOXEL_RES * VOXEL_RES * CASCADE_COUNT * 5;
        let groups = entries.div_ceil(256);
        pass.dispatch_workgroups(groups, 1, 1);
    }
}

/// Resolve pass — drains the accumulation buffer into the persistent
/// radiance texture as sum/count averages with temporal blending.
/// Runs AFTER both the visible-surface inject and the geometry inject
/// so all contributions for the frame are baked together.
pub fn voxel_resolve_pass(
    world: &World,
    view: ViewQuery<&'static VoxelCacheView>,
    mut render_context: RenderContext,
) {
    let view = view.into_inner();
    {
        if !view.inject_active {
            return;
        }
        let _span = info_span!("voxel.resolve").entered();
        let pipelines = world.resource::<VoxelCachePipelines>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(resources) = world.get_resource::<VoxelCacheResources>() else {
            return;
        };
        let Some(resolve_pl) = pipeline_cache.get_compute_pipeline(pipelines.resolve_pipeline)
        else {
            return;
        };
        let bg = render_context.render_device().create_bind_group(
            "voxel_resolve_bg",
            &pipeline_cache.get_bind_group_layout(&pipelines.resolve_layout),
            &BindGroupEntries::sequential((
                resources.accum_buffer.as_entire_binding(),
                // Mip 0 — resolve writes the freshly-accumulated
                // radiance into the pyramid's base level. The
                // downsample pass then builds mips 1..N from this.
                &resources.radiance_mip_views[0],
            )),
        );
        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {
                label: Some("voxel_resolve"),
                timestamp_writes: None,
            });
        pass.set_pipeline(resolve_pl);
        pass.set_bind_group(0, &bg, &[]);
        // Z dimension covers all cascades (each `VOXEL_RES` deep, stacked).
        pass.dispatch_workgroups(VOXEL_RES / 4, VOXEL_RES / 4, VOXEL_RES_Z / 4);
    }
}

pub fn voxel_inject_pass(
    world: &World,
    view: ViewQuery<(
        &'static ViewTarget,
        &'static ViewUniformOffset,
        &'static ViewPrepassTextures,
        &'static VoxelCacheView,
    )>,
    mut render_context: RenderContext,
) {
    let (view_target, view_offset, prepass, view) = view.into_inner();
    {
        if !view.inject_active {
            return;
        }
        let _span = info_span!("voxel.inject").entered();

        let pipelines = world.resource::<VoxelCachePipelines>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let view_uniforms = world.resource::<ViewUniforms>();

        let Some(resources) = world.get_resource::<VoxelCacheResources>() else {
            return;
        };
        let Some(view_binding) = view_uniforms.uniforms.binding() else {
            return;
        };
        let Some(depth_view) = prepass.depth_view() else { return; };

        // Visible-surface inject only — clear runs before us, resolve
        // runs after us (and after the geometry inject between).
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
            pass.dispatch_workgroups(size.width.div_ceil(8), size.height.div_ceil(8), 1);
        }
    }
}

pub fn voxel_debug_pass(
    world: &World,
    view: ViewQuery<(
        &'static ViewTarget,
        &'static ViewUniformOffset,
        &'static ViewPrepassTextures,
        &'static VoxelCacheView,
    )>,
    mut render_context: RenderContext,
) {
    let (view_target, view_offset, prepass, view) = view.into_inner();
    {
        if !view.debug_active {
            return;
        }

        let pipelines = world.resource::<VoxelCachePipelines>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let view_uniforms = world.resource::<ViewUniforms>();

        let Some(resources) = world.get_resource::<VoxelCacheResources>() else {
            return;
        };
        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipelines.debug_pipeline) else {
            return;
        };
        let Some(view_binding) = view_uniforms.uniforms.binding() else {
            return;
        };
        let Some(depth_view) = prepass.depth_view() else { return; };

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
            multiview_mask: None,
        });
        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &bind_group, &[view_offset.offset]);
        render_pass.draw(0..3, 0..1);
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
            // Bevy 0.19: render graph → systems. Ordering (Clear → Inject →
            // [GeometryInject] → Resolve, then Debug after tonemapping) is
            // expressed via the shared `LumenSystems` sets in `lib.rs`.
            render_app
                .add_systems(
                    Render,
                    prepare_voxel_resources.in_set(RenderSystems::PrepareResources),
                )
                .add_systems(
                    Core3d,
                    (
                        voxel_clear_pass.in_set(crate::LumenSystems::VoxelClear),
                        voxel_inject_pass.in_set(crate::LumenSystems::VoxelInject),
                        voxel_resolve_pass.in_set(crate::LumenSystems::VoxelResolve),
                        voxel_debug_pass.in_set(crate::LumenSystems::VoxelDebug),
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

