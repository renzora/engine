// Default Blueprint Material Shader
// This shader is used as a fallback when no custom shader is generated
// It provides basic PBR-like rendering with texture support

#import bevy_pbr::{
    pbr_functions::pbr,
    pbr_types::PbrInput,
    pbr_types::pbr_input_new,
    mesh_view_bindings::view,
    mesh_view_bindings::globals,
    forward_io::VertexOutput,
}

// Material uniforms
struct BlueprintMaterialUniform {
    base_color: vec4<f32>,
};

@group(2) @binding(0) var<uniform> material: BlueprintMaterialUniform;

// Optional textures
@group(2) @binding(1) var base_color_texture: texture_2d<f32>;
@group(2) @binding(2) var base_color_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample base color texture if available, otherwise use uniform color
    var base_color = material.base_color;

    // Sample texture and multiply with base color
    let tex_color = textureSample(base_color_texture, base_color_sampler, in.uv);
    base_color = base_color * tex_color;

    // Set up PBR input
    var pbr_input: PbrInput = pbr_input_new();
    pbr_input.material.base_color = base_color;
    pbr_input.material.metallic = 0.0;
    pbr_input.material.perceptual_roughness = 0.5;
    pbr_input.material.emissive = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    pbr_input.occlusion = vec3<f32>(1.0);
    pbr_input.world_normal = normalize(in.world_normal);
    pbr_input.world_position = vec4<f32>(in.world_position, 1.0);
    pbr_input.frag_coord = in.position;

    // Calculate PBR lighting
    var color = pbr(pbr_input);
    color.a = base_color.a;

    return color;
}
