#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct RainSettings {
    intensity: f32,
    speed: f32,
    drop_size: f32,
    time: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: RainSettings;

fn hash2(p: vec2<f32>) -> vec2<f32> {
    let q = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
    return fract(sin(q) * 43758.5453);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let t = settings.time * settings.speed;
    let grid = in.uv * settings.drop_size;
    let cell = floor(grid);
    let local = fract(grid);

    var distortion = vec2(0.0);

    // Check 3x3 neighborhood for drops
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let neighbor = cell + vec2(f32(x), f32(y));
            let h = hash2(neighbor);
            // Drop position within cell, animated vertically
            let drop_pos = vec2(h.x, fract(h.y - t * (0.5 + h.x * 0.5)));
            let delta = local - drop_pos - vec2(f32(x), f32(y));
            let dist = length(delta);
            let drop_radius = 0.15 + h.x * 0.1;
            if dist < drop_radius {
                let strength = (1.0 - dist / drop_radius);
                distortion += delta * strength * settings.intensity * 0.05;
            }
        }
    }

    let distorted_uv = clamp(in.uv + distortion, vec2(0.0), vec2(1.0));
    return textureSample(screen_texture, texture_sampler, distorted_uv);
}
