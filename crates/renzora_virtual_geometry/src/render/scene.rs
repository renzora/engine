//! The virtual geometry scene on the GPU: every converted mesh's clusters and
//! vertices in one set of buffers, and the instances drawing them.
//!
//! The geometry is rebuilt as a whole when the set of meshes in use changes,
//! which happens when models load or unload, not per frame. Instances are
//! rewritten every frame; they are small (192 bytes each). A skinned instance
//! also gets a skin job, which `skin.wesl` turns into this frame's deformed
//! copy of its vertices.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::UntypedAssetId;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::light::NotShadowReceiver;
use bevy::math::{Affine3, Affine3A, Affine3Ext, Vec4};
use bevy::mesh::skinning::SkinnedMesh;
use bevy::pbr::{PreparedMaterial, SkinUniforms};
use bevy::prelude::*;
use bevy::render::erased_render_asset::ErasedRenderAssets;
use bevy::render::material_bind_groups::MaterialBindGroupIndex;
use bevy::render::render_resource::{
    BindGroup, BindGroupEntries, Buffer, BufferInitDescriptor, BufferUsages, PipelineCache,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::sync_world::MainEntity;
use bevy::render::Extract;
use bytemuck::{Pod, Zeroable};
use renzora_rvg::{Attributes, RvgFile, RvgMesh};

use crate::attach::{VirtualGeometry, VirtualGeometrySettings};
use crate::material::VgMaterial;

use super::pipelines::VgPipelines;
use super::streaming::Streaming;
use super::view::VirtualGeometrySettingsRender;

/// The streaming memory budget before the project's settings have arrived.
const DEFAULT_BUDGET: u64 = (crate::attach::DEFAULT_BUDGET_MB as u64) << 20;

/// One converted mesh: entry `.1` of the `.rvg` file `.0`.
pub(crate) type MeshKey = (AssetId<RvgFile>, u32);

pub(crate) struct ExtractedInstance {
    main: MainEntity,
    mesh: MeshKey,
    transform: Affine3A,
    not_shadow_receiver: bool,
    double_sided: bool,
    masked: bool,
    material: UntypedAssetId,
    layers: [u32; 2],
    skinned: bool,
    transparent: bool,
    /// The world-space centre of its bounds: what a transparent instance is
    /// sorted by, as Bevy sorts an ordinary mesh.
    center: Vec3,
}

/// What this frame's main world asked to draw.
#[derive(Resource, Default)]
pub(crate) struct ExtractedScene {
    instances: Vec<ExtractedInstance>,
    meshes: HashMap<MeshKey, Arc<RvgMesh>>,
}

#[allow(clippy::type_complexity)]
pub(crate) fn extract(
    mut out: ResMut<ExtractedScene>,
    settings: Extract<Option<Res<VirtualGeometrySettings>>>,
    files: Extract<Res<Assets<RvgFile>>>,
    query: Extract<
        Query<(
            Entity,
            &VirtualGeometry,
            &GlobalTransform,
            &ViewVisibility,
            &VgMaterial,
            Has<NotShadowReceiver>,
            Option<&RenderLayers>,
            Has<SkinnedMesh>,
            Option<&Aabb>,
            Has<renzora::SolariShadedTransparent>,
        )>,
    >,
) {
    out.instances.clear();
    out.meshes.clear();
    if !settings.as_ref().is_some_and(|s| s.enabled) {
        return;
    }
    for (entity, vg, transform, visibility, material, not_shadow_receiver, layers, skinned, aabb, ray_traced) in &query {
        let Some(mesh) = files.get(&vg.file).and_then(|f| f.meshes.get(vg.mesh as usize)) else {
            continue;
        };
        // Every converted mesh stays uploaded while it is in the scene, on
        // screen or not: the GPU copy changes only when models load or
        // unload, never because the camera turned.
        let key = (vg.file.id(), vg.mesh);
        out.meshes.entry(key).or_insert_with(|| mesh.clone());
        // Bevy's own CPU frustum test has already run: an entity outside every
        // view is not visible, and skipping it here keeps it out of the cull
        // pass entirely.
        if !visibility.get() {
            continue;
        }
        let Some(surface) = material.surface else {
            continue;
        };
        // A see-through mesh Solari lights itself (glass, a clear-coat shell)
        // is not drawn here as well; see `renzora::SolariShadedTransparent`.
        if surface.transparent && ray_traced {
            continue;
        }
        out.instances.push(ExtractedInstance {
            main: entity.into(),
            mesh: key,
            transform: transform.affine(),
            not_shadow_receiver,
            double_sided: surface.double_sided,
            masked: surface.masked,
            material: material.id,
            layers: layer_words(layers),
            skinned,
            transparent: surface.transparent,
            center: transform.transform_point(aabb.map_or(Vec3::ZERO, |a| Vec3::from(a.center))),
        });
    }
}

/// Matches `VgCluster` in `shaders/types.wesl`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct GpuCluster {
    pub culling: [f32; 4],
    pub lod: [f32; 4],
    pub parent: [f32; 4],
    pub lod_error: f32,
    pub parent_error: f32,
    pub vertex_offset: u32,
    pub triangle_offset: u32,
    pub triangle_count: u32,
    pub level: u32,
    /// Its group and the group it was simplified from (`NO_GROUP` for full
    /// detail), numbered across the whole scene.
    pub group: u32,
    pub refined: u32,
    /// Where its page's vertices start in the pool; its vertex list holds
    /// offsets from here.
    pub vertex_base: u32,
    pub _pad: [u32; 3],
}

