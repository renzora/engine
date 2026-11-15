// Terrain blockout grid: the engine's shape grid, projected from world
// position (see `renzora_terrain::material`).
//
// One shader for both pipelines. Forward, it lights the grid itself. Under
// deferred rendering it writes the G-buffer instead, because that is all the
// deferred lighting pass and Solari read: drawn forward only, the grid was
// absent from the G-buffer, and Solari shaded it as a mirror.

import bevy_pbr::render::{
    pbr_functions,
    pbr_types::{PbrInput, pbr_input_new, STANDARD_MATERIAL_FLAGS_FOG_ENABLED_BIT},
    mesh_bindings::mesh,
    mesh_view_bindings::view,
};
@if(PREPASS_PIPELINE) import bevy_pbr::prepass::io::{VertexOutput, FragmentOutput};
@if(PREPASS_PIPELINE) import bevy_pbr::deferred::functions::deferred_output;
@if(!PREPASS_PIPELINE) import bevy_pbr::render::forward_io::{VertexOutput, FragmentOutput};

struct TerrainGridUniform {
    tint: vec4<f32>,
    // x = tiles per metre, y = metallic, z = roughness, w = 1 with the grid bound
    properties: vec4<f32>,
};

@group(3) @binding(0) var<uniform> material: TerrainGridUniform;
@group(3) @binding(1) var grid_color: texture_2d<f32>;
@group(3) @binding(2) var grid_sampler: sampler;
@group(3) @binding(3) var grid_normal: texture_2d<f32>;

/// The grid as a lit surface, for whichever pipeline is drawing it.
fn grid_input(in: VertexOutput) -> PbrInput {
    let world_position = in.world_position;
    let scale = material.properties.x;
    let world_pos = world_position.xyz;
    let n = normalize(in.world_normal);
    let an = abs(n);

    // Project along the axis the face points most along, so walls are ruled
    // as well as the top. `t` and `b` are the world directions the image's u
    // and v run along on that face, for orienting the groove normals.
    var uv: vec2<f32>;
    var t: vec3<f32>;
    var b: vec3<f32>;
    if an.y >= an.x && an.y >= an.z {
        uv = world_pos.xz;
        t = vec3<f32>(1.0, 0.0, 0.0);
        b = vec3<f32>(0.0, 0.0, 1.0);
    } else if an.x >= an.z {
        uv = vec2<f32>(world_pos.z, -world_pos.y);
        t = vec3<f32>(0.0, 0.0, 1.0);
        b = vec3<f32>(0.0, -1.0, 0.0);
    } else {
        uv = vec2<f32>(world_pos.x, -world_pos.y);
        t = vec3<f32>(1.0, 0.0, 0.0);
        b = vec3<f32>(0.0, -1.0, 0.0);
    }
    uv = uv * scale;

    var base = material.tint.rgb;
    var normal = n;
    if material.properties.w > 0.5 {
        base = base * textureSample(grid_color, grid_sampler, uv).rgb;
        // OpenGL-style tangent-space normal, bent onto the face's own frame.
        let ts = textureSample(grid_normal, grid_sampler, uv).xyz * 2.0 - 1.0;
        normal = normalize(t * ts.x + b * ts.y + n * ts.z);
    }

    var pbr_input: PbrInput = pbr_input_new();
    // The mesh's own flags, as a StandardMaterial reads them. A fresh input
    // has none, and without the shadow-receiver bit the sun's shadows never
    // fell on the grid, in either pipeline (the G-buffer carries the bit).
    pbr_input.flags = mesh[in.instance_index].flags;
    // Lights are found by screen tile, so the lighting needs the pixel's own
    // position; a fresh input says every pixel is the corner one.
    pbr_input.is_orthographic = view.clip_from_view[3].w == 1.0;
    pbr_input.frag_coord = in.position;
    pbr_input.material.base_color = vec4<f32>(base, 1.0);
    pbr_input.material.metallic = material.properties.y;
    pbr_input.material.perceptual_roughness = material.properties.z;
    // Fog is drawn only on materials that ask for it; a fresh input asks for
    // nothing, where a StandardMaterial asks by default. The G-buffer keeps
    // this flag, so the deferred lighting pass fogs the grid too.
    pbr_input.material.flags |= STANDARD_MATERIAL_FLAGS_FOG_ENABLED_BIT;
    pbr_input.world_normal = n;
    pbr_input.world_position = world_position;
    pbr_input.N = normal;
    pbr_input.V = pbr_functions::calculate_view(world_position, pbr_input.is_orthographic);
    return pbr_input;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let pbr_input = grid_input(in);

    @if(PREPASS_PIPELINE) let out = deferred_output(in, pbr_input);

    @if(!PREPASS_PIPELINE) var out: FragmentOutput;
    @if(!PREPASS_PIPELINE) var color = pbr_functions::apply_pbr_lighting(pbr_input);
    @if(!PREPASS_PIPELINE) color.a = 1.0;
    // Fog is applied here, after lighting, not by the lighting itself: returning
    // the lit colour straight left every terrain on the grid unfogged while
    // everything around it faded into the distance.
    @if(!PREPASS_PIPELINE) out.color = pbr_functions::main_pass_post_lighting_processing(pbr_input, color);

    return out;
}
