//! Rebuild a material's vertex inputs from the visibility buffer.
//!
//! Registered as `bevy_pbr::meshlet::visibility_buffer_resolve`, the module
//! Bevy's PBR shaders import when compiled with `MESHLET_MESH_MATERIAL_PASS`.
//! Supplying it is what lets every material whose shader honours that flag
//! (`StandardMaterial` among them) shade virtual geometry with no change of its
//! own: its forward, prepass and deferred fragment shaders run as they are,
//! over a fullscreen triangle, reading their surface from here. Bevy's meshlet
//! renderer is not built, so nothing else claims the path.
//!
//! The barycentric and derivative reconstruction is adapted from Bevy's
//! meshlet resolve (MIT OR Apache-2.0), itself after The Forge's visibility
//! buffer: the triangle's three clip positions give perspective-correct
//! barycentrics and their screen-space derivatives analytically, so texture
//! sampling picks the right mip without a rasterizer's quad derivatives.

import bevy_pbr::render::{
    mesh_view_bindings::view,
    mesh_functions::mesh_position_local_to_world,
    view_transformations::{position_world_to_clip, frag_coord_to_ndc},
};
import bevy_render::maths::{affine3_to_square, mat2x4_f32_to_mat3x3_unpack};
import renzora_virtual_geometry::shaders::{
    types::{VgInstance, VgSkinnedVertex, VG_HAS_TANGENTS, VG_DOUBLE_SIDED},
    bindings::{
        vg_visibility, vg_visible, vg_instances, vg_clusters,
        vg_triangle_vertex, vg_position, vg_normal, vg_uv, vg_tangent,
        vg_is_skinned, vg_skinned_vertex,
    },
};

@if(PREPASS_FRAGMENT && MOTION_VECTOR_PREPASS)
import bevy_pbr::render::pbr_prepass_functions::calculate_motion_vector;

struct PartialDerivatives {
    barycentrics: vec3<f32>,
    ddx: vec3<f32>,
    ddy: vec3<f32>,
}

fn compute_partial_derivatives(vertex_world_positions: array<vec4<f32>, 3>, ndc_uv: vec2<f32>, two_over_screen_size: vec2<f32>) -> PartialDerivatives {
    var result: PartialDerivatives;

    let vertex_clip_position_0 = position_world_to_clip(vertex_world_positions[0].xyz);
    let vertex_clip_position_1 = position_world_to_clip(vertex_world_positions[1].xyz);
    let vertex_clip_position_2 = position_world_to_clip(vertex_world_positions[2].xyz);

    let inv_w = 1.0 / vec3(vertex_clip_position_0.w, vertex_clip_position_1.w, vertex_clip_position_2.w);
    let ndc_0 = vertex_clip_position_0.xy * inv_w[0];
    let ndc_1 = vertex_clip_position_1.xy * inv_w[1];
    let ndc_2 = vertex_clip_position_2.xy * inv_w[2];

    let inv_det = 1.0 / determinant(mat2x2(ndc_2 - ndc_1, ndc_0 - ndc_1));
    result.ddx = vec3(ndc_1.y - ndc_2.y, ndc_2.y - ndc_0.y, ndc_0.y - ndc_1.y) * inv_det * inv_w;
    result.ddy = vec3(ndc_2.x - ndc_1.x, ndc_0.x - ndc_2.x, ndc_1.x - ndc_0.x) * inv_det * inv_w;

    var ddx_sum = dot(result.ddx, vec3(1.0));
    var ddy_sum = dot(result.ddy, vec3(1.0));

    let delta_v = ndc_uv - ndc_0;
    let interp_inv_w = inv_w.x + delta_v.x * ddx_sum + delta_v.y * ddy_sum;
    let interp_w = 1.0 / interp_inv_w;

    result.barycentrics = vec3(
        interp_w * (inv_w[0] + delta_v.x * result.ddx.x + delta_v.y * result.ddy.x),
        interp_w * (delta_v.x * result.ddx.y + delta_v.y * result.ddy.y),
        interp_w * (delta_v.x * result.ddx.z + delta_v.y * result.ddy.z),
    );

    result.ddx *= two_over_screen_size.x;
    result.ddy *= two_over_screen_size.y;
    ddx_sum *= two_over_screen_size.x;
    ddy_sum *= two_over_screen_size.y;

    result.ddy *= -1.0;
    ddy_sum *= -1.0;

    let interp_ddx_w = 1.0 / (interp_inv_w + ddx_sum);
    let interp_ddy_w = 1.0 / (interp_inv_w + ddy_sum);

    result.ddx = interp_ddx_w * (result.barycentrics * interp_inv_w + result.ddx) - result.barycentrics;
    result.ddy = interp_ddy_w * (result.barycentrics * interp_inv_w + result.ddy) - result.barycentrics;
    return result;
}

