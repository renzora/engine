#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct NightVisionSettings {
    intensity: f32,
    noise_amount: f32,
    scanline_amount: f32,
    color_amplification: f32,
    _p1: f32,
    _p2: f32,
    time: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: NightVisionSettings;

fn hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3(p.xyx) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    // Convert to luminance and amplify
    let luma = dot(color.rgb, vec3(0.2126, 0.7152, 0.0722));
    let amplified = clamp(luma * settings.color_amplification, 0.0, 1.0);

    // Green tint
    var night = vec3(amplified * 0.2, amplified, amplified * 0.2);

    // Animated noise
    let noise_uv = in.uv + vec2(settings.time * 0.07, settings.time * 0.13);
    let noise = hash(noise_uv * 512.0) * 2.0 - 1.0;
    night = night + vec3(noise * settings.noise_amount);

    // Scanlines
    let scanline = sin(in.uv.y * 800.0) * 0.5 + 0.5;
    night = night * (1.0 - settings.scanline_amount * (1.0 - scanline));

    night = clamp(night, vec3(0.0), vec3(1.0));
    return vec4(mix(color.rgb, night, settings.intensity), color.a);
}
