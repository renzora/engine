//! Per-view state: the visibility buffer, the material-depth buffer, the list
//! of clusters that survived culling and the two indirect draws that raster
//! them.

use std::collections::HashMap;

use bevy::camera::Camera3d;
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::{CachedTexture, TextureCache};
use bevy::render::view::{Msaa, ViewUniforms};

use super::pipelines::{CullParams, VgPipelines, TRANSPARENT_VG_BASE};
use super::scene::{layer_words, GpuScene};
use bevy::camera::visibility::RenderLayers;
use crate::attach::VirtualGeometrySettings;

/// The vertex count of one cluster draw: 128 triangles, whether or not the
/// cluster has that many.
const CLUSTER_VERTICES: u32 = 128 * 3;

/// Spacing of the per-bucket uniforms: the alignment every device accepts
/// for a dynamic uniform offset.
pub(crate) const BUCKET_STRIDE: u32 = 256;

#[derive(Component)]
pub(crate) struct VgView {
    pub visibility: CachedTexture,
    pub material_depth: CachedTexture,
    pub draws: Buffer,
    /// Indirect draws for the masked buckets, 16 bytes each.
    pub masked_draws: Buffer,
    /// Each bucket's first slot, at [`BUCKET_STRIDE`] intervals, bound with
    /// a dynamic offset per draw.
    pub buckets: Buffer,
    /// Workgroups for the cull dispatch, split over two dimensions once a
    /// scene has more than 65535 × 64 cluster pairs.
    pub dispatch: (u32, u32),
    /// How many masked buckets, and buckets in all, `masked_draws` and
    /// `buckets` hold. A draw past them is skipped rather than sent: a
    /// dynamic offset past the end of a buffer loses the GPU device.
    pub masked_count: u32,
    pub bucket_count: u32,
}

#[derive(Component)]
pub(crate) struct VgViewBindGroups {
    pub cull: BindGroup,
    pub raster_view: BindGroup,
    pub raster_scene: BindGroup,
    pub material_depth: BindGroup,
    pub shade: BindGroup,
    pub masked_scene: BindGroup,
    pub buckets: BindGroup,
    /// Group 2 of the transparent draws: Bevy's `Mesh` uniforms, the scene,
    /// and the bucket (with a dynamic offset per draw).
    pub transparent: BindGroup,
}

/// The buffers a view keeps from frame to frame, so the visible list is not
/// reallocated every frame.
struct ViewBuffers {
    visible: Buffer,
    capacity: u32,
    masked_room: u32,
    draws: Buffer,
    params: Buffer,
}

