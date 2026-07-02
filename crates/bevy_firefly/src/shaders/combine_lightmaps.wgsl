#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0)
var main_lightmap: texture_2d<f32>;

@group(0) @binding(1)
var visibility_lightmap: texture_2d<f32>;

@group(0) @binding(2)
var texture_sampler: sampler;

@fragment
fn fragment(vo: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return vec4f(1.0);
    // return textureSample(main_lightmap, texture_sampler, vo.uv) * textureSample(visibility_lightmap, texture_sampler, vo.uv);
}
