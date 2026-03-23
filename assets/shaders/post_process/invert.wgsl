#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct InvertSettings {
    intensity: f32,
    _p1: f32,
    _p2: f32,
    _p3: f32,
    _p4: f32,
    _p5: f32,
    _p6: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: InvertSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let inverted = vec3(1.0) - color.rgb;
    return vec4(mix(color.rgb, inverted, settings.intensity), color.a);
}
