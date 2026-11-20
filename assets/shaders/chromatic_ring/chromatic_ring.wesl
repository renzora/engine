// @effect(name = "Chromatic Ring", icon = "circle-dashed")
//
// Radial chromatic aberration: the colour split grows with distance from the
// centre of the frame rather than running one way across it, which is what a
// real lens does. `radius` is where it starts and `falloff` how quickly it
// climbs from there.
//
// For a flat lateral split, use **Chromatic Aberration**.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Settings {
    intensity: f32,  // @slider(0, 0.05, default = 0.008, step = 0.001)
    radius: f32,     // @slider(0, 2, default = 0.8, step = 0.01)
    falloff: f32,    // @slider(0.01, 1, default = 0.4, step = 0.01)
};
@group(0) @binding(2) var<uniform> settings: Settings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let center = vec2(0.5);
    let delta = in_uv - center;
    let dist = length(delta);

    // Chromatic split increases with distance from center, focused at radius
    let ring_factor = smoothstep(settings.radius - settings.falloff, settings.radius, dist);
    let offset = normalize(delta) * ring_factor * settings.intensity;

    let r = textureSample(screen_texture, texture_sampler, clamp(in_uv + offset, vec2(0.0), vec2(1.0))).r;
    let g = color.g;
    let b = textureSample(screen_texture, texture_sampler, clamp(in_uv - offset, vec2(0.0), vec2(1.0))).b;

    return vec4(r, g, b, color.a);
}
