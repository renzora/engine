// @effect(name = "Color Grading", icon = "sliders")
//
// Whole-frame colour correction: exposure and contrast, saturation, gamma,
// and a temperature/tint pair for warming or cooling the picture.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Settings {
    brightness: f32,   // @slider(0, 3, default = 1, step = 0.01)
    contrast: f32,     // @slider(0, 3, default = 1, step = 0.01)
    saturation: f32,   // @slider(0, 3, default = 1, step = 0.01)
    gamma: f32,        // @slider(0.1, 3, default = 1, step = 0.01)
    temperature: f32,  // @slider(-1, 1, default = 0, step = 0.01)
    tint: f32,         // @slider(-1, 1, default = 0, step = 0.01)
};
@group(0) @binding(2) var<uniform> settings: Settings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

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
