#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct HeatHazeSettings {
    intensity: f32,
    speed: f32,
    scale: f32,
    time: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: HeatHazeSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let t = settings.time * settings.speed;
    let offset_x = sin(in.uv.y * settings.scale + t) * settings.intensity * 0.01;
    let offset_y = cos(in.uv.x * settings.scale * 1.3 + t * 0.7) * settings.intensity * 0.01;
    let distorted_uv = clamp(in.uv + vec2(offset_x, offset_y), vec2(0.0), vec2(1.0));
    return textureSample(screen_texture, texture_sampler, distorted_uv);
}
