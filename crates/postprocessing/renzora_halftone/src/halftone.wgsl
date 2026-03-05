#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct HalftoneSettings {
    dot_size: f32,
    angle: f32,
    intensity: f32,
    _p1: f32,
    _p2: f32,
    _p3: f32,
    _p4: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: HalftoneSettings;

fn rotate2d(v: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(v.x * c - v.y * s, v.x * s + v.y * c);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    // Luminance of the original pixel
    let luma = dot(color.rgb, vec3<f32>(0.299, 0.587, 0.114));

    // Get pixel coordinates and rotate by angle
    // We use a normalized UV scaled by dot_size (in "cells")
    // To get pixel-space coordinates we need the texture dimensions.
    // We approximate using uv directly scaled by an arbitrary resolution factor.
    // Using uv * 512 / dot_size gives ~dot_size pixel cells at 512px reference.
    let scale = 512.0 / settings.dot_size;
    let rotated = rotate2d(in.uv * scale, settings.angle);

    // Cell coordinate and fractional position within cell
    let cell = floor(rotated);
    let frac = fract(rotated) - 0.5; // center at 0

    // Sample the original color at the center of this cell (rotated back)
    let cell_center_rotated = (cell + 0.5) / scale;
    let cell_center_uv = rotate2d(cell_center_rotated, -settings.angle);
    let cell_center_uv_c = clamp(cell_center_uv, vec2<f32>(0.0), vec2<f32>(1.0));
    let cell_color = textureSample(screen_texture, texture_sampler, cell_center_uv_c);
    let cell_luma = dot(cell_color.rgb, vec3<f32>(0.299, 0.587, 0.114));

    // Dot radius proportional to luminance (brighter = larger dot)
    let radius = cell_luma * 0.5;

    // Distance from cell center
    let dist = length(frac);

    // Dot mask: 1 inside dot, 0 outside
    let dot_mask = step(dist, radius);

    // Mix: dot areas show cell color, gaps show black (halftone look)
    let halftone = mix(vec3<f32>(0.0), cell_color.rgb, dot_mask);

    // Blend halftone with original based on intensity
    let result = mix(color.rgb, halftone, settings.intensity);

    return vec4<f32>(result, color.a);
}
