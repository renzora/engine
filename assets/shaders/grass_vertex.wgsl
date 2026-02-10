// Grass Vertex Shader
// Displaces grass blade vertices with procedural wind animation.
// Expects UV.y = 0 at root, 1 at tip.

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::globals,
}

struct GrassUniforms {
    base_color: vec4<f32>,
    tip_color: vec4<f32>,
    // x = wind dir X, y = wind dir Z, z = strength, w = speed
    wind_params: vec4<f32>,
};

@group(3) @binding(0) var<uniform> material: GrassUniforms;

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;

    var world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    var world_pos = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex_in.position, 1.0));

    // Wind displacement — only affects vertices near the tip (UV.y → 1)
#ifdef VERTEX_UVS_A
    let height_factor = vertex_in.uv.y;
#else
    let height_factor = clamp(vertex_in.position.y, 0.0, 1.0);
#endif

    let time = globals.time;
    let wind_dir = normalize(vec2<f32>(material.wind_params.x, material.wind_params.y) + vec2<f32>(0.001));
    let wind_strength = material.wind_params.z;
    let wind_speed = material.wind_params.w;

    // Primary wind wave
    let phase = dot(world_pos.xz, wind_dir * 0.4) + time * wind_speed;
    let primary = sin(phase) * wind_strength;

    // Secondary gust (higher frequency, lower amplitude)
    let gust_phase = dot(world_pos.xz, wind_dir * 1.2) + time * wind_speed * 1.7;
    let gust = sin(gust_phase) * wind_strength * 0.3;

    // Tertiary micro-turbulence per blade
    let micro_phase = dot(world_pos.xz, vec2<f32>(3.7, 2.3)) + time * 2.5;
    let micro = sin(micro_phase) * wind_strength * 0.1;

    let total_wind = (primary + gust + micro) * height_factor * height_factor;

    world_pos.x += wind_dir.x * total_wind;
    world_pos.z += wind_dir.y * total_wind;

    out.world_position = world_pos;
    out.position = position_world_to_clip(world_pos.xyz);

#ifdef VERTEX_UVS_A
    out.uv = vertex_in.uv;
#endif

#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex_in.normal,
        vertex_in.instance_index
    );
#endif

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex_in.tangent,
        vertex_in.instance_index
    );
#endif

#ifdef VERTEX_COLORS
    out.color = vertex_in.color;
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex_in.instance_index;
#endif

    return out;
}
