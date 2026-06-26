// Splash post-process overlay: a fullscreen UI node that finishes the synthwave
// look with a coloured vignette — darkened corners with a soft magenta glow frame
// just inside them. It outputs a colour with a varying alpha and alpha-blends over
// the whole composite behind it. (A UI overlay can't read the framebuffer, so true
// bloom/chromatic/scanline passes aren't possible here; this is the reliable,
// on-theme finish.) params.x = time.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct CrtUniforms {
    params: vec4<f32>, // x = time, y = width(px), z = height(px)
};

@group(1) @binding(0)
var<uniform> u: CrtUniforms;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let t = u.params.x;

    // Distance from centre (slightly wider than tall).
    let d = length((uv - vec2<f32>(0.5)) * vec2<f32>(1.12, 1.0));

    // Darkening that ramps up toward the corners.
    let darken = smoothstep(0.50, 1.05, d) * 0.55;

    // A magenta glow ring sitting just inside the darkened edge — gently pulsing.
    let ring = exp(-pow((d - 0.66) / 0.13, 2.0));
    let pulse = 0.85 + 0.15 * sin(t * 1.5);
    let glow = ring * 0.30 * pulse;

    // Where the glow dominates, the source colour leans magenta (tints the frame);
    // at the extreme corners it leans black (darkens).
    let frame_col = vec3<f32>(0.55, 0.06, 0.62) * glow;
    let a = clamp(darken + glow, 0.0, 0.85);

    return vec4<f32>(frame_col, a);
}
