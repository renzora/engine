#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct GrayscaleSettings {
    intensity: f32,
    luminance_r: f32,
    luminance_g: f32,
    luminance_b: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: GrayscaleSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let luma = dot(color.rgb, vec3(settings.luminance_r, settings.luminance_g, settings.luminance_b));
    let gray = vec3(luma);
    return vec4(mix(color.rgb, gray, settings.intensity), color.a);
}
