// Curve editor: a cubic bezier (control points in 0..1, y up) drawn as an
// antialiased SDF stroke over a faint grid.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct CurveUniforms {
    ab: vec4<f32>,
    cd: vec4<f32>,
    color: vec4<f32>,
    params: vec4<f32>, // x = stroke width
};

@group(1) @binding(0)
var<uniform> u: CurveUniforms;

fn cubic(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>, t: f32) -> vec2<f32> {
    let m = 1.0 - t;
    return p0 * (m * m * m) + p1 * (3.0 * m * m * t) + p2 * (3.0 * m * t * t) + p3 * (t * t * t);
}

fn seg_dist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-5), 0.0, 1.0);
    return length(pa - ba * h);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let sz = in.size;
    let p = in.uv * sz;
    let p0 = vec2<f32>(u.ab.x, 1.0 - u.ab.y) * sz;
    let p1 = vec2<f32>(u.ab.z, 1.0 - u.ab.w) * sz;
    let p2 = vec2<f32>(u.cd.x, 1.0 - u.cd.y) * sz;
    let p3 = vec2<f32>(u.cd.z, 1.0 - u.cd.w) * sz;

    var prev = p0;
    var d = 1.0e9;
    let n = 32;
    for (var i = 1; i <= n; i = i + 1) {
        let cur = cubic(p0, p1, p2, p3, f32(i) / f32(n));
        d = min(d, seg_dist(p, prev, cur));
        prev = cur;
    }
    let aa = max(fwidth(d), 0.75);
    let stroke = 1.0 - smoothstep(u.params.x * 0.5 - aa, u.params.x * 0.5 + aa, d);

    let gx = abs(in.uv.x * 4.0 - round(in.uv.x * 4.0)) / 4.0 * sz.x;
    let gy = abs(in.uv.y * 4.0 - round(in.uv.y * 4.0)) / 4.0 * sz.y;
    let grid = (1.0 - smoothstep(0.0, 1.0, min(gx, gy))) * 0.14;

    let la = stroke * u.color.a;
    let ga = grid * (1.0 - la);
    let alpha = la + ga;
    if (alpha <= 0.0) {
        discard;
    }
    let col = (u.color.rgb * la + vec3<f32>(0.4, 0.4, 0.48) * ga) / max(alpha, 1e-4);
    return vec4<f32>(col, alpha);
}
