@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ChromaticAberrationSettings {
    intensity: f32,
    samples: f32,
    direction_x: f32,
    direction_y: f32,
};
@group(0) @binding(2) var<uniform> settings: ChromaticAberrationSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let dir = normalize(vec2(settings.direction_x, settings.direction_y)) * settings.intensity;
    let num_samples = max(i32(settings.samples), 1);

    var r = 0.0;
    var g = 0.0;
    var b = 0.0;

    for (var i = 0; i < num_samples; i++) {
        let t = f32(i) / f32(num_samples - 1 + i32(num_samples == 1)) - 0.5;
        let offset = dir * t;
        r += textureSample(screen_texture, texture_sampler, in_uv + offset).r;
        g += textureSample(screen_texture, texture_sampler, in_uv).g;
        b += textureSample(screen_texture, texture_sampler, in_uv - offset).b;
    }

    let s = f32(num_samples);
    return vec4(r / s, g / s, b / s, color.a);
}
