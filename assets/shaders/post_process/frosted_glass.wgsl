#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct FrostedGlassSettings {
    intensity: f32,
    scale: f32,
    _p1: f32,
    _p2: f32,
    _p3: f32,
    _p4: f32,
    _p5: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: FrostedGlassSettings;

// Hash-based pseudo-random value in [0, 1) for a vec2 seed.
fn hash2(p: vec2<f32>) -> vec2<f32> {
    var q = vec2<f32>(
        dot(p, vec2<f32>(127.1, 311.7)),
        dot(p, vec2<f32>(269.5, 183.3)),
    );
    return fract(sin(q) * 43758.5453123);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    // Generate a noise-based UV offset
    let noise_uv = in.uv * settings.scale;
    let noise = hash2(floor(noise_uv)) + hash2(floor(noise_uv) + vec2<f32>(1.0, 0.0));
    // Map noise from [0,2) to [-1, 1)
    let offset = (noise - 1.0) * settings.intensity;

    let displaced_uv = clamp(in.uv + offset, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(screen_texture, texture_sampler, displaced_uv);
}
