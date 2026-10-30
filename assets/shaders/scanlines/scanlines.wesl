// @effect(name = "Scanlines", icon = "rows")
//
// CRT scanlines: horizontal dark lines across the frame, optionally drifting.
//
// For the full CRT treatment (curvature, colour fringe and vignette sharing
// one curved UV), use CRT instead of stacking this with the others.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ScanlinesSettings {
    intensity: f32,  // @slider(0, 1, default = 0.15, step = 0.01)
    count: f32,      // @slider(10, 2000, default = 800, step = 10)
    speed: f32,      // @slider(0, 10, default = 0, step = 0.1)
};
@group(0) @binding(2) var<uniform> settings: ScanlinesSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let scanline = sin(in_uv.y * settings.count * 3.14159) * 0.5 + 0.5;
    let factor = 1.0 - settings.intensity * (1.0 - scanline);
    return vec4(color.rgb * factor, color.a);
}