/// `GpuCluster::refined` of a full-detail cluster.
pub(super) const NO_GROUP: u32 = u32::MAX;

/// Matches `VgInstance` in `shaders/types.wesl`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuInstance {
    world_from_local: [Vec4; 3],
    previous_world_from_local: [Vec4; 3],
    local_from_world_transpose_a: [Vec4; 2],
    local_from_world_transpose_b: f32,
    flags: u32,
    material_slot: u32,
    material: u32,
    first_cluster: u32,
    cluster_count: u32,
    pair_offset: u32,
    max_scale: f32,
    layers: [u32; 2],
    masked_bucket: u32,
    vertex_base: u32,
    skinned_base: u32,
    _pad: [u32; 3],
}

/// Matches `SkinJob` in `shaders/skin.wesl`: one skinned instance's vertices.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuSkinJob {
    vertex_base: u32,
    skin_base: u32,
    joint_base: u32,
    out_base: u32,
}

/// `VgSkinnedVertex` in `shaders/types.wesl`: four `vec4`s.
const SKINNED_VERTEX_SIZE: u64 = 64;

/// One draw of the masked raster pass: every alpha-masked instance whose
/// material lives in bind group `group` and has the same cull mode.
#[derive(Clone, Debug)]
pub(crate) struct MaskedBucket {
    pub group: MaterialBindGroupIndex,
    pub double_sided: bool,
    /// A material in the bucket, to look the bind group and layout up by.
    pub material: UntypedAssetId,
    /// First slot, relative to the masked region of the visible list.
    pub base: u32,
    /// Every cluster of every instance in the bucket: the most it can hold.
    pub capacity: u32,
}

/// One transparent instance, drawn as an item of Bevy's transparent pass.
#[derive(Clone, Debug)]
pub(crate) struct TransparentInstance {
    pub material: UntypedAssetId,
    pub group: MaterialBindGroupIndex,
    /// Its bucket, in the numbering the masked buckets start.
    pub bucket: u32,
    /// Its entry in `GpuScene::mesh_uniforms`.
    pub mesh_index: u32,
    pub has_tangents: bool,
    pub center: Vec3,
    /// First slot, relative to the bucketed region of the visible list, and
    /// how many it may fill: every cluster it has.
    pub base: u32,
    pub capacity: u32,
}

