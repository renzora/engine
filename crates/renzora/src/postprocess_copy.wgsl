// Passthrough blit: samples the input and writes it unchanged. Used by the
// unified post-process node to copy the live frame into a per-view snapshot
// texture for two-image transition effects.
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return textureSample(screen_texture, texture_sampler, in.uv);
}