#[derive(Resource, Default)]
pub(crate) struct VgViewBuffers(HashMap<Entity, ViewBuffers>);

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    mut commands: Commands,
    gpu: Res<GpuScene>,
    settings: Option<Res<VirtualGeometrySettingsRender>>,
    mut buffers: ResMut<VgViewBuffers>,
    mut textures: ResMut<TextureCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    views: Query<(Entity, &ExtractedCamera, &Msaa, Option<&RenderLayers>), With<Camera3d>>,
) {
    let lod_threshold = settings.map_or(1.0, |s| s.lod_error_pixels);
    let mut seen = Vec::new();
    for (entity, camera, msaa, layers) in &views {
        // The visibility buffer holds one sample per pixel. Every editor and
        // game camera runs without MSAA; one that does not gets ordinary
        // rendering only, and converted meshes are not drawn in it.
        let size = camera.physical_target_size.filter(|s| s.x > 0 && s.y > 0);
        let (true, Some(size)) = (gpu.ready() && *msaa == Msaa::Off, size) else {
            commands.entity(entity).remove::<(VgView, VgViewBindGroups)>();
            continue;
        };
        seen.push(entity);

        let extent = Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        };
        let mut texture = |label: &'static str, format: TextureFormat, usage: TextureUsages| {
            textures.get(
                &device,
                TextureDescriptor {
                    label: Some(label),
                    size: extent,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                },
            )
        };
        let visibility = texture(
            "vg_visibility",
            TextureFormat::R32Uint,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );
        let material_depth = texture(
            "vg_material_depth",
            TextureFormat::Depth16Unorm,
            TextureUsages::RENDER_ATTACHMENT,
        );

        // Every pair could survive, so the list holds them all, then a region
        // for the masked buckets past it. Rounded up so a few models streaming
        // in do not reallocate it each frame.
        let capacity = gpu.pair_count.next_power_of_two().max(1024);
        let masked_room = if gpu.masked_total == 0 { 0 } else { gpu.masked_total.next_power_of_two() };
        let visible_size = (capacity as u64 + masked_room as u64) * 8;
        let entry = buffers.0.entry(entity).or_insert_with(|| ViewBuffers {
            visible: device.create_buffer(&BufferDescriptor {
                label: Some("vg_visible"),
                size: visible_size,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: false,
            }),
            capacity,
            masked_room,
            draws: device.create_buffer(&BufferDescriptor {
                label: Some("vg_draws"),
                size: 32,
                usage: BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            params: device.create_buffer(&BufferDescriptor {
                label: Some("vg_cull_params"),
                size: std::mem::size_of::<CullParams>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        });
        if entry.capacity < capacity || entry.masked_room < masked_room {
            entry.capacity = entry.capacity.max(capacity);
            entry.masked_room = entry.masked_room.max(masked_room);
            entry.visible = device.create_buffer(&BufferDescriptor {
                label: Some("vg_visible"),
                size: (entry.capacity as u64 + entry.masked_room as u64) * 8,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: false,
            });
        }

        // One indirect draw per masked bucket, and where its slots start.
        // Rebuilt each frame: a handful of buckets, and their number changes
        // whenever a model with a new masked material appears.
        let buckets = gpu.bucket_count();
        let masked_init: Vec<u32> = std::iter::repeat_n([CLUSTER_VERTICES, 0, 0, 0], buckets)
            .flatten()
            .chain(std::iter::repeat_n(0, if buckets == 0 { 4 } else { 0 }))
            .collect();
        let masked_draws = device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("vg_masked_draws"),
            contents: bytemuck::cast_slice(&masked_init),
            usage: BufferUsages::STORAGE | BufferUsages::INDIRECT,
        });
        // Per bucket: its first absolute slot, then (transparent ones only)
        // its entry in the mesh uniforms.
        let mut bucket_bases = vec![0u8; buckets.max(1) * BUCKET_STRIDE as usize];
        let mesh_indices = std::iter::repeat_n(0, gpu.masked.len()).chain(gpu.transparent.iter().map(|t| t.mesh_index));
        for (i, (base, mesh_index)) in gpu.bucket_bases().zip(mesh_indices).enumerate() {
            let at = i * BUCKET_STRIDE as usize;
            bucket_bases[at..at + 4].copy_from_slice(&(entry.capacity + base).to_le_bytes());
            bucket_bases[at + 4..at + 8].copy_from_slice(&mesh_index.to_le_bytes());
        }
        let buckets = device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("vg_masked_buckets"),
            contents: &bucket_bases,
            usage: BufferUsages::UNIFORM,
        });
        // Written before the frame's commands run, so each frame's cull starts
        // from empty draws.
        let draws: [u32; 8] = [CLUSTER_VERTICES, 0, 0, 0, CLUSTER_VERTICES, 0, 0, 0];
        queue.write_buffer(&entry.draws, 0, bytemuck::cast_slice(&draws));
        let params = CullParams {
            pair_count: gpu.pair_count,
            instance_count: gpu.instance_count,
            visible_capacity: entry.capacity,
            lod_threshold,
            view_layers: layer_words(layers),
            masked_offset: entry.capacity,
            _pad: 0,
        };
        queue.write_buffer(&entry.params, 0, bytemuck::bytes_of(&params));

        let groups = gpu.pair_count.div_ceil(64);
        let x = groups.min(65535);
        let y = groups.div_ceil(x.max(1));
        commands.entity(entity).insert(VgView {
            visibility,
            material_depth,
            draws: entry.draws.clone(),
            masked_draws,
            buckets,
            dispatch: (x, y),
            masked_count: gpu.masked.len() as u32,
            bucket_count: gpu.bucket_count() as u32,
        });
    }
    buffers.0.retain(|entity, _| seen.contains(entity));
}

