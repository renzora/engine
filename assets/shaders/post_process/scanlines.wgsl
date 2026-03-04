#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ScanlinesSettings {
    intensity: f32,
    count: f32,
    speed: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    _padding4: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: ScanlinesSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let scanline = sin(in.uv.y * settings.count * 3.14159) * 0.5 + 0.5;
    let factor = 1.0 - settings.intensity * (1.0 - scanline);
    return vec4(color.rgb * factor, color.a);
}
