#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct PixelationSettings {
    pixel_size: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    _padding4: f32,
    _padding5: f32,
    _padding6: f32,
};
@group(0) @binding(2) var<uniform> settings: PixelationSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(screen_texture));
    let pixel_count = dims / max(settings.pixel_size, 1.0);
    let quantized_uv = floor(in.uv * pixel_count) / pixel_count;
    return textureSample(screen_texture, texture_sampler, quantized_uv);
}
