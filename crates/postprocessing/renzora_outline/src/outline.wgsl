#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct OutlineSettings {
    thickness: f32,
    threshold: f32,
    color_r: f32,
    color_g: f32,
    color_b: f32,
    mix_mode: f32,
    _padding1: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: OutlineSettings;

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3(0.2126, 0.7152, 0.0722));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let tex_size = vec2<f32>(textureDimensions(screen_texture));
    let texel = settings.thickness / tex_size;

    // Sample 8 neighbors for Sobel edge detection
    let tl = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2(-texel.x,  texel.y)).rgb);
    let tc = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2( 0.0,      texel.y)).rgb);
    let tr = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2( texel.x,  texel.y)).rgb);
    let ml = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2(-texel.x,  0.0    )).rgb);
    let mr = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2( texel.x,  0.0    )).rgb);
    let bl = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2(-texel.x, -texel.y)).rgb);
    let bc = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2( 0.0,     -texel.y)).rgb);
    let br = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2( texel.x, -texel.y)).rgb);

    // Sobel kernels
    let gx = (-1.0 * tl) + (1.0 * tr)
           + (-2.0 * ml) + (2.0 * mr)
           + (-1.0 * bl) + (1.0 * br);

    let gy = ( 1.0 * tl) + (2.0 * tc) + ( 1.0 * tr)
           + (-1.0 * bl) + (-2.0 * bc) + (-1.0 * br);

    let edge_magnitude = sqrt(gx * gx + gy * gy);
    let is_edge = step(settings.threshold, edge_magnitude);

    let outline_color = vec3(settings.color_r, settings.color_g, settings.color_b);

    // mix_mode=0: overlay edges on scene, mix_mode=1: show edges only on black background
    let scene_contribution = mix(color.rgb, vec3(0.0), settings.mix_mode);
    let result = mix(scene_contribution, outline_color, is_edge);

    return vec4(result, color.a);
}
