#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ColorSplitSettings {
    offset_r: f32,
    offset_b: f32,
    angle: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: ColorSplitSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let dir = vec2(cos(settings.angle), sin(settings.angle));
    let r = textureSample(screen_texture, texture_sampler, clamp(in.uv + dir * settings.offset_r, vec2(0.0), vec2(1.0))).r;
    let g = color.g;
    let b = textureSample(screen_texture, texture_sampler, clamp(in.uv - dir * settings.offset_b, vec2(0.0), vec2(1.0))).b;
    return vec4(r, g, b, color.a);
}
