// @effect(name = "CRT", icon = "television-simple")
//
// Screen curvature, scanlines, a colour fringe and a vignette in one pass.
// Each exists separately in this set, but a CRT wants them sharing a curved
// UV: the scanlines have to bend with the glass, and stacking four passes
// would leave them flat over a curved picture.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Settings {
    scanline_intensity: f32,  // @slider(0, 2, default = 0.3, step = 0.01)
    curvature: f32,           // @slider(0, 1, default = 0.02, step = 0.01)
    chromatic_amount: f32,    // @slider(0, 0.1, default = 0.003, step = 0.001)
    vignette_amount: f32,     // @slider(0, 2, default = 0.5, step = 0.01)
};
@group(0) @binding(2) var<uniform> settings: Settings;

fn curve_uv(uv: vec2<f32>, curvature: f32) -> vec2<f32> {
    var curved = uv * 2.0 - 1.0;
    let offset = abs(curved.yx) / vec2(6.0, 4.0);
    curved = curved + curved * offset * offset * curvature;
    return curved * 0.5 + 0.5;
}

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);
    let dims = vec2<f32>(textureDimensions(screen_texture));
    let uv = curve_uv(in_uv, settings.curvature * 10.0);

    // Out of bounds check
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
        return vec4(0.0, 0.0, 0.0, 1.0);
    }

    // Chromatic aberration
    let ca = settings.chromatic_amount;
    let r = textureSample(screen_texture, texture_sampler, uv + vec2(ca, 0.0)).r;
    let g = textureSample(screen_texture, texture_sampler, uv).g;
    let b = textureSample(screen_texture, texture_sampler, uv - vec2(ca, 0.0)).b;
    var result = vec3(r, g, b);

    // Scanlines
    let scanline = sin(uv.y * dims.y * 3.14159) * 0.5 + 0.5;
    result = result * mix(1.0, scanline, settings.scanline_intensity);

    // CRT vignette
    let dist = distance(uv, vec2(0.5));
    let vig = smoothstep(0.8, 0.4, dist);
    result = result * mix(1.0, vig, settings.vignette_amount);

    return vec4(result, 1.0);
}
