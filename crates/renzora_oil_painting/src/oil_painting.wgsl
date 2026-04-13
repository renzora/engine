#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct OilPaintingSettings {
    radius: f32,
    levels: f32,
    _p1: f32,
    _p2: f32,
    _p3: f32,
    _p4: f32,
    _p5: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: OilPaintingSettings;

// Simplified Kuwahara filter.
// Samples a neighbourhood of (2*r+1)^2 pixels, quantizes luminance into buckets,
// picks the most-populated bucket, and outputs the average color of that bucket.
@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let tex_size = vec2<f32>(textureDimensions(screen_texture));
    let texel = 1.0 / tex_size;

    // Clamp radius to [1, 4] to avoid GPU timeout
    let r = clamp(i32(settings.radius), 1, 4);
    let num_levels = max(i32(settings.levels), 2);

    // Accumulators: one sum + count per luminance bucket (max 32 buckets)
    var bucket_color = array<vec3<f32>, 32>();
    var bucket_count = array<i32, 32>();

    for (var dy = -r; dy <= r; dy++) {
        for (var dx = -r; dx <= r; dx++) {
            let offset = vec2<f32>(f32(dx), f32(dy)) * texel;
            let s = textureSample(screen_texture, texture_sampler, in.uv + offset).rgb;
            let lum = dot(s, vec3(0.299, 0.587, 0.114));
            let bucket = clamp(i32(lum * f32(num_levels - 1) + 0.5), 0, num_levels - 1);
            bucket_color[bucket] = bucket_color[bucket] + s;
            bucket_count[bucket] = bucket_count[bucket] + 1;
        }
    }

    // Find bucket with most samples
    var best_bucket = 0;
    var best_count = 0;
    for (var b = 0; b < num_levels; b++) {
        if bucket_count[b] > best_count {
            best_count = bucket_count[b];
            best_bucket = b;
        }
    }

    let out_color = bucket_color[best_bucket] / f32(max(best_count, 1));
    return vec4(clamp(out_color, vec3(0.0), vec3(1.0)), color.a);
}
