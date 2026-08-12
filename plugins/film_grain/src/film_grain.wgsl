@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct FilmGrainSettings {
    intensity: f32,
    grain_size: f32,
    time: f32,
};
@group(0) @binding(2) var<uniform> settings: FilmGrainSettings;

// Hoskins' hash13. The two-line hash this replaces stayed well distributed only
// for small inputs; fed a frame counter it drifted into visible diagonal streaks,
// because both of its magic constants pull in the same direction.
fn hash13(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.zyx + 31.32);
    return fract((q.x + q.y) * q.z);
}

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    // Grain cells are sized in pixels, off `pos.xy`. Sizing them in UV — which is
    // what this did — gave a grain that stretched with the aspect ratio and got
    // finer as the window grew, so the same scene grained differently in a docked
    // viewport and fullscreen.
    let cell = floor(pos.xy / max(settings.grain_size, 1.0));

    // Time is a separate hash axis, not an offset added to the coordinate. Adding
    // it only translated one fixed noise field, which reads as a sheet of dirt
    // sliding across the screen instead of as grain.
    //
    // Quantised to 24 Hz: real film re-rolls its grain every frame, but at render
    // framerates that reads as electronic static, and it would make the effect
    // look different on a fast machine than a slow one.
    let seed = floor(settings.time * 24.0);

    // Two taps averaged. One uniform hash is flat noise; the sum of two is
    // triangular, which is much closer to how grain actually clusters.
    let n = hash13(vec3(cell, seed)) + hash13(vec3(cell, seed + 17.0)) - 1.0;

    // Grain is emulsion modulating light that is already there, so it multiplies
    // rather than adds. The additive version lifted deep blacks to a flat grey
    // haze — the most obvious thing wrong with the effect — and washed out clipped
    // highlights. Response peaks in the midtones and falls off toward both ends,
    // as film does; the constant normalises that curve's peak to 1.0 so
    // `intensity` still means roughly what it did.
    let lum = clamp(dot(color.rgb, vec3(0.2126, 0.7152, 0.0722)), 0.0, 1.0);
    let response = sqrt(lum) * (1.0 - lum) * 2.598;

    return vec4(color.rgb * (1.0 + n * settings.intensity * response), color.a);
}
