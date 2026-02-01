#import bevy_render::view::View
#import bevy_render::maths
#import bevy_pbr::mesh_types::SkinnedMesh
#import bevy_pbr::skinning::joint_matrices
#import bevy_mod_outline::common::{OutlineViewUniform, VertexOutput}

struct Instance {
    world_from_local: mat3x4<f32>,
    world_plane_origin: vec3<f32>,
    world_plane_offset: vec3<f32>,
    volume_colour: vec4<f32>,
    volume_offset: f32,
    stencil_offset: f32,
    alpha_mask_threshold: f32,
    first_vertex_index: u32,
    current_skin_index: u32,
};

struct Vertex {
    @location(0) position: vec3<f32>,
    @builtin(instance_index) instance_index: u32,
#ifndef VERTEX_OFFSET_ZERO
    @location(1) outline_normal: vec3<f32>,
#endif
#ifdef ALPHA_MASK_TEXTURE
    @location(2) uv: vec2<f32>,
#endif
#ifdef SKINNED
    @location(5) joint_indices: vec4<u32>,
    @location(6) joint_weights: vec4<f32>,
#endif
#ifdef MORPH_TARGETS
    @builtin(vertex_index) index: u32,
#endif
};

@group(0) @binding(0)
var<uniform> view_uniform: OutlineViewUniform;

#import bevy_pbr::skinning
#import bevy_pbr::morph

#ifdef INSTANCE_BATCH_SIZE
@group(2) @binding(0) var<uniform> mesh: array<Instance, #{INSTANCE_BATCH_SIZE}u>;
#else
@group(2) @binding(0) var<storage> mesh: array<Instance>;
#endif

#ifdef MORPH_TARGETS
fn morph_vertex(vertex_in: Vertex) -> Vertex {
    var vertex = vertex_in;
    let first_vertex = mesh[vertex.instance_index].first_vertex_index;
    let vertex_index = vertex.index - first_vertex;

    let weight_count = bevy_pbr::morph::layer_count();
    for (var i: u32 = 0u; i < weight_count; i ++) {
        let weight = bevy_pbr::morph::weight_at(i);
        if weight == 0.0 {
            continue;
        }
        vertex.position += weight * bevy_pbr::morph::morph(vertex_index, bevy_pbr::morph::position_offset, i);
    }
    return vertex;
}
#endif

#ifdef SKINNED
fn skin_model(
    indexes: vec4<u32>,
    weights: vec4<f32>,
    instance_index: u32,
) -> mat4x4<f32> {
#ifdef SKINS_USE_UNIFORM_BUFFERS
    return weights.x * joint_matrices.data[indexes.x]
        + weights.y * joint_matrices.data[indexes.y]
        + weights.z * joint_matrices.data[indexes.z]
        + weights.w * joint_matrices.data[indexes.w];
#else
    var skin_index = mesh[instance_index].current_skin_index;
    return weights.x * joint_matrices[skin_index + indexes.x]
        + weights.y * joint_matrices[skin_index + indexes.y]
        + weights.z * joint_matrices[skin_index + indexes.z]
        + weights.w * joint_matrices[skin_index + indexes.w];
#endif
}
#endif

fn mat4to3(m: mat4x4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(
        m[0].xyz, m[1].xyz, m[2].xyz
    );
}

fn model_origin_z(plane: vec3<f32>, view_proj: mat4x4<f32>) -> f32 {
    var proj_zw = mat4x2<f32>(
        view_proj[0].zw, view_proj[1].zw,
        view_proj[2].zw, view_proj[3].zw);
    var zw = proj_zw * vec4<f32>(plane, 1.0);
    return zw.x / zw.y;
}

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    let iid = vertex_no_morph.instance_index;
#ifdef MORPH_TARGETS
    var vertex = morph_vertex(vertex_no_morph);
#else
    var vertex = vertex_no_morph;
#endif
#ifdef SKINNED
    let model = skin_model(vertex.joint_indices, vertex.joint_weights, iid);
#else
    let model = bevy_render::maths::affine3_to_square(mesh[iid].world_from_local);
#endif
    let clip_pos = view_uniform.clip_from_world * (model * vec4<f32>(vertex.position, 1.0));
#ifdef VOLUME
    let offset = mesh[iid].volume_offset;
#else
    let offset = mesh[iid].stencil_offset;
#endif
#ifdef VERTEX_OFFSET_ZERO
    let out_xy = clip_pos.xy;
#else
    let clip_norm = mat4to3(view_uniform.clip_from_world) * (mat4to3(model) * vertex.outline_normal);
    let corrected_norm = normalize(clip_norm.xy * vec2<f32>(view_uniform.aspect, 1.0));
    let ndc_delta = offset * corrected_norm * view_uniform.scale_clip_from_logical * clip_pos.w;
    let out_xy = clip_pos.xy + ndc_delta;
#endif
    var out: VertexOutput;
    out.position = vec4<f32>(out_xy, clip_pos.zw);
#ifdef FLAT_DEPTH
#ifdef PLANE_OFFSET_ZERO
    let world_plane = mesh[iid].world_plane_origin;
#else
    let world_from_view = bevy_render::maths::mat2x4_f32_to_mat3x3_unpack(view_uniform.world_from_view_a, view_uniform.world_from_view_b);
    let model_eye = normalize(world_from_view * vec3<f32>(0.0, 0.0, -1.0));
    let world_plane = mesh[iid].world_plane_origin + model_eye * mesh[iid].world_plane_offset;
#endif
    out.flat_depth = model_origin_z(world_plane, view_uniform.clip_from_world);
#endif
#ifdef VOLUME
    out.volume_colour = mesh[iid].volume_colour;
#endif
#ifdef ALPHA_MASK_TEXTURE
    out.alpha_mask_threshold = mesh[iid].alpha_mask_threshold;
    out.uv = vertex.uv;
#endif
    return out;
}