/// Bevy's `Mesh` uniform (`mesh_types.wesl`), the struct a material's forward
/// fragment shader reads its flags and bindless slot from. Written here
/// rather than taken from Bevy's `MeshUniform`, which is laid out by
/// `encase` and not `Pod`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuMeshUniform {
    world_from_local: [Vec4; 3],
    previous_world_from_local: [Vec4; 3],
    local_from_world_transpose_a: [Vec4; 2],
    local_from_world_transpose_b: f32,
    flags: u32,
    lightmap_uv_rect: [u32; 2],
    first_vertex_index: u32,
    current_skin_index: u32,
    material_and_lightmap_bind_group_slot: u32,
    tag: u32,
    morph_descriptor_index: u32,
    metadata_index: u32,
    _pad: [u32; 2],
}

/// Render layers 0..63 as two words, the form the cull pass compares. Layers
/// past 63 are not representable and never match; the engine's own layers all
/// sit below 32.
pub(crate) fn layer_words(layers: Option<&RenderLayers>) -> [u32; 2] {
    let bits = layers.map_or(1, |l| l.bits().first().copied().unwrap_or(0));
    [bits as u32, (bits >> 32) as u32]
}

/// `MeshFlags` bits the PBR shaders read, plus two of this crate's own in the
/// bits Bevy leaves unused, which those shaders never test.
const SHADOW_RECEIVER: u32 = 1 << 29;
const SIGN_DETERMINANT_MODEL_3X3: u32 = 1 << 31;
const NO_LOD: u32 = 0xffff;
/// The mesh has its own tangents; without them the resolve derives a frame
/// from the UV gradients. Matches `VG_HAS_TANGENTS` in `types.wesl`.
const HAS_TANGENTS: u32 = 1 << 21;
/// Skinned: read this frame's deformed vertices. Matches `VG_SKINNED`.
const SKINNED: u32 = 1 << 22;
const DOUBLE_SIDED: u32 = 1 << 31;
const MASKED: u32 = 1 << 30;
const TRANSPARENT: u32 = 1 << 29;

/// Packed vertex attributes, 4 words each: octahedral normal, octahedral
/// tangent with the bitangent sign in its lowest bit, and the UV as two
/// floats. 16 bytes against 36 unpacked, which is the difference between
/// Bistro's attributes fitting one storage binding or not.
pub(super) fn pack_attributes(mesh: &RvgMesh, out: &mut Vec<u32>) {
    let a = mesh.attributes;
    for i in 0..mesh.positions.len() {
        let normal = if a.has(Attributes::NORMAL) {
            Vec3::from(mesh.normals[i])
        } else {
            Vec3::Z
        };
        let (tangent, sign_negative) = if a.has(Attributes::TANGENT) {
            let t = mesh.tangents[i];
            (Vec3::new(t[0], t[1], t[2]), t[3] < 0.0)
        } else {
            (Vec3::ZERO, false)
        };
        let uv = if a.has(Attributes::UV0) {
            mesh.uv0[i]
        } else {
            [0.0; 2]
        };
        let tangent_word = (octahedral(tangent) & !1) | sign_negative as u32;
        out.extend([
            octahedral(normal.normalize_or(Vec3::Z)),
            tangent_word,
            uv[0].to_bits(),
            uv[1].to_bits(),
        ]);
    }
}

/// Octahedral encoding into two snorm16s, the inverse of Bevy's
/// `octahedral_decode_signed` after `unpack2x16snorm`. The `.rvg` stores
/// normals and tangents in exactly this form, so one encoder serves both.
fn octahedral(v: Vec3) -> u32 {
    renzora_rvg::octahedral_encode(v.to_array())
}

/// Joints and weights, 3 words a vertex: joints 0 and 1, joints 2 and 3, then
/// the weights as unorm8. 12 bytes against 24 unpacked; the shader
/// renormalizes the weights.
fn pack_skin(mesh: &RvgMesh, out: &mut Vec<u32>) {
    for (joints, weights) in mesh.joints.iter().zip(&mesh.weights) {
        let unorm = |w: f32| (w.clamp(0.0, 1.0) * 255.0).round() as u32;
        out.extend([
            joints[0] as u32 | (joints[1] as u32) << 16,
            joints[2] as u32 | (joints[3] as u32) << 16,
            unorm(weights[0]) | unorm(weights[1]) << 8 | unorm(weights[2]) << 16 | unorm(weights[3]) << 24,
        ]);
    }
}

