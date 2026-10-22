// Node-graph square grid background. Painted on a full-viewport UI node behind
// the canvas; pan/zoom come from the canvas's transform so the lines track the
// nodes 1:1 (the canvas scales around its centre = the viewport centre).
//
// Two levels, the way Unreal's material graph draws it: a faint line every grid
// step (the step nodes snap to) and a stronger one every `major` steps, so the
// eye can measure across a large graph without counting dots.
//
// Everything is in physical pixels: `in.size` is the node's physical size, and
// `pan`/`spacing`/`line width` are supplied pre-scaled by the sync system.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct GridUniforms {
    view: vec4<f32>, // x = zoom, yz = pan (phys px), w = grid spacing (phys px)
    size: vec4<f32>, // x = line width (phys px), y = major every N steps
    bg: vec4<f32>,   // canvas background (linear rgb)
    dot: vec4<f32>,  // minor line colour (linear rgb); major is twice as far from bg
};

@group(1) @binding(0) var<uniform> u: GridUniforms;

// Coverage of a 1D line family at canvas coordinate `c` with period `period`
// (canvas px), drawn `w` screen px wide.
fn lines(c: vec2<f32>, period: f32, zoom: f32, w: f32) -> f32 {
    let g = c / period;
    let off = abs(g - round(g)) * period * zoom; // screen px to the nearest line
    let d = min(off.x, off.y);
    return 1.0 - smoothstep(w * 0.5, w * 0.5 + 1.0, d);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let zoom = max(u.view.x, 0.0001);
    let pan = u.view.yz;
    let spacing = max(u.view.w, 1.0);
    let w = max(u.size.x, 1.0);
    let major_n = max(u.size.y, 1.0);

    let dims = in.size;
    let px = in.uv * dims;
    let center = dims * 0.5;
    let p = center + (px - center - pan) / zoom;

    // Minor lines fade out when zoomed far out, before they would merge into
    // a flat wash; the major lines carry the grid from there.
    let minor_fade = clamp((spacing * zoom - 6.0) / 8.0, 0.0, 1.0);
    let minor = lines(p, spacing, zoom, w) * minor_fade;
    let major = lines(p, spacing * major_n, zoom, w);

    let major_col = u.dot.rgb + (u.dot.rgb - u.bg.rgb);
    var col = mix(u.bg.rgb, u.dot.rgb, minor);
    col = mix(col, major_col, major);
    return vec4<f32>(col, 1.0);
}
