#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct SwirlSettings {
    angle: f32,
    radius: f32,
    center_x: f32,
    center_y: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: SwirlSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let center = vec2(settings.center_x, settings.center_y);
    let delta = in.uv - center;
    let dist = length(delta);
    let factor = max(1.0 - dist / settings.radius, 0.0);
    let theta = factor * factor * settings.angle;
    let s = sin(theta);
    let c = cos(theta);
    let rotated = vec2(c * delta.x - s * delta.y, s * delta.x + c * delta.y);
    let new_uv = clamp(rotated + center, vec2(0.0), vec2(1.0));
    return textureSample(screen_texture, texture_sampler, new_uv);
}
