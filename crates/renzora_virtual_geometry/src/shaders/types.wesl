//! The GPU layout of the virtual geometry scene. Mirrors `GpuInstance` and
//! `GpuCluster` in `render/scene.rs`; the two must change together.

/// One drawn copy of a mesh. The first five fields are laid out like Bevy's
/// `Mesh` uniform so the same transform helpers apply.
struct VgInstance {
    world_from_local: mat3x4<f32>,
    previous_world_from_local: mat3x4<f32>,
    local_from_world_transpose_a: mat2x4<f32>,
    local_from_world_transpose_b: f32,
    flags: u32,
    material_slot: u32,
    /// Low 16 bits: the material id the material-depth pass writes. Bit 31:
    /// the material is double-sided, so its clusters are rasterized unculled.
    material: u32,
    first_cluster: u32,
    cluster_count: u32,
    /// Where this instance's clusters start in the flat list of
    /// (instance, cluster) pairs the cull pass walks.
    pair_offset: u32,
    max_scale: f32,
    /// Render layers 0..63 as a bitmask, low word first. A view draws the
    /// instance only when its own layers share a bit with these.
    layers: vec2<u32>,
    /// For an alpha-masked material, which masked draw it is rasterized in:
    /// one per material bind group and cull mode. For a transparent one, its
    /// own bucket, numbered after the masked ones.
    masked_bucket: u32,
    /// The mesh's first vertex in `vg_positions`.
    vertex_base: u32,
    /// For a skinned instance (`VG_SKINNED`): where this frame's deformed
    /// copy of its vertices starts in `vg_skinned`.
    skinned_base: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

/// One deformed vertex of a skinned instance, written by `skin.wesl` every
/// frame. World space: the joint matrices already include the model's place.
struct VgSkinnedVertex {
    position: vec4<f32>,
    /// Where the vertex was last frame, from last frame's joint matrices, for
    /// motion vectors.
    previous: vec4<f32>,
    normal: vec4<f32>,
    /// xyz direction, w the bitangent sign.
    tangent: vec4<f32>,
}

/// One cluster of up to 128 triangles, with the bounds that cull it and the
/// two errors that decide whether it is part of the cut.
struct VgCluster {
    /// Tight sphere, local space: xyz centre, w radius.
    culling: vec4<f32>,
    /// The sphere the cluster's own error is measured from.
    lod: vec4<f32>,
    /// The sphere its group's simplified error is measured from.
    parent: vec4<f32>,
    lod_error: f32,
    /// 3.4e38 when nothing coarser exists.
    parent_error: f32,
    /// Start of this cluster's vertex list in `vg_indices`.
    vertex_offset: u32,
    /// Byte offset of its local triangle indices in `vg_indices`.
    triangle_offset: u32,
    triangle_count: u32,
    /// DAG level: 0 for full detail, higher for coarser. Read by the debug
    /// view only.
    level: u32,
    /// Its group and the group it was simplified from (`VG_NO_GROUP` for
    /// full detail), numbered across the scene.
    group: u32,
    refined: u32,
    /// Where its page's vertices start; its vertex list holds offsets from
    /// here.
    vertex_base: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

/// `VgCluster::refined` of a full-detail cluster.
const VG_NO_GROUP: u32 = 0xffffffffu;

/// What the cull pass needs that the view uniform does not carry.
struct VgCullParams {
    pair_count: u32,
    instance_count: u32,
    visible_capacity: u32,
    /// Error, in pixels, under which a cluster counts as detailed enough.
    lod_threshold: f32,
    /// The view's render layers, as `VgInstance::layers`.
    view_layers: vec2<u32>,
    /// Where the masked buckets start in the visible list: past the region the
    /// opaque clusters fill.
    masked_offset: u32,
    _pad: u32,
}

const VG_DOUBLE_SIDED: u32 = 0x80000000u;
/// In `VgInstance::material`: alpha-masked, so rasterized by a pass that
/// samples the material's alpha and discards what is cut out.
const VG_MASKED: u32 = 0x40000000u;
/// In `VgInstance::material`: blended, so drawn by the transparent pass from
/// its own bucket, never into the visibility buffer.
const VG_TRANSPARENT: u32 = 0x20000000u;
/// The visibility buffer stores `((visible slot << 7) | triangle) + 1`, so a
/// cleared texel, 0, means no virtual geometry. Clearing an integer target to
/// zero is the one clear every backend does the same way.
const VG_NOTHING: u32 = 0u;
/// `MeshFlags::SIGN_DETERMINANT_MODEL_3X3`: set when the transform does not
/// mirror the mesh.
const VG_SIGN_DETERMINANT: u32 = 0x80000000u;
/// In `VgInstance::flags`, a bit `MeshFlags` leaves unused: the mesh carries
/// its own tangents.
const VG_HAS_TANGENTS: u32 = 0x00200000u;
/// In `VgInstance::flags`: skinned. Its vertices are read from `vg_skinned`
/// (already in world space) instead of transformed from the rest pose.
const VG_SKINNED: u32 = 0x00400000u;
