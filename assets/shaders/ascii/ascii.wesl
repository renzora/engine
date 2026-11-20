// @effect(name = "ASCII", icon = "text-aa")
//
// ASCII art post-process effect. The character shapes are generated in the
// shader from luminance rather than sampled from a font atlas, so there is no
// texture to ship and the cell size can be any number rather than a multiple
// of a glyph.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Settings {
    char_size: f32,  // @slider(2, 32, default = 8, step = 0.5)
    color_mix: f32,  // @slider(0, 1, default = 0.5, step = 0.01)
    contrast: f32,   // @slider(0.5, 3, default = 1.2, step = 0.01)
};
@group(0) @binding(2) var<uniform> settings: Settings;

// Procedural character patterns based on luminance
fn char_pattern(uv: vec2<f32>, lum: f32) -> f32 {
    let p = uv;

    // Different density patterns for different brightness levels
    if lum > 0.9 {
        return 1.0; // Full bright - solid
    }
    if lum > 0.7 {
        // Hash #
        let h = step(0.35, abs(p.x - 0.3)) * step(0.35, abs(p.x - 0.7));
        let v = step(0.35, abs(p.y - 0.3)) * step(0.35, abs(p.y - 0.7));
        return 1.0 - h * v;
    }
    if lum > 0.5 {
        // Plus +
        let h = step(abs(p.y - 0.5), 0.12);
        let v = step(abs(p.x - 0.5), 0.12);
        return max(h, v);
    }
    if lum > 0.3 {
        // Dash -
        return step(abs(p.y - 0.5), 0.1) * step(abs(p.x - 0.5), 0.3);
    }
    if lum > 0.15 {
        // Dot .
        return 1.0 - step(0.15, length(p - vec2(0.5, 0.7)));
    }
    return 0.0; // Space
}

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let dims = vec2<f32>(textureDimensions(screen_texture));
    let cell = vec2(settings.char_size) / dims;
    let cell_center = (floor(in_uv / cell) + 0.5) * cell;
    let cell_color = textureSample(screen_texture, texture_sampler, cell_center);

    let lum = clamp(dot(cell_color.rgb, vec3(0.299, 0.587, 0.114)) * settings.contrast, 0.0, 1.0);
    let local_uv = fract(in_uv / cell);
    let pattern = char_pattern(local_uv, lum);

    let mono = vec3(pattern);
    let tinted = cell_color.rgb * pattern;
    let result = mix(mono, tinted, settings.color_mix);
    return vec4(result, color.a);
}
