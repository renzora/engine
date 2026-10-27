//! The scene, bound read-only, plus the visibility buffer. Shared by the
//! raster, material-depth, material and transparent passes, each of which puts
//! it in a different bind group: `VG_GROUP` says which, and `VG_BASE` where its
//! bindings start. The transparent pass shares group 2 with Bevy's own mesh
//! bindings (which its material's fragment shader reads at binding 0), so it
//! moves these past them; every other pass uses 0.

import super::types::{VgInstance, VgCluster, VgSkinnedVertex, VG_SKINNED};
import bevy_render::utils::octahedral_decode_signed;
import bevy_render::maths::affine3_to_square;

/// Per pixel: `((visible slot << 7) | triangle) + 1`, or `VG_NOTHING`.
@group(constants::VG_GROUP) @binding(constants::VG_BASE) var vg_visibility: texture_2d<u32>;
/// Per visible slot: (instance, cluster).
@group(constants::VG_GROUP) @binding(constants::VG_BASE + 1) var<storage, read> vg_visible: array<vec2<u32>>;
@group(constants::VG_GROUP) @binding(constants::VG_BASE + 2) var<storage, read> vg_instances: array<VgInstance>;
@group(constants::VG_GROUP) @binding(constants::VG_BASE + 3) var<storage, read> vg_clusters: array<VgCluster>;
/// Cluster vertex lists (indices into `vg_vertices`), followed by the
/// clusters' local triangle indices packed four to a word.
@group(constants::VG_GROUP) @binding(constants::VG_BASE + 4) var<storage, read> vg_indices: array<u32>;
/// 3 floats per vertex.
@group(constants::VG_GROUP) @binding(constants::VG_BASE + 5) var<storage, read> vg_positions: array<f32>;
/// 4 words per vertex: octahedral normal, octahedral tangent with the
/// bitangent sign in its lowest bit, UV x and y as float bits.
@group(constants::VG_GROUP) @binding(constants::VG_BASE + 6) var<storage, read> vg_attributes: array<u32>;
/// This frame's deformed vertices of every skinned instance.
@group(constants::VG_GROUP) @binding(constants::VG_BASE + 7) var<storage, read> vg_skinned: array<VgSkinnedVertex>;

fn vg_is_skinned(instance: VgInstance) -> bool {
    return (instance.flags & VG_SKINNED) != 0u;
}

/// A skinned instance's deformed copy of one of its mesh's vertices.
fn vg_skinned_vertex(instance: VgInstance, vertex: u32) -> VgSkinnedVertex {
    return vg_skinned[instance.skinned_base + vertex - instance.vertex_base];
}

/// A vertex of an instance in world space: deformed for a skinned instance,
/// the rest pose through the instance's transform for everything else.
fn vg_world_position(instance: VgInstance, vertex: u32) -> vec3<f32> {
    if vg_is_skinned(instance) {
        return vg_skinned_vertex(instance, vertex).position.xyz;
    }
    return (affine3_to_square(instance.world_from_local) * vec4(vg_position(vertex), 1.0)).xyz;
}

/// The vertex at one corner of one of a cluster's triangles: an index into
/// the vertex pool. A cluster's vertex list holds offsets from its page's
/// first vertex, so a page can land anywhere in the pool.
fn vg_triangle_vertex(cluster: VgCluster, triangle: u32, corner: u32) -> u32 {
    let byte = cluster.triangle_offset + triangle * 3u + corner;
    let local = extractBits(vg_indices[byte >> 2u], (byte & 3u) * 8u, 8u);
    return cluster.vertex_base + vg_indices[cluster.vertex_offset + local];
}

fn vg_position(vertex: u32) -> vec3<f32> {
    let b = vertex * 3u;
    return vec3(vg_positions[b], vg_positions[b + 1u], vg_positions[b + 2u]);
}

fn vg_normal(vertex: u32) -> vec3<f32> {
    return octahedral_decode_signed(unpack2x16snorm(vg_attributes[vertex * 4u]));
}

/// xyz direction, w the bitangent sign.
fn vg_tangent(vertex: u32) -> vec4<f32> {
    let word = vg_attributes[vertex * 4u + 1u];
    let direction = octahedral_decode_signed(unpack2x16snorm(word & ~1u));
    return vec4(direction, select(1.0, -1.0, (word & 1u) != 0u));
}

fn vg_uv(vertex: u32) -> vec2<f32> {
    let b = vertex * 4u + 2u;
    return vec2(bitcast<f32>(vg_attributes[b]), bitcast<f32>(vg_attributes[b + 1u]));
}
