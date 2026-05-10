//! Geometry voxelization — Phase 2 follow-up.
//!
//! Bakes a sparse set of world-space sample points per mesh, then each
//! frame injects those samples (multiplied by their material's base
//! color) into the voxel radiance accumulation buffer alongside the
//! visible-surface inject. Result: voxels carry geometry data even for
//! surfaces never visible from the camera, which is what Phase 5 needs
//! to ray-trace through the cache.
//!
//! V1 limitations (deferred to follow-ups):
//!   - StandardMaterial.base_color only; no albedo texture sampling.
//!   - Static meshes only; skinned/morphed meshes ignore deformation.
//!   - CPU samples re-flattened into a fresh GPU buffer every frame.
//!     Fine up to ~1M total samples; needs per-mesh persistent buffers
//!     + indirect dispatch for serious scenes.
//!   - No occlusion bit yet — voxels just get color contributions, so
//!     inside-mesh "solid air" isn't represented. Phase 5's ray tracer
//!     will need an additional alpha/density signal.

use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::mesh::{Indices, Mesh, VertexAttributeValues};
use bevy::render::render_graph::{
    NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
};
use bevy::render::render_resource::binding_types::{storage_buffer_read_only_sized, storage_buffer_sized, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::Extract;
use bevy::render::{Render, RenderApp, RenderSystems};
use bytemuck::{Pod, Zeroable};

use crate::voxel_cache::{VoxelCacheResources, VoxelCacheView, VoxelGridUniform, VOXEL_RES};

/// Approximate spacing between sample points along a triangle's
/// surface, in world units. Larger spacing = less CPU/GPU work for
/// city-scale scenes. 1.5m means each voxel (0.5m) gets at most ~1
/// sample per triangle face passing through it; voxel coverage relies
/// on the fact that most surfaces have multiple triangles per voxel.
const SAMPLE_SPACING: f32 = 1.5;

/// Hard cap on samples per mesh so a particularly dense mesh doesn't
/// blow out the per-frame buffer.
const MAX_SAMPLES_PER_MESH: usize = 512;

/// Beyond this distance from the camera, entities are skipped at
/// extract time. The voxel grid extends 16m from camera so 24m gives
/// a generous skirt for big meshes whose origin is far from their
/// actual geometry (e.g. terrain chunks).
const CULL_RADIUS: f32 = 24.0;

/// Cap on total samples uploaded per frame across all entities. Each
/// sample is 32 bytes; 100k samples = 3.2 MB/frame upload, plenty for
/// the cache resolution.
const MAX_SAMPLES_PER_FRAME: usize = 100_000;

/// Per-mesh-instance baked sample list. Lives on the entity (not the
/// asset) so per-instance albedo overrides work cleanly. Re-baked when
/// the mesh asset or material handle changes.
#[derive(Component, Clone, Default)]
pub struct MeshVoxelSamples {
    pub local_positions: Vec<Vec3>,
    pub albedo: LinearRgba,
}

/// Bakes voxel samples for `Mesh3d` entities backed by either:
///   - `MeshMaterial3d<StandardMaterial>` directly, or
///   - `MeshMaterial3d<GraphMaterial>` (renzora_shader's node-graph
///     wrapper, which is `ExtendedMaterial<StandardMaterial, ...>`).
///
/// Bake runs when samples are missing or the mesh/material handle
/// changes. We pull `base_color` from the underlying `StandardMaterial`
/// — for the graph wrapper that's `.base.base_color` since the wrapper
/// composes albedo procedurally and a single representative is enough
/// for V1.
fn bake_mesh_samples(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    standard_materials: Res<Assets<StandardMaterial>>,
    graph_materials: Res<Assets<renzora_shader::material::GraphMaterial>>,
    standard_query: Query<
        (
            Entity,
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
            Option<&MeshVoxelSamples>,
        ),
        Or<(Without<MeshVoxelSamples>, Changed<Mesh3d>, Changed<MeshMaterial3d<StandardMaterial>>)>,
    >,
    graph_query: Query<
        (
            Entity,
            &Mesh3d,
            &MeshMaterial3d<renzora_shader::material::GraphMaterial>,
            Option<&MeshVoxelSamples>,
        ),
        Or<(
            Without<MeshVoxelSamples>,
            Changed<Mesh3d>,
            Changed<MeshMaterial3d<renzora_shader::material::GraphMaterial>>,
        )>,
    >,
) {
    for (entity, mesh_handle, mat_handle, existing) in &standard_query {
        let Some(mesh) = meshes.get(&mesh_handle.0) else { continue; };
        let albedo = standard_materials
            .get(&mat_handle.0)
            .map(|m| m.base_color.to_linear())
            .unwrap_or(LinearRgba::WHITE);
        bake_one(&mut commands, entity, mesh, albedo, existing);
    }

    for (entity, mesh_handle, mat_handle, existing) in &graph_query {
        let Some(mesh) = meshes.get(&mesh_handle.0) else { continue; };
        let albedo = graph_materials
            .get(&mat_handle.0)
            .map(|m| m.base.base_color.to_linear())
            .unwrap_or(LinearRgba::WHITE);
        bake_one(&mut commands, entity, mesh, albedo, existing);
    }
}

fn bake_one(
    commands: &mut Commands,
    entity: Entity,
    mesh: &Mesh,
    albedo: LinearRgba,
    existing: Option<&MeshVoxelSamples>,
) {
    let local_positions = sample_mesh_surface(mesh, SAMPLE_SPACING);
    if local_positions.is_empty() && existing.is_none() {
        return;
    }
    commands.entity(entity).insert(MeshVoxelSamples {
        local_positions,
        albedo,
    });
}

/// Generate sample points across the mesh's surface, spaced roughly
/// `spacing` apart. Stratified per-triangle: each triangle gets a
/// number of samples proportional to its area, with deterministic
/// pseudo-random barycentric offsets so the bake is reproducible.
fn sample_mesh_surface(mesh: &Mesh, spacing: f32) -> Vec<Vec3> {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return Vec::new();
    };
    let Some(indices) = mesh.indices() else {
        // Unindexed meshes — just sample every triangle as 3 sequential
        // verts.
        return sample_unindexed(positions, spacing);
    };

    let mut samples = Vec::new();
    let mut tri_iter = match indices {
        Indices::U16(v) => Box::new(v.chunks_exact(3).map(|c| {
            (c[0] as usize, c[1] as usize, c[2] as usize)
        })) as Box<dyn Iterator<Item = (usize, usize, usize)>>,
        Indices::U32(v) => Box::new(v.chunks_exact(3).map(|c| {
            (c[0] as usize, c[1] as usize, c[2] as usize)
        })) as Box<dyn Iterator<Item = (usize, usize, usize)>>,
    };

    let voxel_area = spacing * spacing;
    while let Some((i0, i1, i2)) = tri_iter.next() {
        if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
            continue;
        }
        let p0 = Vec3::from(positions[i0]);
        let p1 = Vec3::from(positions[i1]);
        let p2 = Vec3::from(positions[i2]);
        let area = (p1 - p0).cross(p2 - p0).length() * 0.5;
        if area < 1e-8 {
            continue;
        }
        let n = ((area / voxel_area).ceil() as usize).clamp(1, 64);

        for sample_idx in 0..n {
            let p = barycentric_sample(p0, p1, p2, sample_idx as u32);
            samples.push(p);
            if samples.len() >= MAX_SAMPLES_PER_MESH {
                return samples;
            }
        }
    }
    samples
}

