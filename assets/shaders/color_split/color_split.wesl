// @effect(name = "Color Split", icon = "arrows-out-line-horizontal")
//
// Directional RGB split. Red and blue are pushed opposite ways along one axis
// and green is left where it is, which is why there are only two offsets to
// author. `angle` is in radians, so its maximum is a full turn.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Settings {
    offset_r: f32,  // @slider(0, 0.05, default = 0.005, step = 0.001)
    offset_b: f32,  // @slider(0, 0.05, default = 0.005, step = 0.001)
    angle: f32,     // @slider(0, 6.283, default = 0, step = 0.01)
};
@group(0) @binding(2) var<uniform> settings: Settings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let dir = vec2(cos(settings.angle), sin(settings.angle));
    let r = textureSample(screen_texture, texture_sampler, clamp(in_uv + dir * settings.offset_r, vec2(0.0), vec2(1.0))).r;
    let g = color.g;
    let b = textureSample(screen_texture, texture_sampler, clamp(in_uv - dir * settings.offset_b, vec2(0.0), vec2(1.0))).b;
    return vec4(r, g, b, color.a);
}
