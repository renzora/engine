// @effect(name = "Chromatic Aberration", icon = "rainbow")
//
// Splits the colour channels across the frame, the way a cheap lens does. The
// direction is a fixed horizontal split rather than a setting: a directional
// aberration reads as a camera fault, and the one people reach for is lateral.
//
// For the radial version, where the split grows with distance from the
// centre, use **Chromatic Ring**.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Settings {
    intensity: f32,    // @slider(0, 0.1, default = 0.005, step = 0.001)
    samples: f32,      // @slider(1, 16, default = 3, step = 1)
    direction_x: f32,  // @hidden(default = 1)
    direction_y: f32,  // @hidden(default = 0)
};
@group(0) @binding(2) var<uniform> settings: Settings;

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
