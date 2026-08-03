@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct SepiaSettings {
    intensity: f32,
    tone_r: f32,
    tone_g: f32,
    tone_b: f32,
};
@group(0) @binding(2) var<uniform> settings: SepiaSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let luma = dot(color.rgb, vec3(0.2126, 0.7152, 0.0722));
    let sepia = vec3(
        luma * settings.tone_r,
        luma * settings.tone_g,
        luma * settings.tone_b,
    );
    return vec4(mix(color.rgb, sepia, settings.intensity), color.a);
}
