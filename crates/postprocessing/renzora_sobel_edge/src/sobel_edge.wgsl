#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct SobelEdgeSettings {
    intensity: f32,
    threshold: f32,
    color_r: f32,
    color_g: f32,
    color_b: f32,
    _padding1: f32,
    _padding2: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: SobelEdgeSettings;

fn lum(c: vec3<f32>) -> f32 {
    return dot(c, vec3(0.299, 0.587, 0.114));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let texel = vec2(1.0) / vec2<f32>(textureDimensions(screen_texture));

    let tl = lum(textureSample(screen_texture, texture_sampler, in.uv + vec2(-texel.x, -texel.y)).rgb);
    let tc = lum(textureSample(screen_texture, texture_sampler, in.uv + vec2(0.0, -texel.y)).rgb);
    let tr = lum(textureSample(screen_texture, texture_sampler, in.uv + vec2(texel.x, -texel.y)).rgb);
    let ml = lum(textureSample(screen_texture, texture_sampler, in.uv + vec2(-texel.x, 0.0)).rgb);
    let mr = lum(textureSample(screen_texture, texture_sampler, in.uv + vec2(texel.x, 0.0)).rgb);
    let bl = lum(textureSample(screen_texture, texture_sampler, in.uv + vec2(-texel.x, texel.y)).rgb);
    let bc = lum(textureSample(screen_texture, texture_sampler, in.uv + vec2(0.0, texel.y)).rgb);
    let br = lum(textureSample(screen_texture, texture_sampler, in.uv + vec2(texel.x, texel.y)).rgb);

    let gx = -tl - 2.0 * ml - bl + tr + 2.0 * mr + br;
    let gy = -tl - 2.0 * tc - tr + bl + 2.0 * bc + br;
    let edge = sqrt(gx * gx + gy * gy) * settings.intensity;

    let edge_color = vec3(settings.color_r, settings.color_g, settings.color_b);
    if edge > settings.threshold {
        return vec4(edge_color * min(edge, 1.0), color.a);
    }
    return vec4(vec3(0.0), color.a);
}
