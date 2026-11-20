// @effect(name = "Pulse", icon = "pulse")
//
// A pulsing vignette: the edges of the frame darken and brighten on their own,
// for heartbeats, low health and anything that should feel like strain.
//
// For a still vignette, use Vignette.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct PulseSettings {
    strength: f32,  // @slider(0, 2, default = 0.6, step = 0.01)
    speed: f32,     // @slider(0, 10, default = 2, step = 0.01)
    time: f32,
};
@group(0) @binding(2) var<uniform> settings: PulseSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let c = textureSample(screen_texture, texture_sampler, uv);
    let d = distance(uv, vec2<f32>(0.5, 0.5));
    // The engine supplies plain seconds; the plugin used to advance its clock
    // by `speed` per second, so the scaling happens here instead.
    let wave = sin(settings.time * settings.speed) * 0.5 + 0.5;
    let vignette = 1.0 - d * settings.strength * wave * 2.0;
    return vec4<f32>(c.rgb * clamp(vignette, 0.0, 1.0), c.a);
}
