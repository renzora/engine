// Splatmap Blend Fragment Shader (startup fallback)
// Replaced immediately by the generated shader from shader_gen.

#import bevy_pbr::forward_io::VertexOutput

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
    return vec4<f32>(0.5, 0.5, 0.5, 1.0);
}