/// The fields Bevy's PBR shaders read in a `MESHLET_MESH_MATERIAL_PASS`.
struct VertexOutput {
    position: vec4<f32>,
    world_position: vec4<f32>,
    world_normal: vec3<f32>,
    uv: vec2<f32>,
    ddx_uv: vec2<f32>,
    ddy_uv: vec2<f32>,
    world_tangent: vec4<f32>,
    mesh_flags: u32,
    cluster_id: u32,
    material_bind_group_slot: u32,
    @if(PREPASS_FRAGMENT && MOTION_VECTOR_PREPASS)
    motion_vector: vec2<f32>,
}

fn resolve_vertex_output(frag_coord: vec4<f32>) -> VertexOutput {
    // Only pixels with virtual geometry reach here: the material-depth test
    // rejects the rest, so the stored value is never 0.
    let packed = textureLoad(vg_visibility, vec2<u32>(frag_coord.xy), 0).r - 1u;
    let entry = vg_visible[packed >> 7u];
    let triangle = packed & 127u;
    var instance = vg_instances[entry.x];
    let cluster = vg_clusters[entry.y];

    let v0 = vg_triangle_vertex(cluster, triangle, 0u);
    let v1 = vg_triangle_vertex(cluster, triangle, 1u);
    let v2 = vg_triangle_vertex(cluster, triangle, 2u);
    let skinned = vg_is_skinned(instance);
    let world_from_local = affine3_to_square(instance.world_from_local);
    // A skinned instance's vertices were deformed into world space this frame
    // by `skin.wesl`; everything else is the rest pose through its transform.
    var corners: array<VgSkinnedVertex, 3>;
    if skinned {
        corners = array(vg_skinned_vertex(instance, v0), vg_skinned_vertex(instance, v1), vg_skinned_vertex(instance, v2));
    }
    var world_position_0: vec4<f32>;
    var world_position_1: vec4<f32>;
    var world_position_2: vec4<f32>;
    if skinned {
        world_position_0 = vec4(corners[0].position.xyz, 1.0);
        world_position_1 = vec4(corners[1].position.xyz, 1.0);
        world_position_2 = vec4(corners[2].position.xyz, 1.0);
    } else {
        world_position_0 = mesh_position_local_to_world(world_from_local, vec4(vg_position(v0), 1.0));
        world_position_1 = mesh_position_local_to_world(world_from_local, vec4(vg_position(v1), 1.0));
        world_position_2 = mesh_position_local_to_world(world_from_local, vec4(vg_position(v2), 1.0));
    }

    let frag_coord_ndc = frag_coord_to_ndc(frag_coord).xy;
    let partial_derivatives = compute_partial_derivatives(
        array(world_position_0, world_position_1, world_position_2),
        frag_coord_ndc,
        2.0 / view.viewport.zw,
    );
    let bary = partial_derivatives.barycentrics;

    let world_position = mat3x4(world_position_0, world_position_1, world_position_2) * bary;
    let world_positions_camera_relative = mat3x3(
        world_position_0.xyz - view.world_position,
        world_position_1.xyz - view.world_position,
        world_position_2.xyz - view.world_position,
    );
    let ddx_world_position = world_positions_camera_relative * partial_derivatives.ddx;
    let ddy_world_position = world_positions_camera_relative * partial_derivatives.ddy;

    var world_normal: vec3<f32>;
    if skinned {
        world_normal = normalize(mat3x3(corners[0].normal.xyz, corners[1].normal.xyz, corners[2].normal.xyz) * bary);
    } else {
        world_normal = normalize(mat3x3(
            normal_local_to_world(vg_normal(v0), &instance),
            normal_local_to_world(vg_normal(v1), &instance),
            normal_local_to_world(vg_normal(v2), &instance),
        ) * bary);
    }

    let uvs = mat3x2(vg_uv(v0), vg_uv(v1), vg_uv(v2));
    let uv = uvs * bary;
    let ddx_uv = uvs * partial_derivatives.ddx;
    let ddy_uv = uvs * partial_derivatives.ddy;

    // The mesh's own tangents when it has them, so a normal map baked against
    // them shades exactly as on ordinary geometry; otherwise derive a frame
    // from the position and UV gradients.
    var world_tangent: vec4<f32>;
    if (instance.flags & VG_HAS_TANGENTS) != 0u && skinned {
        // Already in world space, the sign already carrying any mirroring.
        let tangent = mat3x3(corners[0].tangent.xyz, corners[1].tangent.xyz, corners[2].tangent.xyz) * bary;
        world_tangent = vec4(normalize(tangent), sign(corners[0].tangent.w));
    } else if (instance.flags & VG_HAS_TANGENTS) != 0u {
        let t0 = vg_tangent(v0);
        let local_tangent = mat3x4(t0, vg_tangent(v1), vg_tangent(v2)) * bary;
        let tangent = (world_from_local * vec4(local_tangent.xyz, 0.0)).xyz;
        // A mirroring transform flips the bitangent, as in Bevy's own
        // `mesh_tangent_local_to_world`.
        let mirrored = (instance.flags & 0x80000000u) == 0u;
        world_tangent = vec4(normalize(tangent), sign(t0.w) * select(1.0, -1.0, mirrored));
    } else {
        world_tangent = calculate_world_tangent(world_normal, ddx_world_position, ddy_world_position, ddx_uv, ddy_uv);
    }

    // A double-sided surface seen from behind is shaded with its normal
    // turned toward the camera, as Bevy's own fragment shaders do with their
    // `is_front`. Bevy's PBR shaders take every visibility-buffer pixel as a
    // front face, so without this the back of a thin double-sided shell (a
    // chair's seat, a leaf) kept a normal facing away from the eye: raster
    // lighting only dimmed it, but Solari, which lights from that normal,
    // left it black wherever the reversed normal faced away from every light.
    // Which side is in view comes from the triangle's own face, oriented by
    // its vertex normals, not from the interpolated normal, which near a
    // silhouette can tip away from the camera on a front face. All three axes
    // turn, as Bevy's `Nt = -Nt` does to a normal map on a back face.
    if (instance.material & VG_DOUBLE_SIDED) != 0u {
        var face = cross(world_position_1.xyz - world_position_0.xyz, world_position_2.xyz - world_position_0.xyz);
        if dot(face, world_normal) < 0.0 {
            face = -face;
        }
        if dot(face, view.world_position - world_position.xyz) < 0.0 {
            world_normal = -world_normal;
            world_tangent = vec4(-world_tangent.xyz, -world_tangent.w);
        }
    }

    var out: VertexOutput;
    out.position = frag_coord;
    out.world_position = world_position;
    out.world_normal = world_normal;
    out.uv = uv;
    out.ddx_uv = ddx_uv;
    out.ddy_uv = ddy_uv;
    out.world_tangent = world_tangent;
    out.mesh_flags = instance.flags;
    out.cluster_id = entry.y;
    out.material_bind_group_slot = instance.material_slot;

    @if(PREPASS_FRAGMENT && MOTION_VECTOR_PREPASS) {
        var previous_world_position: vec4<f32>;
        if skinned {
            previous_world_position = mat3x4(
                vec4(corners[0].previous.xyz, 1.0),
                vec4(corners[1].previous.xyz, 1.0),
                vec4(corners[2].previous.xyz, 1.0),
            ) * bary;
        } else {
            let previous_world_from_local = affine3_to_square(instance.previous_world_from_local);
            previous_world_position = mat3x4(
                mesh_position_local_to_world(previous_world_from_local, vec4(vg_position(v0), 1.0)),
                mesh_position_local_to_world(previous_world_from_local, vec4(vg_position(v1), 1.0)),
                mesh_position_local_to_world(previous_world_from_local, vec4(vg_position(v2), 1.0)),
            ) * bary;
        }
        out.motion_vector = calculate_motion_vector(world_position, previous_world_position);
    }
    return out;
}

