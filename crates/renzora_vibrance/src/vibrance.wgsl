#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct VibranceSettings {
    intensity: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    _padding4: f32,
    _padding5: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: VibranceSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let mx = max(color.r, max(color.g, color.b));
    let avg = (color.r + color.g + color.b) / 3.0;
    // Saturation is low when max ~= avg
    let sat = mx - avg;
    // Boost less-saturated pixels more (smart vibrance)
    let boost = settings.intensity * (1.0 - sat) * (mx - avg);
    let result = color.rgb + (color.rgb - vec3(avg)) * boost;
    return vec4(clamp(result, vec3(0.0), vec3(1.0)), color.a);
}
