// @effect(name = "Dream", icon = "cloud")
//
// A dreamy soft glow: a thresholded blur mixed back over the picture.
//
// Unlike bloom it keeps the blurred copy visible in the midtones rather than
// only where the image is bright, which is what gives it a hazy look instead
// of a highlight sheen.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Settings {
    intensity: f32,    // @slider(0, 1, default = 0.4, step = 0.01)
    blur_radius: f32,  // @slider(1, 10, default = 3, step = 0.1)
    threshold: f32,    // @slider(0, 1, default = 0.5, step = 0.01)
};
@group(0) @binding(2) var<uniform> settings: Settings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let texel = vec2(1.0) / vec2<f32>(textureDimensions(screen_texture));
    let r = settings.blur_radius;

    // Simple box blur for glow
    var glow = vec3(0.0);
    var count = 0.0;
    for (var y = -2; y <= 2; y++) {
        for (var x = -2; x <= 2; x++) {
            let offset = vec2(f32(x), f32(y)) * texel * r;
            let s = textureSample(screen_texture, texture_sampler, in_uv + offset).rgb;
            let lum = dot(s, vec3(0.299, 0.587, 0.114));
            if lum > settings.threshold {
                glow += s;
            }
            count += 1.0;
        }
    }
    glow /= count;

    // Soft additive blend with desaturation
    let dreamy = color.rgb + glow * settings.intensity;
    let avg = dot(dreamy, vec3(0.333));
    let result = mix(dreamy, vec3(avg), settings.intensity * 0.3);
    return vec4(clamp(result, vec3(0.0), vec3(1.0)), color.a);
}