pub(super) fn sphere(b: &renzora_rvg::Bounds) -> [f32; 4] {
    [b.center[0], b.center[1], b.center[2], b.radius]
}

/// Where one mesh's clusters sit in the shared cluster buffer.
#[derive(Clone, Copy)]
struct MeshSlot {
    first_cluster: u32,
    cluster_count: u32,
    vertex_base: u32,
    vertex_count: u32,
    /// Its first vertex in the skin buffer, when it has joints and weights.
    skin_base: Option<u32>,
}

#[derive(Resource, Default)]
pub(crate) struct GpuScene {
    meshes_built: Vec<MeshKey>,
    slots: HashMap<MeshKey, MeshSlot>,
    /// What of each converted mesh is on the GPU (`streaming.rs`).
    pub streaming: Streaming,
    pub clusters: Option<Buffer>,
    pub indices: Option<Buffer>,
    pub positions: Option<Buffer>,
    pub attributes: Option<Buffer>,
    pub instances: Option<Buffer>,
    instance_capacity: usize,
    pub instance_count: u32,
    pub pair_count: u32,
    /// Bumped whenever a buffer is replaced, so bind groups know to follow.
    pub generation: u64,
    previous: HashMap<MainEntity, [Vec4; 3]>,
    material_ids: HashMap<UntypedAssetId, u16>,
    /// The materials drawn this frame, with their material-depth ids.
    pub materials: Vec<(UntypedAssetId, u16)>,
    /// The masked raster pass's draws.
    pub masked: Vec<MaskedBucket>,
    /// The transparent instances, each a bucket of its own, numbered after
    /// the masked ones.
    pub transparent: Vec<TransparentInstance>,
    /// The slots every bucket, masked and transparent, needs in total.
    pub masked_total: u32,
    /// Each bucket's `base`, for the cull pass.
    pub masked_bases: Option<Buffer>,
    /// One Bevy `Mesh` uniform per transparent instance, for its material's
    /// fragment shader. Never empty, since the transparent layout binds it.
    pub mesh_uniforms: Option<Buffer>,
    /// Every skinned mesh's joints and weights (`pack_skin`).
    pub skin: Option<Buffer>,
    /// This frame's deformed vertices, written by the skin pass. Always
    /// present, at least one vertex long, since every scene layout binds it.
    pub skinned: Option<Buffer>,
    skinned_capacity: u64,
    /// The skin pass's jobs and its parameters, when anything is skinned.
    pub skin_jobs: Option<Buffer>,
    pub skin_params: Option<Buffer>,
    pub skinned_vertex_count: u32,
}

impl GpuScene {
    pub fn ready(&self) -> bool {
        self.instance_count > 0
            && self.clusters.is_some()
            && self.indices.is_some()
            && self.positions.is_some()
            && self.attributes.is_some()
            && self.instances.is_some()
            && self.masked_bases.is_some()
            && self.skinned.is_some()
            && self.skin.is_some()
            && self.mesh_uniforms.is_some()
            && self.streaming.ready()
    }

    /// Masked and transparent buckets together: the draws the cull pass
    /// counts into.
    pub fn bucket_count(&self) -> usize {
        self.masked.len() + self.transparent.len()
    }

