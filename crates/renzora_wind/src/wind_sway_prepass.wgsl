// Prepass / shadow vertex stage for `WindSwayMaterial`.
//
// Bevy renders shadow maps through `PrepassPipeline` (see `render/light.rs`),
// so this one file gives swaying geometry BOTH a matching depth-prepass sample
// and a shadow that moves with it. Without it, leaves would sway in the lit
// pass while casting a rigid shadow and writing rigid depth — and since leaves
// are `AlphaMode::Mask` they do write depth, so the mismatch shows up as
// z-fighting, not merely as a wrong shadow.
//
// A copy of Bevy 0.19's `prepass.wgsl` vertex entry point with the displacement
// spliced in, for the same reason as the forward one: the hook has to sit
// between the world transform and the clip projection. Re-sync on Bevy upgrades.

// `prepass_bindings` is deliberately NOT imported: the only thing in it is
// `previous_view_uniforms`, which this stage never reads, and it only exists in
// the layout when MOTION_VECTOR_PREPASS is on.
#import bevy_pbr::{
    mesh_bindings::mesh,
    mesh_functions,
    prepass_io::{Vertex, VertexOutput},
    skinning,
    morph,
    morph::{morph_position, morph_normal, morph_tangent},
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
}
#import bevy_render::globals::Globals
#import renzora_wind::common::{wind_displace, wind_weights}

// Declared by hand rather than imported from `mesh_view_bindings`: the prepass
// view layout puts globals at binding 1, the forward one at binding 11. Pulling
// in the forward declaration here would compile and then fail to bind.
@group(0) @binding(1) var<uniform> globals: Globals;

#ifdef MORPH_TARGETS
fn morph_vertex(vertex_in: Vertex, instance_index: u32) -> Vertex {
    var vertex = vertex_in;
    let first_vertex = mesh[instance_index].first_vertex_index;
    let vertex_index = vertex.index - first_vertex;

    let weight_count = morph::layer_count(instance_index);
    for (var i: u32 = 0u; i < weight_count; i ++) {
        let weight = morph::weight_at(i, instance_index);
        if weight == 0.0 {
            continue;
        }
        vertex.position += weight * morph_position(vertex_index, i, instance_index);
#ifdef VERTEX_NORMALS
        vertex.normal += weight * morph_normal(vertex_index, i, instance_index);
#endif
#ifdef VERTEX_TANGENTS
        vertex.tangent += vec4(weight * morph_tangent(vertex_index, i, instance_index), 0.0);
#endif
    }
    return vertex;
}

fn morph_prev_vertex(vertex_in: Vertex, instance_index: u32) -> Vertex {
    var vertex = vertex_in;
    let first_vertex = mesh[instance_index].first_vertex_index;
    let vertex_index = vertex.index - first_vertex;
    let weight_count = morph::layer_count(instance_index);
    for (var i: u32 = 0u; i < weight_count; i ++) {
        let weight = morph::prev_weight_at(i, instance_index);
        if weight == 0.0 {
            continue;
        }
        vertex.position += weight * morph_position(vertex_index, i, instance_index);
    }
    return vertex;
}
#endif  // MORPH_TARGETS

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;

#ifdef MORPH_TARGETS
    var vertex = morph_vertex(vertex_no_morph, vertex_no_morph.instance_index);
#else
    var vertex = vertex_no_morph;
#endif

    let mesh_world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);

#ifdef SKINNED
    var world_from_local = skinning::skin_model(
        vertex.joint_indices,
        vertex.joint_weights,
        vertex_no_morph.instance_index
    );
#else // SKINNED
    var world_from_local = mesh_world_from_local;
#endif // SKINNED

#ifdef VERTEX_UVS_B
    let weights = wind_weights(vertex.position, vertex.uv_b, true);
#else
    let weights = wind_weights(vertex.position, vec2<f32>(0.0), false);
#endif
    let object_pos = mesh_world_from_local[3].xyz;

    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.world_position = vec4<f32>(
        out.world_position.xyz + wind_displace(
            object_pos,
            out.world_position.xyz,
            weights.x,
            weights.y,
            globals.time,
        ),
        out.world_position.w,
    );
    out.position = position_world_to_clip(out.world_position.xyz);
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0); // Clamp depth to avoid clipping
#endif // UNCLIPPED_DEPTH_ORTHO_EMULATION

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif // VERTEX_UVS_A

#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif // VERTEX_UVS_B

#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
#ifdef VERTEX_NORMALS
#ifdef SKINNED
    out.world_normal = skinning::skin_normals(world_from_local, vertex.normal);
#else // SKINNED
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex_no_morph.instance_index
    );
#endif // SKINNED
#endif // VERTEX_NORMALS

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex_no_morph.instance_index
    );
#endif // VERTEX_TANGENTS
#endif // NORMAL_PREPASS_OR_DEFERRED_PREPASS

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

#ifdef MOTION_VECTOR_PREPASS

#ifdef MORPH_TARGETS
#ifdef HAS_PREVIOUS_MORPH
    let prev_vertex = morph_prev_vertex(vertex_no_morph, vertex_no_morph.instance_index);
#else   // HAS_PREVIOUS_MORPH
    let prev_vertex = vertex_no_morph;
#endif  // HAS_PREVIOUS_MORPH
#else   // MORPH_TARGETS
    let prev_vertex = vertex_no_morph;
#endif  // MORPH_TARGETS

#ifdef SKINNED
#ifdef HAS_PREVIOUS_SKIN
    let prev_model = skinning::skin_prev_model(
        prev_vertex.joint_indices,
        prev_vertex.joint_weights,
        vertex_no_morph.instance_index
    );
#else   // HAS_PREVIOUS_SKIN
    let prev_model = mesh_functions::get_previous_world_from_local(vertex_no_morph.instance_index);
#endif  // HAS_PREVIOUS_SKIN
#else   // SKINNED
    let prev_model = mesh_functions::get_previous_world_from_local(vertex_no_morph.instance_index);
#endif  // SKINNED

    out.previous_world_position = mesh_functions::mesh_position_local_to_world(
        prev_model,
        vec4<f32>(prev_vertex.position, 1.0)
    );
    // Displace last frame's position at last frame's time too. Skipping this
    // makes every swaying vertex report a motion vector as if it were static,
    // which TAA resolves into a smeared, crawling canopy — the artefact reads
    // as "the anti-aliasing is broken", not as "the wind is wrong".
    out.previous_world_position = vec4<f32>(
        out.previous_world_position.xyz + wind_displace(
            prev_model[3].xyz,
            out.previous_world_position.xyz,
            weights.x,
            weights.y,
            globals.time - globals.delta_time,
        ),
        out.previous_world_position.w,
    );
#endif // MOTION_VECTOR_PREPASS

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex_no_morph.instance_index;
#endif

#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_no_morph.instance_index, mesh_world_from_local[3]);
#endif  // VISIBILITY_RANGE_DITHER

    return out;
}
