// @effect(name = "Tilt Shift", icon = "camera")
//
// Blurs the top and bottom of the frame, leaving a sharp band in the middle,
// which reads as a miniature.
//
// A screen-space fake rather than a real depth of field: the focus band is a
// horizontal strip at `focus_position`, so it costs nothing to sample the
// depth buffer and works on a 2D scene that has none. For real depth of
// field, use the engine's own Depth of Field.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct TiltShiftSettings {
    blur_amount: f32,     // @slider(0, 10, default = 3.0, step = 0.1)
    focus_position: f32,  // @slider(0, 1, default = 0.5, step = 0.01)
    focus_width: f32,     // @slider(0.01, 0.5, default = 0.1, step = 0.01)
    focus_falloff: f32,   // @slider(0.01, 0.5, default = 0.15, step = 0.01)
};
@group(0) @binding(2) var<uniform> settings: TiltShiftSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let dist_from_focus = abs(in_uv.y - settings.focus_position);
    let blur_factor = smoothstep(settings.focus_width, settings.focus_width + settings.focus_falloff, dist_from_focus);

    if blur_factor < 0.01 {
        return color;
    }

    let texel = vec2(1.0) / vec2<f32>(textureDimensions(screen_texture));
    let r = settings.blur_amount * blur_factor;

    var result = vec3(0.0);
    var total = 0.0;
    for (var y = -3; y <= 3; y++) {
        for (var x = -3; x <= 3; x++) {
            let w = 1.0 / (1.0 + f32(x * x + y * y));
            let offset = vec2(f32(x), f32(y)) * texel * r;
            result += textureSample(screen_texture, texture_sampler, in_uv + offset).rgb * w;
            total += w;
        }
    }
    result /= total;

    // Slight saturation boost for miniature look
    let avg = dot(result, vec3(0.333));
    let saturated = mix(vec3(avg), result, 1.2);

    return vec4(clamp(saturated, vec3(0.0), vec3(1.0)), color.a);
}
