@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ScanlinesSettings {
    intensity: f32,
    count: f32,
    speed: f32,
};
@group(0) @binding(2) var<uniform> settings: ScanlinesSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let scanline = sin(in_uv.y * settings.count * 3.14159) * 0.5 + 0.5;
    let factor = 1.0 - settings.intensity * (1.0 - scanline);
    return vec4(color.rgb * factor, color.a);
}
