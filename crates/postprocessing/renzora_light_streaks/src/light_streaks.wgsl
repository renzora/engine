#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct LightStreaksSettings {
    intensity: f32,
    threshold: f32,
    samples: f32,
    direction: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: LightStreaksSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let dir = vec2(cos(settings.direction), sin(settings.direction));
    let texel = vec2(1.0) / vec2<f32>(textureDimensions(screen_texture));
    let num = max(i32(settings.samples), 1);

    var streak = vec3(0.0);
    for (var i = 0; i < num; i++) {
        let t = f32(i) / f32(num);
        let offset = dir * t * 0.1;
        let s = textureSample(screen_texture, texture_sampler, in.uv + offset).rgb;
        let lum = dot(s, vec3(0.299, 0.587, 0.114));
        if lum > settings.threshold {
            let weight = 1.0 - t;
            streak += s * weight;
        }
    }
    streak /= f32(num) * 0.5;

    return vec4(color.rgb + streak * settings.intensity, color.a);
}
