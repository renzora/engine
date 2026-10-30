// @effect(name = "Sepia", icon = "drop-half")
//
// Sepia tone mapping. The simplest effect in the set, and the one worth
// reading if you are writing your own.
//
// The tone weights are a tuned constant rather than three sliders, so they
// stay in the struct and out of the Inspector.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct SepiaSettings {
    intensity: f32,  // @slider(0, 1, default = 1.0, step = 0.01)
    tone_r: f32,     // @hidden(default = 1.2)
    tone_g: f32,     // @hidden(default = 1.0)
    tone_b: f32,     // @hidden(default = 0.8)
};
@group(0) @binding(2) var<uniform> settings: SepiaSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let luma = dot(color.rgb, vec3(0.2126, 0.7152, 0.0722));
    let sepia = vec3(
        luma * settings.tone_r,
        luma * settings.tone_g,
        luma * settings.tone_b,
    );
    return vec4(mix(color.rgb, sepia, settings.intensity), color.a);
}
