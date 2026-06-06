// Glitch post-effect for the splash city render. Samples the city image and,
// driven by `params.y` (0 = clean .. 1 = full glitch), applies horizontal slice
// jitter, chromatic RGB split and scanline flicker. At intensity 0 it returns
// the image unchanged. Alpha is preserved so the transparent sky keeps showing
// the grid/network behind it.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct GlitchUniforms {
    params: vec4<f32>, // x = time (s), y = intensity (0..1)
};

@group(1) @binding(0) var<uniform> u: GlitchUniforms;
@group(1) @binding(1) var tex: texture_2d<f32>;
@group(1) @binding(2) var tex_sampler: sampler;

fn rand(p: f32) -> f32 {
    return fract(sin(p * 12.9898) * 43758.5453);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let t = u.params.x;
    let g = u.params.y;

    // Horizontal slice jitter (a few rows shoved sideways).
    let slice = floor(in.uv.y * 26.0);
    let jitter = (rand(slice + floor(t * 36.0)) - 0.5) * 0.10 * g;
    let ux = clamp(in.uv.x + jitter, 0.0, 1.0);

    // Chromatic RGB split.
    let sep = 0.015 * g;
    let cr = textureSample(tex, tex_sampler, vec2<f32>(clamp(ux + sep, 0.0, 1.0), in.uv.y));
    let cg = textureSample(tex, tex_sampler, vec2<f32>(ux, in.uv.y));
    let cb = textureSample(tex, tex_sampler, vec2<f32>(clamp(ux - sep, 0.0, 1.0), in.uv.y));
    var col = vec3<f32>(cr.r, cg.g, cb.b);

    // Scanline brightness flicker.
    let flick = 1.0 + (rand(slice * 2.3 + floor(t * 50.0)) - 0.5) * 0.6 * g;
    col = col * flick;

    return vec4<f32>(col, cg.a);
}
