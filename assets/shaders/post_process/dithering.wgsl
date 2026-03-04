#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct DitheringSettings {
    color_depth: f32,
    intensity: f32,
    _p1: f32,
    _p2: f32,
    _p3: f32,
    _p4: f32,
    _p5: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: DitheringSettings;

// 4x4 Bayer matrix, normalized to [0, 1) range (values / 16.0)
fn bayer4x4(px: vec2<u32>) -> f32 {
    let bayer = array<f32, 16>(
         0.0 / 16.0,  8.0 / 16.0,  2.0 / 16.0, 10.0 / 16.0,
        12.0 / 16.0,  4.0 / 16.0, 14.0 / 16.0,  6.0 / 16.0,
         3.0 / 16.0, 11.0 / 16.0,  1.0 / 16.0,  9.0 / 16.0,
        15.0 / 16.0,  7.0 / 16.0, 13.0 / 16.0,  5.0 / 16.0,
    );
    let x = px.x % 4u;
    let y = px.y % 4u;
    return bayer[y * 4u + x];
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let tex_size = vec2<f32>(textureDimensions(screen_texture));
    let px = vec2<u32>(in.uv * tex_size);

    let threshold = bayer4x4(px);
    // Scale threshold to one quantization step, modulated by intensity
    let levels = max(settings.color_depth, 2.0);
    let step = 1.0 / levels;
    let dithered = color.rgb + (threshold - 0.5) * step * settings.intensity;
    let quantized = floor(dithered * levels + 0.5) / levels;

    return vec4<f32>(clamp(quantized, vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
}
