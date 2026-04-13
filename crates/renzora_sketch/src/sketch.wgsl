#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct SketchSettings {
    edge_strength: f32,
    paper_brightness: f32,
    line_density: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: SketchSettings;

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3(0.299, 0.587, 0.114));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let texel = vec2(1.0) / vec2<f32>(textureDimensions(screen_texture));
    let step = texel * settings.line_density;

    // Sobel edge detection
    let tl = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2(-step.x, -step.y)).rgb);
    let tc = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2(0.0, -step.y)).rgb);
    let tr = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2(step.x, -step.y)).rgb);
    let ml = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2(-step.x, 0.0)).rgb);
    let mr = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2(step.x, 0.0)).rgb);
    let bl = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2(-step.x, step.y)).rgb);
    let bc = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2(0.0, step.y)).rgb);
    let br = luminance(textureSample(screen_texture, texture_sampler, in.uv + vec2(step.x, step.y)).rgb);

    let gx = -tl - 2.0 * ml - bl + tr + 2.0 * mr + br;
    let gy = -tl - 2.0 * tc - tr + bl + 2.0 * bc + br;
    let edge = sqrt(gx * gx + gy * gy) * settings.edge_strength;

    let pencil = settings.paper_brightness - edge;
    return vec4(vec3(clamp(pencil, 0.0, 1.0)), color.a);
}
