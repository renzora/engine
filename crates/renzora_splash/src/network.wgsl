// Particle network — drifting nodes joined by soft lines, behind the loading
// terminal. Computed in pixel space (params.yz = the node's size) so dots stay
// round and lines crisp at any resolution.
#import bevy_ui::ui_vertex_output::UiVertexOutput

struct NetUniforms {
    params: vec4<f32>, // x = time, y = width px, z = height px
};
@group(1) @binding(0) var<uniform> u: NetUniforms;

const N: i32 = 14;

/// Node position in 0..1, gently wandering (smooth + bounded — no wrap jumps, so
/// the links never flicker).
fn net_node(i: i32, t: f32) -> vec2<f32> {
    let fi = f32(i);
    let x = 0.5 + 0.42 * sin(t * (0.06 + fi * 0.007) + fi * 1.7);
    let y = 0.5 + 0.42 * sin(t * (0.05 + fi * 0.009) + fi * 2.7 + 1.1);
    return vec2<f32>(x, y);
}

fn seg_dist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1.0e-4), 0.0, 1.0);
    return length(pa - ba * h);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let t = u.params.x;
    let res = max(vec2<f32>(u.params.y, u.params.z), vec2<f32>(1.0));
    let pix = in.uv * res;
    let max_link = 0.26 * max(res.x, res.y);

    var pts: array<vec2<f32>, 14>;
    for (var i = 0; i < N; i = i + 1) {
        pts[i] = net_node(i, t) * res;
    }

    var line_glow = 0.0;
    var dot_glow = 0.0;
    for (var i = 0; i < N; i = i + 1) {
        let qi = pts[i];
        dot_glow = max(dot_glow, 1.0 - smoothstep(0.5, 2.6, distance(pix, qi)));
        for (var j = i + 1; j < N; j = j + 1) {
            let qj = pts[j];
            let link = distance(qi, qj);
            let fade = 1.0 - smoothstep(0.0, max_link, link); // closer = stronger
            if (fade > 0.0) {
                let sd = seg_dist(pix, qi, qj);
                line_glow = max(line_glow, (1.0 - smoothstep(0.0, 1.4, sd)) * fade);
            }
        }
    }

    let base = vec3<f32>(0.012, 0.022, 0.018);    // near-black, matches backdrop
    let line_col = vec3<f32>(0.16, 0.66, 0.85);   // cyan
    let dot_col = vec3<f32>(0.55, 1.0, 0.85);     // bright teal nodes
    var color = base;
    color = mix(color, line_col, line_glow * 0.5);
    color = mix(color, dot_col, dot_glow * 0.9);
    return vec4<f32>(pow(color, vec3<f32>(2.2)), 1.0);
}
