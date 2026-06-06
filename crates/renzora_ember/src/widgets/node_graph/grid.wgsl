// Node-graph dotted grid background. Painted on a full-viewport UI node behind
// the canvas; pan/zoom come from the canvas's transform so the dots track the
// nodes 1:1 (the canvas scales around its centre = the viewport centre).
//
// Everything is in physical pixels: `in.size` is the node's physical size, and
// `pan`/`spacing`/`dot radius` are supplied pre-scaled by the sync system.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct GridUniforms {
    view: vec4<f32>, // x = zoom, yz = pan (phys px), w = grid spacing (phys px)
    size: vec4<f32>, // x = dot radius (phys px)
    bg: vec4<f32>,   // canvas background (linear rgb)
    dot: vec4<f32>,  // dot colour (linear rgb)
};

@group(1) @binding(0) var<uniform> u: GridUniforms;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let zoom = max(u.view.x, 0.0001);
    let pan = u.view.yz;
    let spacing = max(u.view.w, 1.0);

    let dims = in.size;
    let px = in.uv * dims;            // fragment, phys px from top-left
    let center = dims * 0.5;
    // Canvas-local position of this fragment (inverse of the canvas transform).
    let p = center + (px - center - pan) / zoom;

    // Distance (screen px) to the nearest grid intersection.
    let g = p / spacing;
    let off = (g - round(g)) * spacing;
    let d = length(off) * zoom;

    let r = max(u.size.x, 0.5);
    let a = 1.0 - smoothstep(r, r + 1.5, d);
    let col = mix(u.bg.rgb, u.dot.rgb, a);
    return vec4<f32>(col, 1.0);
}
