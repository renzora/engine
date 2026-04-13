#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct EdgeGlowSettings {
    threshold: f32,
    glow_intensity: f32,
    color_r: f32,
    color_g: f32,
    color_b: f32,
    _p1: f32,
    _p2: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: EdgeGlowSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let tex_size = vec2<f32>(textureDimensions(screen_texture));
    let texel = 1.0 / tex_size;

    // Sample 3x3 neighbourhood for Sobel edge detection
    let tl = dot(textureSample(screen_texture, texture_sampler, in.uv + vec2(-texel.x, -texel.y)).rgb, vec3(0.299, 0.587, 0.114));
    let tc = dot(textureSample(screen_texture, texture_sampler, in.uv + vec2( 0.0,     -texel.y)).rgb, vec3(0.299, 0.587, 0.114));
    let tr = dot(textureSample(screen_texture, texture_sampler, in.uv + vec2( texel.x, -texel.y)).rgb, vec3(0.299, 0.587, 0.114));
    let cl = dot(textureSample(screen_texture, texture_sampler, in.uv + vec2(-texel.x,  0.0    )).rgb, vec3(0.299, 0.587, 0.114));
    let cr = dot(textureSample(screen_texture, texture_sampler, in.uv + vec2( texel.x,  0.0    )).rgb, vec3(0.299, 0.587, 0.114));
    let bl = dot(textureSample(screen_texture, texture_sampler, in.uv + vec2(-texel.x,  texel.y)).rgb, vec3(0.299, 0.587, 0.114));
    let bc = dot(textureSample(screen_texture, texture_sampler, in.uv + vec2( 0.0,      texel.y)).rgb, vec3(0.299, 0.587, 0.114));
    let br = dot(textureSample(screen_texture, texture_sampler, in.uv + vec2( texel.x,  texel.y)).rgb, vec3(0.299, 0.587, 0.114));

    // Sobel kernels
    let gx = -tl - 2.0 * cl - bl + tr + 2.0 * cr + br;
    let gy = -tl - 2.0 * tc - tr + bl + 2.0 * bc + br;
    let edge_magnitude = sqrt(gx * gx + gy * gy);

    // Apply threshold and multiply by glow color + intensity
    let edge = max(edge_magnitude - settings.threshold, 0.0);
    let glow_color = vec3(settings.color_r, settings.color_g, settings.color_b);
    let glow = glow_color * edge * settings.glow_intensity;

    // Additive blend onto original scene
    let result = color.rgb + glow;
    return vec4(result, color.a);
}