fn sample_unindexed(positions: &[[f32; 3]], spacing: f32) -> Vec<Vec3> {
    let mut samples = Vec::new();
    let voxel_area = spacing * spacing;
    for tri in positions.chunks_exact(3) {
        let p0 = Vec3::from(tri[0]);
        let p1 = Vec3::from(tri[1]);
        let p2 = Vec3::from(tri[2]);
        let area = (p1 - p0).cross(p2 - p0).length() * 0.5;
        if area < 1e-8 {
            continue;
        }
        let n = ((area / voxel_area).ceil() as usize).clamp(1, 64);
        for sample_idx in 0..n {
            let p = barycentric_sample(p0, p1, p2, sample_idx as u32);
            samples.push(p);
            if samples.len() >= MAX_SAMPLES_PER_MESH {
                return samples;
            }
        }
    }
    samples
}

fn barycentric_sample(p0: Vec3, p1: Vec3, p2: Vec3, seed: u32) -> Vec3 {
    let s = seed
        .wrapping_mul(2654435761)
        .wrapping_add(0x9E3779B9);
    let u = ((s ^ (s >> 16)) & 0xFFFF) as f32 / 65535.0;
    let v = ((s.wrapping_mul(1597334677) >> 8) & 0xFFFF) as f32 / 65535.0;
    let (mut u, mut v) = (u, v);
    if u + v > 1.0 {
        u = 1.0 - u;
        v = 1.0 - v;
    }
    let w = 1.0 - u - v;
    p0 * w + p1 * u + p2 * v
}

