@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ThermalSettings {
    intensity: f32,
    contrast: f32,
    cold_threshold: f32,
};
@group(0) @binding(2) var<uniform> settings: ThermalSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let luminance = dot(color.rgb, vec3(0.299, 0.587, 0.114));
    let heat = clamp((luminance - settings.cold_threshold) * settings.contrast, 0.0, 1.0);

    // Cold (blue/purple) -> warm (yellow/red) -> hot (white)
    var thermal: vec3<f32>;
    if heat < 0.25 {
        let t = heat / 0.25;
        thermal = mix(vec3(0.0, 0.0, 0.3), vec3(0.2, 0.0, 0.8), t);
    } else if heat < 0.5 {
        let t = (heat - 0.25) / 0.25;
        thermal = mix(vec3(0.2, 0.0, 0.8), vec3(0.9, 0.1, 0.1), t);
    } else if heat < 0.75 {
        let t = (heat - 0.5) / 0.25;
        thermal = mix(vec3(0.9, 0.1, 0.1), vec3(1.0, 0.9, 0.0), t);
    } else {
        let t = (heat - 0.75) / 0.25;
        thermal = mix(vec3(1.0, 0.9, 0.0), vec3(1.0, 1.0, 1.0), t);
    }

    let result = mix(color.rgb, thermal, settings.intensity);
    return vec4(result, color.a);
}