fn normal_local_to_world(vertex_normal: vec3<f32>, instance: ptr<function, VgInstance>) -> vec3<f32> {
    if any(vertex_normal != vec3<f32>(0.0)) {
        return normalize(
            mat2x4_f32_to_mat3x3_unpack(
                (*instance).local_from_world_transpose_a,
                (*instance).local_from_world_transpose_b,
            ) * vertex_normal
        );
    }
    return vertex_normal;
}

/// https://www.jeremyong.com/graphics/2023/12/16/surface-gradient-bump-mapping/#surface-gradient-from-a-tangent-space-normal-vector-without-an-explicit-tangent-basis
fn calculate_world_tangent(
    world_normal: vec3<f32>,
    ddx_world_position: vec3<f32>,
    ddy_world_position: vec3<f32>,
    ddx_uv: vec2<f32>,
    ddy_uv: vec2<f32>,
) -> vec4<f32> {
    let ddx_world_position_s = ddx_world_position - dot(ddx_world_position, world_normal) * world_normal;
    let ddy_world_position_s = ddy_world_position - dot(ddy_world_position, world_normal) * world_normal;
    let jacobian_sign = sign(ddx_uv.x * ddy_uv.y - ddx_uv.y * ddy_uv.x);
    var world_tangent = jacobian_sign * (ddy_uv.y * ddx_world_position_s - ddx_uv.y * ddy_world_position_s);
    if jacobian_sign != 0.0 {
        world_tangent = normalize(world_tangent);
    }
    let w = jacobian_sign * sign(dot(ddy_world_position, cross(world_normal, ddx_world_position)));
    return vec4(world_tangent, -w);
}
