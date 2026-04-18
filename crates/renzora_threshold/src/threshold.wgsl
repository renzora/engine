#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ThresholdSettings {
    threshold: f32,
    smoothness: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    _padding4: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: ThresholdSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let lum = dot(color.rgb, vec3(0.299, 0.587, 0.114));
    let bw = smoothstep(settings.threshold - settings.smoothness, settings.threshold + settings.smoothness, lum);
    return vec4(vec3(bw), color.a);
}
