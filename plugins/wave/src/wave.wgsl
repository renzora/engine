@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct WaveSettings {
    amplitude: f32,
    frequency: f32,
    speed: f32,
    time: f32,
};
@group(0) @binding(2) var<uniform> settings: WaveSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let t = settings.time * settings.speed;
    let offset_x = sin(in_uv.y * settings.frequency + t) * settings.amplitude;
    let offset_y = sin(in_uv.x * settings.frequency * 1.3 + t * 0.8) * settings.amplitude;
    let new_uv = clamp(in_uv + vec2(offset_x, offset_y), vec2(0.0), vec2(1.0));
    return textureSample(screen_texture, texture_sampler, new_uv);
}