    /// Each bucket's first slot, masked ones first.
    pub fn bucket_bases(&self) -> impl Iterator<Item = u32> + '_ {
        self.masked.iter().map(|b| b.base).chain(self.transparent.iter().map(|t| t.base))
    }

    /// A stable id per material, 1-based: 0 is "no virtual geometry" in the
    /// material-depth buffer.
    fn material_id(&mut self, material: UntypedAssetId) -> u16 {
        if let Some(&id) = self.material_ids.get(&material) {
            return id;
        }
        if self.material_ids.len() >= u16::MAX as usize - 1 {
            // Thousands of materials came and went. Start the numbering over;
            // every id this frame is assigned afresh.
            self.material_ids.clear();
        }
        let id = self.material_ids.len() as u16 + 1;
        self.material_ids.insert(material, id);
        id
    }

    /// The skin pass's inputs for this frame, and room for its output. The
    /// output buffer is kept and grown rather than made per frame: it is
    /// 64 bytes a vertex, and a crowd of characters is megabytes.
    fn prepare_skinning(&mut self, jobs: &[GpuSkinJob], vertex_count: u32, device: &RenderDevice) {
        let needed = u64::from(vertex_count.max(1)) * SKINNED_VERTEX_SIZE;
        if self.skinned.is_none() || self.skinned_capacity < needed {
            let size = (needed * 3 / 2).max(SKINNED_VERTEX_SIZE * 1024);
            self.skinned = Some(device.create_buffer(&bevy::render::render_resource::BufferDescriptor {
                label: Some("vg_skinned"),
                size,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: false,
            }));
            self.skinned_capacity = size;
            self.generation += 1;
        }
        if jobs.is_empty() {
            return;
        }
        self.skinned_vertex_count = vertex_count;
        self.skin_jobs = Some(device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("vg_skin_jobs"),
            contents: bytemuck::cast_slice(jobs),
            usage: BufferUsages::STORAGE,
        }));
        let params: [u32; 4] = [vertex_count, jobs.len() as u32, 0, 0];
        self.skin_params = Some(device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("vg_skin_params"),
            contents: bytemuck::cast_slice(&params),
            usage: BufferUsages::UNIFORM,
        }));
    }

    /// Follow the set of converted meshes (streaming uploads the ones that
    /// appeared and frees the ones that went), rebuild what is laid out per
    /// mesh when that set changed, then load and evict pages within `budget`.
    fn update_geometry(&mut self, scene: &ExtractedScene, budget: u64, device: &RenderDevice, queue: &RenderQueue) {
        let mut keys: Vec<MeshKey> = scene.meshes.keys().copied().collect();
        keys.sort_unstable_by_key(|(file, mesh)| (*file, *mesh));
        let wanted: Vec<(MeshKey, Arc<RvgMesh>)> = keys.iter().map(|k| (*k, scene.meshes[k].clone())).collect();
        self.streaming.sync(&wanted, device, queue);

        if keys != self.meshes_built || self.skin.is_none() {
            let mut skin: Vec<u32> = Vec::new();
            self.slots.clear();
            for (key, mesh) in &wanted {
                let Some(placement) = self.streaming.placement(key) else {
                    continue;
                };
                let skin_base = mesh.attributes.has(Attributes::SKIN).then(|| {
                    let base = (skin.len() / 3) as u32;
                    pack_skin(mesh, &mut skin);
                    base
                });
                self.slots.insert(
                    *key,
                    MeshSlot {
                        first_cluster: placement.first_cluster,
                        cluster_count: placement.cluster_count,
                        vertex_base: placement.vertex_base,
                        vertex_count: mesh.positions.len() as u32,
                        skin_base,
                    },
                );
            }
            self.skin = Some(device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("vg_skin"),
                // wgpu refuses a zero-sized binding.
                contents: if skin.is_empty() { &[0; 16] } else { bytemuck::cast_slice(&skin) },
                usage: BufferUsages::STORAGE,
            }));
            self.meshes_built = keys;
            self.generation += 1;
        }

        self.streaming.update(budget, device, queue);
        // The pool's buffers are replaced when it grows; follow them.
        self.clusters = self.streaming.clusters.clone();
        if let Some(pool) = self.streaming.pool() {
            self.positions = Some(pool.positions.clone());
            self.attributes = Some(pool.attributes.clone());
            self.indices = Some(pool.indices.clone());
            let limit = device.limits().max_storage_buffer_binding_size;
            for (name, buffer) in [
                ("positions", &pool.positions),
                ("attributes", &pool.attributes),
                ("indices", &pool.indices),
            ] {
                if buffer.size() > limit {
                    error!(
                        "[virtual geometry] the scene's {name} take {} MB, over this GPU's {} MB \
                         storage binding limit; some converted meshes will render wrongly",
                        buffer.size() / 1_000_000,
                        limit / 1_000_000
                    );
                }
            }
        }
    }
}

