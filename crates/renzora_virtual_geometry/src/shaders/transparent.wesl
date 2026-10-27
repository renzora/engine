//! The vertex stage for transparent virtual geometry: glass, water surfaces,
//! anything blended.
//!
//! A visibility buffer holds one surface per pixel, and blending needs every
//! layer, so transparent clusters skip it. The cull pass still picks them (the
//! level of detail cut and the frustum test are the same), each transparent
//! instance in its own bucket of the visible list, and each instance is then
//! drawn by Bevy's transparent pass as one of its own sorted items, with the
//! material's ordinary forward fragment shader. This stage stands in for
//! Bevy's mesh vertex shader: it fetches the selected clusters' vertices and
//! writes the same `VertexOutput`.
//!
//! The material's fragment shader reads Bevy's `Mesh` uniform at group 2,
//! binding 0 (flags, and the material's slot for bindless materials), so this
//! pass binds one per transparent instance there and keeps the scene's own
//! bindings past it (`VG_BASE`). `instance_index` in the output picks it.

import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::view;
import bevy_render::maths::{affine3_to_square, mat2x4_f32_to_mat3x3_unpack};
import super::types::{VgInstance, VG_SIGN_DETERMINANT};
import super::bindings::{
    vg_visible, vg_instances, vg_clusters, vg_triangle_vertex, vg_world_position,
    vg_normal, vg_uv, vg_tangent, vg_is_skinned, vg_skinned_vertex,
};

/// One transparent instance's draw: where its slots start in the visible
/// list, and its entry in the `Mesh` array the fragment shader reads.
struct Bucket {
    base: u32,
    mesh_index: u32,
}
@group(2) @binding(constants::VG_BASE + 8) var<uniform> bucket: Bucket;

fn normal_to_world(instance: VgInstance, normal: vec3<f32>) -> vec3<f32> {
    if all(normal == vec3(0.0)) {
        return normal;
    }
    return normalize(
        mat2x4_f32_to_mat3x3_unpack(instance.local_from_world_transpose_a, instance.local_from_world_transpose_b)
            * normal,
    );
}

@vertex
fn vertex(@builtin(instance_index) instance_index: u32, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let entry = vg_visible[bucket.base + instance_index];
    let instance = vg_instances[entry.x];
    let cluster = vg_clusters[entry.y];

    let triangle = vertex_index / 3u;
    if triangle >= cluster.triangle_count {
        // Past the cluster's last triangle: collapse to a point, which
        // rasterizes nothing.
        out.position = vec4(0.0);
        return out;
    }
    // A mirroring transform turns every triangle inside out; swapping two
    // corners keeps the material's cull mode facing the right way.
    var corner = vertex_index % 3u;
    if (instance.flags & VG_SIGN_DETERMINANT) == 0u && corner != 0u {
        corner = 3u - corner;
    }
    let vertex = vg_triangle_vertex(cluster, triangle, corner);
    let world = vg_world_position(instance, vertex);

    var normal: vec3<f32>;
    var tangent: vec4<f32>;
    if vg_is_skinned(instance) {
        let skinned = vg_skinned_vertex(instance, vertex);
        normal = skinned.normal.xyz;
        tangent = skinned.tangent;
    } else {
        normal = normal_to_world(instance, vg_normal(vertex));
        let local = vg_tangent(vertex);
        let world_from_local = affine3_to_square(instance.world_from_local);
        let mirrored = (instance.flags & VG_SIGN_DETERMINANT) == 0u;
        tangent = vec4(
            normalize((world_from_local * vec4(local.xyz, 0.0)).xyz),
            local.w * select(1.0, -1.0, mirrored),
        );
    }

    out.position = view.clip_from_world * vec4(world, 1.0);
    out.world_position = vec4(world, 1.0);
    out.world_normal = normal;
    @if(VERTEX_UVS_A)
    out.uv = vg_uv(vertex);
    @if(VERTEX_UVS_B)
    out.uv_b = vg_uv(vertex);
    @if(VERTEX_TANGENTS)
    out.world_tangent = tangent;
    @if(VERTEX_COLORS)
    out.color = vec4(1.0);
    @if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = bucket.mesh_index;
    return out;
}
