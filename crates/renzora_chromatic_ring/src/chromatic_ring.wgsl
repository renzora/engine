#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ChromaticRingSettings {
    intensity: f32,
    radius: f32,
    falloff: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: ChromaticRingSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let center = vec2(0.5);
    let delta = in.uv - center;
    let dist = length(delta);

    // Chromatic split increases with distance from center, focused at radius
    let ring_factor = smoothstep(settings.radius - settings.falloff, settings.radius, dist);
    let offset = normalize(delta) * ring_factor * settings.intensity;

    let r = textureSample(screen_texture, texture_sampler, clamp(in.uv + offset, vec2(0.0), vec2(1.0))).r;
    let g = color.g;
    let b = textureSample(screen_texture, texture_sampler, clamp(in.uv - offset, vec2(0.0), vec2(1.0))).b;

    return vec4(r, g, b, color.a);
}