// ─── Render world ──────────────────────────────────────────────────

/// Flat per-frame GPU storage layout. Each entry is one sample:
/// (world_x, world_y, world_z, _pad, albedo_r, albedo_g, albedo_b, _pad).
#[derive(Clone, Copy, Pod, Zeroable, ShaderType)]
#[repr(C)]
pub struct GpuSample {
    pub world_pos: [f32; 4],
    pub albedo: [f32; 4],
}

#[derive(Resource, Default)]
pub struct GeometrySampleBuffer {
    pub buffer: Option<Buffer>,
    pub capacity_bytes: u64,
    pub count: u32,
}

#[derive(Resource)]
pub struct GeometryInjectPipeline {
    pub layout: BindGroupLayoutDescriptor,
    pub pipeline_id: CachedComputePipelineId,
}

impl FromWorld for GeometryInjectPipeline {
    fn from_world(world: &mut World) -> Self {
        // 4 MB matches the accum buffer in voxel_cache.
        let accum_size = (VOXEL_RES * VOXEL_RES * VOXEL_RES * 4 * 4) as u64;
        let accum_size_nz = std::num::NonZeroU64::new(accum_size).unwrap();

        let layout = BindGroupLayoutDescriptor::new(
            "voxel_geo_inject_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    // 0: sample list (read-only storage)
                    storage_buffer_read_only_sized(false, None),
                    // 1: voxel accumulation buffer (atomic adds)
                    storage_buffer_sized(false, Some(accum_size_nz)),
                    // 2: voxel grid uniform
                    uniform_buffer::<VoxelGridUniform>(false),
                ),
            ),
        );

        let asset_server = world.resource::<AssetServer>();
        let shader = asset_server.load("embedded://renzora_lumen/voxel_geo_inject.wgsl");
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline_id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("voxel_geo_inject_pipeline".into()),
            layout: vec![layout.clone()],
            shader,
            shader_defs: vec![],
            entry_point: Some("inject".into()),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        });

        Self { layout, pipeline_id }
    }
}

/// Extract — flatten all (transform × samples) into a single flat
/// vector each frame, transformed into world space. Then upload to a
/// (re)allocated GPU buffer. Per-frame CPU + transfer cost is roughly
/// O(total samples × 8 floats); ~1M samples = 32 MB/frame, well
/// within budget for V1.
pub fn extract_geometry_samples(
    mut commands: Commands,
    query: Extract<Query<(&MeshVoxelSamples, &GlobalTransform)>>,
    cameras: Extract<Query<&GlobalTransform, With<Camera3d>>>,
) {
    // Use the first 3D camera's position as the cull pivot. Voxel
    // grid follows whichever camera ends up active; even if multiple
    // cameras exist this is a reasonable approximation since the
    // grid is small.
    let camera_pos = cameras
        .iter()
        .next()
        .map(|t| t.translation())
        .unwrap_or(Vec3::ZERO);
    let cull_sq = CULL_RADIUS * CULL_RADIUS;

    // Preallocate so push doesn't reallocate. Worst case it's a bit
    // oversized; we shrink-to-fit only if memory matters (it doesn't).
    let mut samples: Vec<GpuSample> = Vec::with_capacity(MAX_SAMPLES_PER_FRAME);

    for (mesh_samples, transform) in query.iter() {
        if samples.len() >= MAX_SAMPLES_PER_FRAME {
            break;
        }
        if mesh_samples.local_positions.is_empty() {
            continue;
        }

        // Cheap cull: entity origin → camera distance. Misses big
        // meshes whose origin is far from their geometry, but for a
        // city of separated buildings it's a 10-100× win.
        let entity_pos = transform.translation();
        if entity_pos.distance_squared(camera_pos) > cull_sq {
            continue;
        }

        let albedo = [
            mesh_samples.albedo.red,
            mesh_samples.albedo.green,
            mesh_samples.albedo.blue,
            0.0,
        ];
        let model = transform.to_matrix();
        let budget = MAX_SAMPLES_PER_FRAME - samples.len();
        let count = mesh_samples.local_positions.len().min(budget);
        for &local in &mesh_samples.local_positions[..count] {
            let world = model.transform_point3(local);
            samples.push(GpuSample {
                world_pos: [world.x, world.y, world.z, 0.0],
                albedo,
            });
        }
    }
    commands.insert_resource(ExtractedGeometrySamples(samples));
}

