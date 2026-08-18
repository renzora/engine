// Forward vertex stage for `WindSwayMaterial`.
//
// This is Bevy 0.19's `bevy_pbr::render::mesh` vertex entry point with the wind
// displacement spliced in after the world position is computed. It is a copy
// rather than a wrapper because `MaterialExtension::vertex_shader()` replaces
// the whole stage — there is no hook that runs *between* Bevy's world-space
// transform and its clip-space projection, which is precisely where a vertex
// animation has to happen. Re-sync this file when Bevy's mesh.wgsl changes.

#import bevy_pbr::{
    mesh_functions,
    skinning,
    morph::{morph_position, morph_normal, morph_tangent},
    mesh_bindings::mesh,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::globals,
}
#import renzora_wind::common::{wind_displace, wind_weights}

#ifdef MORPH_TARGETS
fn morph_vertex(vertex_in: Vertex, instance_index: u32) -> Vertex {
    var vertex = vertex_in;
    let first_vertex = mesh[instance_index].first_vertex_index;
    let vertex_index = vertex.index - first_vertex;

    let weight_count = bevy_pbr::morph::layer_count(instance_index);
    for (var i: u32 = 0u; i < weight_count; i ++) {
        let weight = bevy_pbr::morph::weight_at(i, instance_index);
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
#endif

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
#else
    var world_from_local = mesh_world_from_local;
#endif

#ifdef VERTEX_NORMALS
#ifdef SKINNED
    out.world_normal = skinning::skin_normals(world_from_local, vertex.normal);
#else
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex_no_morph.instance_index
    );
#endif
#endif

#ifdef VERTEX_POSITIONS
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));

    // ── Wind ──────────────────────────────────────────────────────────────
    // Column 3 of the model matrix is the object's world-space origin, which is
    // what gives every vertex of one plant a single shared sway phase.
#ifdef VERTEX_UVS_B
    let weights = wind_weights(vertex.position, vertex.uv_b, true);
#else
    let weights = wind_weights(vertex.position, vec2<f32>(0.0), false);
#endif
    out.world_position = vec4<f32>(
        out.world_position.xyz + wind_displace(
            mesh_world_from_local[3].xyz,
            out.world_position.xyz,
            weights.x,
            weights.y,
            globals.time,
        ),
        out.world_position.w,
    );
    // ──────────────────────────────────────────────────────────────────────

    out.position = position_world_to_clip(out.world_position.xyz);
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex_no_morph.instance_index
    );
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex_no_morph.instance_index;
#endif

#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_no_morph.instance_index, mesh_world_from_local[3]);
#endif

    return out;
}
