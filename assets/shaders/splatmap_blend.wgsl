// Splatmap Blend Fragment Shader (PBR fallback)
// Blends up to 4 material layers using an RGBA weight texture with PBR lighting.

#import bevy_pbr::{
    pbr_functions::apply_pbr_lighting,
    pbr_types::PbrInput,
    pbr_types::pbr_input_new,
    forward_io::VertexOutput,
}

// Each uniform field gets its own binding, matching the Rust AsBindGroup layout
@group(3) @binding(0) var<uniform> layer_colors_0: vec4<f32>;
@group(3) @binding(1) var<uniform> layer_colors_1: vec4<f32>;
@group(3) @binding(2) var<uniform> layer_colors_2: vec4<f32>;
@group(3) @binding(3) var<uniform> layer_colors_3: vec4<f32>;
@group(3) @binding(4) var<uniform> layer_props_0: vec4<f32>;
@group(3) @binding(5) var<uniform> layer_props_1: vec4<f32>;
@group(3) @binding(6) var<uniform> layer_props_2: vec4<f32>;
@group(3) @binding(7) var<uniform> layer_props_3: vec4<f32>;
@group(3) @binding(8) var splatmap_texture: texture_2d<f32>;
@group(3) @binding(9) var splatmap_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample splatmap weights at mesh UV
    let weights = textureSample(splatmap_texture, splatmap_sampler, in.uv);
    let total = weights.r + weights.g + weights.b + weights.a + 0.001;
    let w = weights / total;

    // Blend base colors by weight
    let blended_color = layer_colors_0 * w.r
                      + layer_colors_1 * w.g
                      + layer_colors_2 * w.b
                      + layer_colors_3 * w.a;

    // Blend metallic/roughness from layer props (x=metallic, y=roughness)
    let blended_metallic = layer_props_0.x * w.r + layer_props_1.x * w.g
                         + layer_props_2.x * w.b + layer_props_3.x * w.a;
    let blended_roughness = layer_props_0.y * w.r + layer_props_1.y * w.g
                          + layer_props_2.y * w.b + layer_props_3.y * w.a;

    // PBR lighting
    var pbr_input: PbrInput = pbr_input_new();
    pbr_input.material.base_color = blended_color;
    pbr_input.material.metallic = blended_metallic;
    pbr_input.material.perceptual_roughness = blended_roughness;
    pbr_input.diffuse_occlusion = vec3<f32>(1.0);
    pbr_input.world_normal = normalize(in.world_normal);
    pbr_input.world_position = in.world_position;
    pbr_input.frag_coord = in.position;

    var color = apply_pbr_lighting(pbr_input);
    color.a = 1.0;
    return color;
}
