#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct GaussianBlurSettings {
    sigma: f32,
    kernel_size: u32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    _padding4: f32,
    _padding5: f32,
};
@group(0) @binding(2) var<uniform> settings: GaussianBlurSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(screen_texture));
    let texel = 1.0 / dims;

    let half_size = i32(min(settings.kernel_size, 15u)) / 2;
    let sigma = max(settings.sigma, 0.001);
    let inv_2sigma2 = 1.0 / (2.0 * sigma * sigma);

    var total = vec4(0.0);
    var weight_sum = 0.0;

    // Single-pass box approximation (sample both axes)
    for (var y = -half_size; y <= half_size; y = y + 1) {
        for (var x = -half_size; x <= half_size; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y));
            let dist2 = dot(offset, offset);
            let w = exp(-dist2 * inv_2sigma2);
            total = total + textureSample(screen_texture, texture_sampler, in.uv + offset * texel) * w;
            weight_sum = weight_sum + w;
        }
    }

    return total / weight_sum;
}