#[derive(Resource)]
pub struct ExtractedGeometrySamples(pub Vec<GpuSample>);

pub fn prepare_geometry_sample_buffer(
    extracted: Option<Res<ExtractedGeometrySamples>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut buffer: ResMut<GeometrySampleBuffer>,
) {
    let Some(extracted) = extracted else {
        buffer.count = 0;
        return;
    };
    if extracted.0.is_empty() {
        buffer.count = 0;
        return;
    }

    let bytes = bytemuck::cast_slice(&extracted.0);
    let needed = bytes.len() as u64;

    // Reallocate if we don't have a buffer yet or the existing one is
    // too small. Round up to the next power-of-two MB so we don't
    // bounce on tiny size changes.
    let needs_alloc = match buffer.buffer.as_ref() {
        None => true,
        Some(_) => buffer.capacity_bytes < needed,
    };
    if needs_alloc {
        let cap = needed.next_power_of_two().max(1 << 20); // ≥ 1 MB
        buffer.buffer = Some(render_device.create_buffer(&BufferDescriptor {
            label: Some("voxel_geometry_samples"),
            size: cap,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        buffer.capacity_bytes = cap;
    }
    if let Some(buf) = buffer.buffer.as_ref() {
        render_queue.write_buffer(buf, 0, bytes);
    }
    buffer.count = extracted.0.len() as u32;
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct GeometryInjectLabel;

#[derive(Default)]
pub struct GeometryInjectNode;

impl ViewNode for GeometryInjectNode {
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
        let buffer = world.resource::<GeometrySampleBuffer>();
        if buffer.count == 0 {
            return Ok(());
        }
        let Some(sample_buf) = buffer.buffer.as_ref() else { return Ok(()); };

        let pipeline = world.resource::<GeometryInjectPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(compute) = pipeline_cache.get_compute_pipeline(pipeline.pipeline_id) else {
            return Ok(());
        };
        let Some(resources) = world.get_resource::<VoxelCacheResources>() else {
            return Ok(());
        };

        let bg = render_context.render_device().create_bind_group(
            "voxel_geo_inject_bg",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((
                sample_buf.as_entire_binding(),
                resources.accum_buffer.as_entire_binding(),
                resources.uniform_buffer.as_entire_binding(),
            )),
        );

        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {
                label: Some("voxel_geo_inject"),
                timestamp_writes: None,
            });
        pass.set_pipeline(compute);
        pass.set_bind_group(0, &bg, &[]);
        // 64 threads per workgroup.
        let groups = (buffer.count + 63) / 64;
        pass.dispatch_workgroups(groups, 1, 1);
        Ok(())
    }
}

#[derive(Default)]
pub struct GeometryVoxelizePlugin;

impl Plugin for GeometryVoxelizePlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "voxel_geo_inject.wgsl");
        app.add_systems(Update, bake_mesh_samples);

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<GeometrySampleBuffer>();
            render_app.add_systems(bevy::render::ExtractSchedule, extract_geometry_samples);
            render_app.add_systems(
                Render,
                prepare_geometry_sample_buffer.in_set(RenderSystems::PrepareResources),
            );
            render_app
                .add_render_graph_node::<ViewNodeRunner<GeometryInjectNode>>(
                    Core3d,
                    GeometryInjectLabel,
                )
                // Pipeline order: Clear → VoxelInject (visible) →
                // GeometryInject → Resolve. We slot between Inject and
                // Resolve so all contributions land in the accum buffer
                // before the single resolve drains them.
                .add_render_graph_edges(
                    Core3d,
                    (
                        crate::voxel_cache::VoxelInjectLabel,
                        GeometryInjectLabel,
                        crate::voxel_cache::VoxelResolveLabel,
                    ),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<GeometryInjectPipeline>();
        }
    }
}
