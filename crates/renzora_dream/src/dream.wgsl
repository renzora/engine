#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct DreamSettings {
    intensity: f32,
    blur_radius: f32,
    threshold: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: DreamSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let texel = vec2(1.0) / vec2<f32>(textureDimensions(screen_texture));
    let r = settings.blur_radius;

    // Simple box blur for glow
    var glow = vec3(0.0);
    var count = 0.0;
    for (var y = -2; y <= 2; y++) {
        for (var x = -2; x <= 2; x++) {
            let offset = vec2(f32(x), f32(y)) * texel * r;
            let s = textureSample(screen_texture, texture_sampler, in.uv + offset).rgb;
            let lum = dot(s, vec3(0.299, 0.587, 0.114));
            if lum > settings.threshold {
                glow += s;
            }
            count += 1.0;
        }
    }
    glow /= count;

    // Soft additive blend with desaturation
    let dreamy = color.rgb + glow * settings.intensity;
    let avg = dot(dreamy, vec3(0.333));
    let result = mix(dreamy, vec3(avg), settings.intensity * 0.3);
    return vec4(clamp(result, vec3(0.0), vec3(1.0)), color.a);
}
