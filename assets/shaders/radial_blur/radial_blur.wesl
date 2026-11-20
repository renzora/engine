// @effect(name = "Radial Blur", icon = "sun-dim")
//
// Streaks the frame outward from a point, for speed and impact shots.
//
// Unlike Swirl and Kaleidoscope, the centre is authored: a radial blur is
// usually aimed at something, so the point it streaks away from is the whole
// control.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct RadialBlurSettings {
    intensity: f32,  // @slider(0, 0.2, default = 0.02, step = 0.001)
    center_x: f32,   // @slider(0, 1, default = 0.5, step = 0.01)
    center_y: f32,   // @slider(0, 1, default = 0.5, step = 0.01)
    samples: f32,    // @slider(4, 32, default = 8, step = 1)
};
@group(0) @binding(2) var<uniform> settings: RadialBlurSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let center = vec2<f32>(settings.center_x, settings.center_y);
    let dir = in_uv - center;
    let num_samples = max(1.0, settings.samples);

    var acc = vec4<f32>(0.0);
    for (var i = 0.0; i < num_samples; i = i + 1.0) {
        let t = i / (num_samples - 1.0);
        let sample_uv = in_uv - dir * settings.intensity * t;
        let sample_uv_c = clamp(sample_uv, vec2<f32>(0.0), vec2<f32>(1.0));
        acc = acc + textureSample(screen_texture, texture_sampler, sample_uv_c);
    }

    return acc / num_samples;
}
