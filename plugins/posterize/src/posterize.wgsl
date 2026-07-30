@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct PosterizeSettings {
    levels: f32,
};
@group(0) @binding(2) var<uniform> settings: PosterizeSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let levels = max(settings.levels, 2.0);
    let quantized = floor(color.rgb * levels) / (levels - 1.0);
    return vec4(clamp(quantized, vec3(0.0), vec3(1.0)), color.a);
}
