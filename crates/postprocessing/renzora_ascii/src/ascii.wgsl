#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct AsciiSettings {
    char_size: f32,
    color_mix: f32,
    contrast: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: AsciiSettings;

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
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let dims = vec2<f32>(textureDimensions(screen_texture));
    let cell = vec2(settings.char_size) / dims;
    let cell_center = (floor(in.uv / cell) + 0.5) * cell;
    let cell_color = textureSample(screen_texture, texture_sampler, cell_center);

    let lum = clamp(dot(cell_color.rgb, vec3(0.299, 0.587, 0.114)) * settings.contrast, 0.0, 1.0);
    let local_uv = fract(in.uv / cell);
    let pattern = char_pattern(local_uv, lum);

    let mono = vec3(pattern);
    let tinted = cell_color.rgb * pattern;
    let result = mix(mono, tinted, settings.color_mix);
    return vec4(result, color.a);
}
