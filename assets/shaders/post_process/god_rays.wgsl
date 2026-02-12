#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct GodRaysSettings {
    intensity: f32,
    decay: f32,
    density: f32,
    num_samples: u32,
    light_pos_x: f32,
    light_pos_y: f32,
    _padding1: f32,
    _padding2: f32,
};
@group(0) @binding(2) var<uniform> settings: GodRaysSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let light_pos = vec2(settings.light_pos_x, settings.light_pos_y);
    let delta = (in.uv - light_pos) * settings.density / f32(settings.num_samples);

    var uv = in.uv;
    var illumination_decay = 1.0;
    var accumulated = vec3(0.0);

    let samples = min(settings.num_samples, 128u);
    for (var i = 0u; i < samples; i = i + 1u) {
        uv = uv - delta;
        let sample_color = textureSample(screen_texture, texture_sampler, uv).rgb;
        // Use luminance threshold to identify bright areas
        let lum = dot(sample_color, vec3(0.299, 0.587, 0.114));
        let bright = max(lum - 0.7, 0.0) * sample_color;
        accumulated = accumulated + bright * illumination_decay;
        illumination_decay = illumination_decay * settings.decay;
    }

    let color = textureSample(screen_texture, texture_sampler, in.uv);
    return vec4(color.rgb + accumulated * settings.intensity, color.a);
}
