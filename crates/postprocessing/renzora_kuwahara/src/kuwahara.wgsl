#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct KuwaharaSettings {
    radius: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    _padding4: f32,
    _padding5: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: KuwaharaSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let texel = vec2(1.0) / vec2<f32>(textureDimensions(screen_texture));
    let r = i32(settings.radius);

    // Compute mean and variance for 4 quadrants, pick lowest variance
    var best_color = vec3(0.0);
    var best_var = 999.0;

    for (var qy = 0; qy < 2; qy++) {
        for (var qx = 0; qx < 2; qx++) {
            var sum = vec3(0.0);
            var sum_sq = vec3(0.0);
            var count = 0.0;

            let start_x = select(0, -r, qx == 0);
            let end_x = select(r, 0, qx == 0);
            let start_y = select(0, -r, qy == 0);
            let end_y = select(r, 0, qy == 0);

            for (var y = start_y; y <= end_y; y++) {
                for (var x = start_x; x <= end_x; x++) {
                    let s = textureSample(screen_texture, texture_sampler, in.uv + vec2(f32(x), f32(y)) * texel).rgb;
                    sum += s;
                    sum_sq += s * s;
                    count += 1.0;
                }
            }

            let mean = sum / count;
            let variance = dot(sum_sq / count - mean * mean, vec3(1.0));

            if variance < best_var {
                best_var = variance;
                best_color = mean;
            }
        }
    }

    return vec4(best_color, color.a);
}
