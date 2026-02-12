#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct VignetteSettings {
    intensity: f32,
    radius: f32,
    smoothness: f32,
    color_r: f32,
    color_g: f32,
    color_b: f32,
    _padding1: f32,
    _padding2: f32,
};
@group(0) @binding(2) var<uniform> settings: VignetteSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    let center = vec2(0.5, 0.5);
    let dist = distance(in.uv, center);
    let vignette = smoothstep(settings.radius, settings.radius - settings.smoothness, dist);
    let tint = vec3(settings.color_r, settings.color_g, settings.color_b);
    let result = mix(tint, color.rgb, mix(1.0, vignette, settings.intensity));
    return vec4(result, color.a);
}
