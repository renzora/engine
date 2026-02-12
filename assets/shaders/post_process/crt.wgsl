#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct CrtSettings {
    scanline_intensity: f32,
    curvature: f32,
    chromatic_amount: f32,
    vignette_amount: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
};
@group(0) @binding(2) var<uniform> settings: CrtSettings;

fn curve_uv(uv: vec2<f32>, curvature: f32) -> vec2<f32> {
    var curved = uv * 2.0 - 1.0;
    let offset = abs(curved.yx) / vec2(6.0, 4.0);
    curved = curved + curved * offset * offset * curvature;
    return curved * 0.5 + 0.5;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(screen_texture));
    let uv = curve_uv(in.uv, settings.curvature * 10.0);

    // Out of bounds check
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
        return vec4(0.0, 0.0, 0.0, 1.0);
    }

    // Chromatic aberration
    let ca = settings.chromatic_amount;
    let r = textureSample(screen_texture, texture_sampler, uv + vec2(ca, 0.0)).r;
    let g = textureSample(screen_texture, texture_sampler, uv).g;
    let b = textureSample(screen_texture, texture_sampler, uv - vec2(ca, 0.0)).b;
    var color = vec3(r, g, b);

    // Scanlines
    let scanline = sin(uv.y * dims.y * 3.14159) * 0.5 + 0.5;
    color = color * mix(1.0, scanline, settings.scanline_intensity);

    // CRT vignette
    let dist = distance(uv, vec2(0.5));
    let vig = smoothstep(0.8, 0.4, dist);
    color = color * mix(1.0, vig, settings.vignette_amount);

    return vec4(color, 1.0);
}