/// After the frame is submitted: send the cull pass's requests back to the
/// CPU (see `streaming.rs`).
pub(crate) fn read_back(mut gpu: ResMut<GpuScene>, device: Res<RenderDevice>, queue: Res<RenderQueue>) {
    gpu.streaming.read_back(&device, &queue);
}

pub(crate) fn prepare(
    mut gpu: ResMut<GpuScene>,
    scene: Res<ExtractedScene>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    render_materials: Res<ErasedRenderAssets<PreparedMaterial>>,
    skins: Option<Res<SkinUniforms>>,
    settings: Option<Res<VirtualGeometrySettingsRender>>,
) {
    let gpu = &mut *gpu;
    gpu.materials.clear();
    gpu.masked.clear();
    gpu.transparent.clear();
    gpu.masked_total = 0;
    gpu.instance_count = 0;
    gpu.pair_count = 0;
    gpu.skinned_vertex_count = 0;
    gpu.skin_jobs = None;
    gpu.skin_params = None;
    if scene.instances.is_empty() {
        gpu.previous.clear();
        return;
    }
    let budget = settings.map_or(DEFAULT_BUDGET, |s| s.budget_bytes);
    gpu.update_geometry(&scene, budget, &device, &queue);

    let mut instances: Vec<GpuInstance> = Vec::with_capacity(scene.instances.len());
    let mut previous = HashMap::with_capacity(scene.instances.len());
    let mut pairs = 0u32;
    let mut jobs: Vec<GpuSkinJob> = Vec::new();
    let mut skinned_total = 0u32;
    let mut mesh_uniforms: Vec<GpuMeshUniform> = Vec::new();
    // Which entries of `instances` are transparent: their bucket numbers are
    // only known once every masked bucket exists.
    let mut transparent_instances: Vec<usize> = Vec::new();
    for inst in &scene.instances {
        let Some(slot) = gpu.slots.get(&inst.mesh).copied() else {
            continue;
        };
        // A skinned mesh is drawn deformed or not at all: its rest pose is a
        // T-pose somewhere near the character, which is worse than a frame
        // without it. Bevy has no joints for it until the skin is extracted.
        let skin = if inst.skinned {
            let joint_base = skins.as_ref().and_then(|s| s.skin_index(inst.main));
            let (Some(joint_base), Some(skin_base)) = (joint_base, slot.skin_base) else {
                continue;
            };
            Some((joint_base, skin_base))
        } else {
            None
        };
        // Not uploaded yet, or not a material the material passes can bind:
        // skip it this frame rather than draw it with someone else's.
        let Some(material) = render_materials.get(inst.material) else {
            continue;
        };
        let affine = Affine3 {
            matrix3: inst.transform.matrix3.into(),
            translation: inst.transform.translation.into(),
        };
        let world_from_local = affine.to_transpose();
        let previous_world_from_local = gpu
            .previous
            .get(&inst.main)
            .copied()
            .unwrap_or(world_from_local);
        previous.insert(inst.main, world_from_local);
        let (transpose_a, transpose_b) = affine.inverse_transpose_3x3();

        let m = inst.transform.matrix3;
        let max_scale = m.x_axis.length().max(m.y_axis.length()).max(m.z_axis.length());
        let mut flags = NO_LOD;
        if !inst.not_shadow_receiver {
            flags |= SHADOW_RECEIVER;
        }
        if m.determinant().is_sign_positive() {
            flags |= SIGN_DETERMINANT_MODEL_3X3;
        }
        if scene.meshes[&inst.mesh].attributes.has(Attributes::TANGENT) {
            flags |= HAS_TANGENTS;
        }
        let mut skinned_base = 0;
        if let Some((joint_base, skin_base)) = skin {
            flags |= SKINNED;
            skinned_base = skinned_total;
            jobs.push(GpuSkinJob {
                vertex_base: slot.vertex_base,
                skin_base,
                joint_base,
                out_base: skinned_total,
            });
            skinned_total += slot.vertex_count;
        }
        // A transparent material never reaches the visibility buffer, so it
        // needs no material-depth id or shading pass.
        let mut material_word = 0;
        if !inst.transparent {
            let id = gpu.material_id(inst.material);
            if !gpu.materials.iter().any(|(m, _)| *m == inst.material) {
                gpu.materials.push((inst.material, id));
            }
            material_word = id as u32;
        }
        if inst.double_sided {
            material_word |= DOUBLE_SIDED;
        }
        let mut masked_bucket = 0;
        if inst.transparent {
            material_word |= TRANSPARENT;
            transparent_instances.push(instances.len());
            gpu.transparent.push(TransparentInstance {
                material: inst.material,
                group: material.binding.group,
                bucket: 0,
                mesh_index: mesh_uniforms.len() as u32,
                has_tangents: flags & HAS_TANGENTS != 0,
                center: inst.center,
                base: 0,
                capacity: slot.cluster_count,
            });
            mesh_uniforms.push(GpuMeshUniform {
                world_from_local,
                previous_world_from_local,
                local_from_world_transpose_a: transpose_a,
                local_from_world_transpose_b: transpose_b,
                flags,
                lightmap_uv_rect: [0; 2],
                first_vertex_index: 0,
                current_skin_index: u32::MAX,
                material_and_lightmap_bind_group_slot: u32::from(material.binding.slot),
                tag: 0,
                morph_descriptor_index: u32::MAX,
                metadata_index: 0,
                _pad: [0; 2],
            });
        } else if inst.masked {
            material_word |= MASKED;
            let group = material.binding.group;
            masked_bucket = match gpu
                .masked
                .iter()
                .position(|b| b.group == group && b.double_sided == inst.double_sided)
            {
                Some(i) => i,
                None => {
                    gpu.masked.push(MaskedBucket {
                        group,
                        double_sided: inst.double_sided,
                        material: inst.material,
                        base: 0,
                        capacity: 0,
                    });
                    gpu.masked.len() - 1
                }
            };
            gpu.masked[masked_bucket].capacity += slot.cluster_count;
        }

        instances.push(GpuInstance {
            world_from_local,
            previous_world_from_local,
            local_from_world_transpose_a: transpose_a,
            local_from_world_transpose_b: transpose_b,
            flags,
            material_slot: u32::from(material.binding.slot),
            material: material_word,
            first_cluster: slot.first_cluster,
            cluster_count: slot.cluster_count,
            pair_offset: pairs,
            max_scale,
            layers: inst.layers,
            masked_bucket: masked_bucket as u32,
            vertex_base: slot.vertex_base,
            skinned_base,
            _pad: [0; 3],
        });
        pairs += slot.cluster_count;
    }
    gpu.prepare_skinning(&jobs, skinned_total, &device);

    let mut base = 0;
    for bucket in &mut gpu.masked {
        bucket.base = base;
        base += bucket.capacity;
    }
    let first_transparent = gpu.masked.len() as u32;
    for (t, (entry, &index)) in gpu.transparent.iter_mut().zip(&transparent_instances).enumerate() {
        entry.bucket = first_transparent + t as u32;
        entry.base = base;
        base += entry.capacity;
        instances[index].masked_bucket = entry.bucket;
    }
    gpu.masked_total = base;
    // Never empty: wgpu refuses a zero-sized binding.
    if mesh_uniforms.is_empty() {
        mesh_uniforms.push(GpuMeshUniform::zeroed());
    }
    gpu.mesh_uniforms = Some(device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("vg_transparent_mesh_uniforms"),
        contents: bytemuck::cast_slice(&mesh_uniforms),
        usage: BufferUsages::STORAGE,
    }));
    let bases: Vec<u32> = gpu.bucket_bases().collect();
    gpu.masked_bases = Some(device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("vg_masked_bases"),
        // Never empty: wgpu refuses a zero-sized binding.
        contents: if bases.is_empty() { &[0; 4] } else { bytemuck::cast_slice(&bases) },
        usage: BufferUsages::STORAGE,
    }));
    gpu.previous = previous;
    gpu.instance_count = instances.len() as u32;
    gpu.pair_count = pairs;
    if instances.is_empty() {
        return;
    }

    let bytes: &[u8] = bytemuck::cast_slice(&instances);
    if gpu.instance_capacity < instances.len() || gpu.instances.is_none() {
        // Grow with headroom so models streaming in do not reallocate every
        // frame.
        let capacity = (instances.len() * 3 / 2).max(64);
        gpu.instances = Some(device.create_buffer(&bevy::render::render_resource::BufferDescriptor {
            label: Some("vg_instances"),
            size: (capacity * std::mem::size_of::<GpuInstance>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        gpu.instance_capacity = capacity;
        gpu.generation += 1;
    }
    if let Some(buffer) = &gpu.instances {
        queue.write_buffer(buffer, 0, bytes);
    }
}

/// The skin pass's bind group, when anything is skinned this frame. One for
/// the frame, not per view: the deformed vertices do not depend on a camera.
#[derive(Resource, Default)]
pub(crate) struct VgSkinBindGroup(pub Option<(BindGroup, u32)>);

/// Made after Bevy's `prepare_skins`, which swaps the current and previous
/// joint buffers and may replace both, so the bind group must take whichever
/// ones it left behind.
pub(crate) fn prepare_skin_bind_group(
    gpu: Res<GpuScene>,
    skins: Option<Res<SkinUniforms>>,
    pipelines: Option<Res<VgPipelines>>,
    pipeline_cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    mut out: ResMut<VgSkinBindGroup>,
) {
    out.0 = None;
    let (Some(skins), Some(pipelines)) = (skins, pipelines) else {
        return;
    };
    let (Some(params), Some(jobs), Some(skin), Some(positions), Some(attributes), Some(skinned)) = (
        gpu.skin_params.as_ref(),
        gpu.skin_jobs.as_ref(),
        gpu.skin.as_ref(),
        gpu.positions.as_ref(),
        gpu.attributes.as_ref(),
        gpu.skinned.as_ref(),
    ) else {
        return;
    };
    let bind_group = device.create_bind_group(
        "vg_skin",
        &pipeline_cache.get_bind_group_layout(&pipelines.skin_layout),
        &BindGroupEntries::sequential((
            params.as_entire_binding(),
            jobs.as_entire_binding(),
            skin.as_entire_binding(),
            positions.as_entire_binding(),
            attributes.as_entire_binding(),
            skins.current_buffer.as_entire_binding(),
            skins.prev_buffer.as_entire_binding(),
            skinned.as_entire_binding(),
        )),
    );
    out.0 = Some((bind_group, gpu.skinned_vertex_count));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bevy's decode, in Rust, to check the encoder against.
    fn decode(word: u32) -> Vec3 {
        let unpack = |x: u32| ((x & 0xffff) as u16 as i16 as f32 / 32767.0).max(-1.0);
        let v = Vec2::new(unpack(word), unpack(word >> 16));
        let mut n = Vec3::new(v.x, v.y, 1.0 - v.x.abs() - v.y.abs());
        let t = (-n.z).clamp(0.0, 1.0);
        n.x += if n.x >= 0.0 { -t } else { t };
        n.y += if n.y >= 0.0 { -t } else { t };
        n.normalize()
    }

    #[test]
    fn octahedral_round_trips() {
        for v in [
            Vec3::X,
            Vec3::NEG_Y,
            Vec3::Z,
            Vec3::NEG_Z,
            Vec3::new(0.3, -0.5, -0.8).normalize(),
            Vec3::new(-0.9, 0.1, 0.2).normalize(),
        ] {
            let back = decode(octahedral(v));
            assert!(back.dot(v) > 0.9999, "{v} came back as {back}");
        }
    }

    #[test]
    fn gpu_structs_match_the_shader_layout() {
        assert_eq!(std::mem::size_of::<GpuCluster>(), 96);
        assert_eq!(std::mem::size_of::<GpuInstance>(), 192);
        assert_eq!(std::mem::size_of::<GpuSkinJob>(), 16);
        // Bevy's `Mesh` struct: 168 bytes of fields, rounded to 16.
        assert_eq!(std::mem::size_of::<GpuMeshUniform>(), 176);
    }
}
