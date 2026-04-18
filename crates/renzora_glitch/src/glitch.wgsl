#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct GlitchSettings {
    intensity: f32,
    block_size: f32,
    color_drift: f32,
    speed: f32,
    _p1: f32,
    _p2: f32,
    time: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: GlitchSettings;

fn hash1(x: f32) -> f32 {
    let s = fract(x * 127.1 + 311.7);
    return fract(s * (s + 19.19) * s);
}

fn hash2(p: vec2<f32>) -> f32 {
    let p2 = fract(p * vec2<f32>(443.897, 441.423));
    return fract((p2.x + p2.y) * (p2.x * p2.y + 19.19));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let t = settings.time * settings.speed;

    // Intermittent glitch: only active a fraction of the time
    let is_active = step(0.85, hash1(floor(t * 0.5)));

    // Quantize Y to block rows
    let row = floor(in.uv.y * settings.block_size);

    // Per-block horizontal displacement
    let block_t = floor(t);
    let disp_raw = hash2(vec2<f32>(row, block_t)) * 2.0 - 1.0;
    // Only displace strong glitch rows
    let is_glitch_row = step(0.75, hash2(vec2<f32>(row, block_t + 0.5)));
    let disp = disp_raw * is_glitch_row * settings.intensity * is_active;

    let uv_r = vec2<f32>(in.uv.x + disp + settings.color_drift, in.uv.y);
    let uv_g = vec2<f32>(in.uv.x + disp, in.uv.y);
    let uv_b = vec2<f32>(in.uv.x + disp - settings.color_drift, in.uv.y);

    // Clamp UVs to [0,1]
    let uv_r_c = clamp(uv_r, vec2<f32>(0.0), vec2<f32>(1.0));
    let uv_g_c = clamp(uv_g, vec2<f32>(0.0), vec2<f32>(1.0));
    let uv_b_c = clamp(uv_b, vec2<f32>(0.0), vec2<f32>(1.0));

    let r = textureSample(screen_texture, texture_sampler, uv_r_c).r;
    let g = textureSample(screen_texture, texture_sampler, uv_g_c).g;
    let b = textureSample(screen_texture, texture_sampler, uv_b_c).b;

    return vec4<f32>(r, g, b, color.a);
}
