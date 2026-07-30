@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ThresholdSettings {
    threshold: f32,
    smoothness: f32,
};
@group(0) @binding(2) var<uniform> settings: ThresholdSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let lum = dot(color.rgb, vec3(0.299, 0.587, 0.114));
    let bw = smoothstep(settings.threshold - settings.smoothness, settings.threshold + settings.smoothness, lum);
    return vec4(vec3(bw), color.a);
}
