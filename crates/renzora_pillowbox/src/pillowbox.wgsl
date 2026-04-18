#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct PillowboxSettings {
    bar_width: f32,
    softness: f32,
    aspect_ratio: f32,
    _p1: f32,
    _p2: f32,
    _p3: f32,
    _p4: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: PillowboxSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);

    if settings.enabled < 0.5 {
        return color;
    }

    // Compute bar width: use aspect ratio if set, otherwise manual bar_width
    var bar: f32;
    if settings.aspect_ratio > 0.01 {
        let dims = vec2<f32>(textureDimensions(screen_texture, 0));
        let screen_ratio = dims.x / dims.y;
        // Pillarbox: bars appear when target ratio is narrower than screen
        bar = max(0.0, (1.0 - settings.aspect_ratio / screen_ratio) * 0.5);
    } else {
        bar = settings.bar_width;
    }

    let soft = settings.softness;

    // Compute distance from each side edge
    let dist_left = in.uv.x;
    let dist_right = 1.0 - in.uv.x;
    let dist = min(dist_left, dist_right);

    // Create mask: 0 in bars, 1 in visible area
    var mask: f32;
    if soft > 0.001 {
        mask = smoothstep(bar - soft * bar, bar + soft * bar, dist);
    } else {
        mask = step(bar, dist);
    }

    return vec4(color.rgb * mask, color.a);
}
