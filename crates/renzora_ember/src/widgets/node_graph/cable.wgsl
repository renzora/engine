// Node-graph cable: a stroked cubic bezier rendered as a signed-distance field
// on a UI node. `uv * size` gives the fragment's pixel inside the (full-viewport)
// node, which matches the control points supplied in viewport-local pixels.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct CableUniforms {
    // p0.xy, p1.xy
    ab: vec4<f32>,
    // p2.xy, p3.xy
    cd: vec4<f32>,
    color: vec4<f32>,
    // Colour at the cable's far end; equal to `color` for same-type wires.
    color_b: vec4<f32>,
    // x = stroke width (px), y = feather (px)
    params: vec4<f32>,
};

@group(1) @binding(0)
var<uniform> u: CableUniforms;

fn cubic(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>, t: f32) -> vec2<f32> {
    let m = 1.0 - t;
    return p0 * (m * m * m)
         + p1 * (3.0 * m * m * t)
         + p2 * (3.0 * m * t * t)
         + p3 * (t * t * t);
}

fn seg_dist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-5), 0.0, 1.0);
    return length(pa - ba * h);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv * in.size;
    let p0 = u.ab.xy;
    let p1 = u.ab.zw;
    let p2 = u.cd.xy;
    let p3 = u.cd.zw;
    let width = u.params.x;
    let feather = max(u.params.y, 0.001);

    // Distance to the curve, approximated by a dense polyline of the cubic.
    // `best_t` tracks the curve parameter at the closest point so the stroke
    // can run a gradient from the source pin's colour to the target's.
    var prev = p0;
    var prev_t = 0.0;
    var d = 1.0e9;
    var best_t = 0.0;
    let n = 30;
    for (var i = 1; i <= n; i = i + 1) {
        let t = f32(i) / f32(n);
        let cur = cubic(p0, p1, p2, p3, t);
        let sd = seg_dist(p, prev, cur);
        if sd < d {
            d = sd;
            // Interpolate within the segment: the closest point sits partway
            // between prev_t and t in proportion to how the distances fall off.
            best_t = (prev_t + t) * 0.5;
        }
        prev = cur;
        prev_t = t;
    }

    let edge = d - width * 0.5;
    let aa = max(fwidth(d), feather);
    let alpha = 1.0 - smoothstep(-aa, aa, edge);
    if alpha <= 0.0 {
        discard;
    }
    let rgb = mix(u.color.rgb, u.color_b.rgb, best_t);
    return vec4<f32>(rgb, u.color.a * alpha);
}
