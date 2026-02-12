#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct DistortionSettings {
    intensity: f32,
    speed: f32,
    scale: f32,
    time: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
};
@group(0) @binding(2) var<uniform> settings: DistortionSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let t = settings.time * settings.speed;
    let offset = vec2(
        sin(in.uv.y * settings.scale + t) * settings.intensity,
        cos(in.uv.x * settings.scale + t * 0.7) * settings.intensity
    );
    let uv = clamp(in.uv + offset, vec2(0.0), vec2(1.0));
    return textureSample(screen_texture, texture_sampler, uv);
}
