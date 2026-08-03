@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct InvertSettings {
    intensity: f32,
};
@group(0) @binding(2) var<uniform> settings: InvertSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let inverted = vec3(1.0) - color.rgb;
    return vec4(mix(color.rgb, inverted, settings.intensity), color.a);
}
