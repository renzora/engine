@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct PixelationSettings {
    pixel_size: f32,
};
@group(0) @binding(2) var<uniform> settings: PixelationSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);
    let dims = vec2<f32>(textureDimensions(screen_texture));
    let pixel_count = dims / max(settings.pixel_size, 1.0);
    let quantized_uv = floor(in_uv * pixel_count) / pixel_count;
    return textureSample(screen_texture, texture_sampler, quantized_uv);
}
