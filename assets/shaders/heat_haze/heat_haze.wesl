// @effect(name = "Heat Haze", icon = "fire")
//
// The shimmer of hot air: a scrolling distortion that rises up the frame.
// `time` advances on its own.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct HeatHazeSettings {
    intensity: f32,  // @slider(0, 1, default = 0.15, step = 0.01)
    speed: f32,      // @slider(0.1, 10, default = 2, step = 0.1)
    scale: f32,      // @slider(1, 100, default = 20, step = 0.1)
    time: f32,
};
@group(0) @binding(2) var<uniform> settings: HeatHazeSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let t = settings.time * settings.speed;
    let offset_x = sin(in_uv.y * settings.scale + t) * settings.intensity * 0.01;
    let offset_y = cos(in_uv.x * settings.scale * 1.3 + t * 0.7) * settings.intensity * 0.01;
    let distorted_uv = clamp(in_uv + vec2(offset_x, offset_y), vec2(0.0), vec2(1.0));
    return textureSample(screen_texture, texture_sampler, distorted_uv);
}
