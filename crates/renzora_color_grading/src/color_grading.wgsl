#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ColorGradingSettings {
    brightness: f32,
    contrast: f32,
    saturation: f32,
    gamma: f32,
    temperature: f32,
    tint: f32,
    _padding1: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: ColorGradingSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    // Brightness
    var rgb = color.rgb * settings.brightness;

    // Contrast (pivot at 0.5)
    rgb = (rgb - vec3(0.5)) * settings.contrast + vec3(0.5);

    // Saturation
    let luma = dot(rgb, vec3(0.2126, 0.7152, 0.0722));
    rgb = mix(vec3(luma), rgb, settings.saturation);

    // Gamma
    rgb = pow(max(rgb, vec3(0.0)), vec3(1.0 / settings.gamma));

    // Temperature (warm/cool shift)
    rgb.r += settings.temperature * 0.1;
    rgb.b -= settings.temperature * 0.1;

    // Tint (green/magenta shift)
    rgb.g += settings.tint * 0.1;

    return vec4(clamp(rgb, vec3(0.0), vec3(1.0)), color.a);
}