pub(crate) fn prepare_bind_groups(
    mut commands: Commands,
    gpu: Res<GpuScene>,
    pipelines: Option<Res<VgPipelines>>,
    pipeline_cache: Res<PipelineCache>,
    buffers: Res<VgViewBuffers>,
    view_uniforms: Res<ViewUniforms>,
    device: Res<RenderDevice>,
    views: Query<(Entity, &VgView)>,
) {
    let (Some(pipelines), Some(view_binding)) = (pipelines, view_uniforms.uniforms.binding()) else {
        return;
    };
    let (
        Some(clusters),
        Some(indices),
        Some(positions),
        Some(attributes),
        Some(instances),
        Some(masked_bases),
        Some(skinned),
        Some(mesh_uniforms),
        Some(residency),
        Some(requests),
        Some(drawn),
    ) = (
        gpu.clusters.as_ref(),
        gpu.indices.as_ref(),
        gpu.positions.as_ref(),
        gpu.attributes.as_ref(),
        gpu.instances.as_ref(),
        gpu.masked_bases.as_ref(),
        gpu.skinned.as_ref(),
        gpu.mesh_uniforms.as_ref(),
        gpu.streaming.residency.as_ref(),
        gpu.streaming.requests.as_ref(),
        gpu.streaming.drawn.as_ref(),
    )
    else {
        return;
    };
    for (entity, view) in &views {
        let Some(own) = buffers.0.get(&entity) else {
            continue;
        };
        let layout = |d: &BindGroupLayoutDescriptor| pipeline_cache.get_bind_group_layout(d);
        let cull = device.create_bind_group(
            "vg_cull",
            &layout(&pipelines.cull_layout),
            &BindGroupEntries::sequential((
                view_binding.clone(),
                own.params.as_entire_binding(),
                instances.as_entire_binding(),
                clusters.as_entire_binding(),
                own.visible.as_entire_binding(),
                own.draws.as_entire_binding(),
                masked_bases.as_entire_binding(),
                view.masked_draws.as_entire_binding(),
                residency.as_entire_binding(),
                requests.as_entire_binding(),
                drawn.as_entire_binding(),
            )),
        );
        let masked_scene = device.create_bind_group(
            "vg_masked_scene",
            &layout(&pipelines.masked_scene_layout),
            &BindGroupEntries::with_indices((
                (1, own.visible.as_entire_binding()),
                (2, instances.as_entire_binding()),
                (3, clusters.as_entire_binding()),
                (4, indices.as_entire_binding()),
                (5, positions.as_entire_binding()),
                (6, attributes.as_entire_binding()),
                (7, skinned.as_entire_binding()),
            )),
        );
        let buckets = device.create_bind_group(
            "vg_masked_buckets",
            &layout(&pipelines.bucket_layout),
            &BindGroupEntries::single(BufferBinding {
                buffer: &view.buckets,
                offset: 0,
                size: std::num::NonZeroU64::new(4),
            }),
        );
        let raster_view = device.create_bind_group(
            "vg_raster_view",
            &layout(&pipelines.raster_view_layout),
            &BindGroupEntries::sequential((view_binding.clone(), own.params.as_entire_binding())),
        );
        let raster_scene = device.create_bind_group(
            "vg_raster_scene",
            &layout(&pipelines.raster_scene_layout),
            &BindGroupEntries::with_indices((
                (1, own.visible.as_entire_binding()),
                (2, instances.as_entire_binding()),
                (3, clusters.as_entire_binding()),
                (4, indices.as_entire_binding()),
                (5, positions.as_entire_binding()),
                (7, skinned.as_entire_binding()),
            )),
        );
        let material_depth = device.create_bind_group(
            "vg_material_depth",
            &layout(&pipelines.material_depth_layout),
            &BindGroupEntries::sequential((
                &view.visibility.default_view,
                own.visible.as_entire_binding(),
                instances.as_entire_binding(),
            )),
        );
        let shade = device.create_bind_group(
            "vg_shade",
            &layout(&pipelines.shade_layout),
            &BindGroupEntries::sequential((
                &view.visibility.default_view,
                own.visible.as_entire_binding(),
                instances.as_entire_binding(),
                clusters.as_entire_binding(),
                indices.as_entire_binding(),
                positions.as_entire_binding(),
                attributes.as_entire_binding(),
                skinned.as_entire_binding(),
            )),
        );
        let base = TRANSPARENT_VG_BASE;
        let transparent = device.create_bind_group(
            "vg_transparent",
            &layout(&pipelines.transparent_layout),
            &BindGroupEntries::with_indices((
                (0, mesh_uniforms.as_entire_binding()),
                (base + 1, own.visible.as_entire_binding()),
                (base + 2, instances.as_entire_binding()),
                (base + 3, clusters.as_entire_binding()),
                (base + 4, indices.as_entire_binding()),
                (base + 5, positions.as_entire_binding()),
                (base + 6, attributes.as_entire_binding()),
                (base + 7, skinned.as_entire_binding()),
                (
                    base + 8,
                    BufferBinding {
                        buffer: &view.buckets,
                        offset: 0,
                        size: std::num::NonZeroU64::new(8),
                    },
                ),
            )),
        );
        commands.entity(entity).insert(VgViewBindGroups {
            transparent,
            cull,
            raster_view,
            raster_scene,
            material_depth,
            shade,
            masked_scene,
            buckets,
        });
    }
}

/// The parts of [`VirtualGeometrySettings`] the render world reads.
#[derive(Resource, Clone)]
pub(crate) struct VirtualGeometrySettingsRender {
    pub lod_error_pixels: f32,
    pub budget_bytes: u64,
}

/// While a ray tracer traces the full detail ([`renzora::TracedGeometryWanted`]),
/// virtual geometry draws its finest clusters too, whatever the project's LOD
/// threshold. A simplified level stays within a pixel of the full detail, but
/// that is not close enough for light: the tracer starts its rays from the
/// drawn surface and meets the traced one, and where the two parted by a
/// fraction of a pixel, curtain folds and grazing cabinet edges went black in
/// patches or let light through gaps. The finest clusters are the very
/// triangles `full_detail_mesh` builds the traced surface from, so the two
/// agree exactly. Clusters still stream and cull per view, so this costs
/// triangles drawn, never memory beyond what tracing already holds.
pub(crate) fn extract_settings(
    mut commands: Commands,
    settings: bevy::render::Extract<Option<Res<VirtualGeometrySettings>>>,
    traced: bevy::render::Extract<Option<Res<renzora::TracedGeometryWanted>>>,
) {
    if let Some(settings) = settings.as_ref() {
        commands.insert_resource(VirtualGeometrySettingsRender {
            lod_error_pixels: if traced.is_some() { 0.0 } else { settings.lod_error_pixels },
            budget_bytes: u64::from(settings.budget_mb) << 20,
        });
    }
}
