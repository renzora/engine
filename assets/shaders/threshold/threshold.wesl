// @effect(name = "Threshold", icon = "circle-half-tilt")
//
// Cuts the frame to black and white at a brightness threshold.
//
// `smoothness` is what keeps the cut from aliasing: at zero the split is a
// hard step and every edge in the picture stairsteps, so the default eases it
// over a narrow band either side.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ThresholdSettings {
    threshold: f32,   // @slider(0, 1, default = 0.5, step = 0.01)
    smoothness: f32,  // @slider(0, 0.5, default = 0.05, step = 0.01)
};
@group(0) @binding(2) var<uniform> settings: ThresholdSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let lum = dot(color.rgb, vec3(0.299, 0.587, 0.114));
    let bw = smoothstep(settings.threshold - settings.smoothness, settings.threshold + settings.smoothness, lum);
    return vec4(vec3(bw), color.a);
}
