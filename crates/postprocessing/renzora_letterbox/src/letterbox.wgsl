#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct LetterboxSettings {
    bar_height: f32,
    softness: f32,
    aspect_ratio: f32,
    _p1: f32,
    _p2: f32,
    _p3: f32,
    _p4: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: LetterboxSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);

    if settings.enabled < 0.5 {
        return color;
    }

    // Compute bar height: use aspect ratio if set, otherwise manual bar_height
    var bar: f32;
    if settings.aspect_ratio > 0.01 {
        let dims = vec2<f32>(textureDimensions(screen_texture, 0));
        let screen_ratio = dims.x / dims.y;
        // Letterbox: bars appear when target ratio is wider than screen
        bar = max(0.0, (1.0 - screen_ratio / settings.aspect_ratio) * 0.5);
    } else {
        bar = settings.bar_height;
    }

    let soft = settings.softness;

    // Compute distance from each horizontal edge
    let dist_top = in.uv.y;
    let dist_bottom = 1.0 - in.uv.y;
    let dist = min(dist_top, dist_bottom);

    // Create mask: 0 in bars, 1 in visible area
    var mask: f32;
    if soft > 0.001 {
        mask = smoothstep(bar - soft * bar, bar + soft * bar, dist);
    } else {
        mask = step(bar, dist);
    }

    return vec4(color.rgb * mask, color.a);
}
