// Line/area chart: up to 32 normalized samples (0..1) drawn as an antialiased
// polyline with a soft fill below it, an optional target line, a faint grid, and
// a dot on the latest sample. `uv * size` is the fragment pixel inside the node.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct ChartUniforms {
    // 32 samples packed 4-per-vec4
    data: array<vec4<f32>, 8>,
    color: vec4<f32>,
    // x = sample count, y = line width (px), z = fill alpha,
    // w = normalized target-line y in 0..1 (< 0 disables it)
    params: vec4<f32>,
};

@group(1) @binding(0)
var<uniform> u: ChartUniforms;

fn sample(i: i32) -> f32 {
    let v = u.data[i >> 2u];
    let c = i & 3;
    if (c == 0) { return v.x; }
    if (c == 1) { return v.y; }
    if (c == 2) { return v.z; }
    return v.w;
}

fn seg_dist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-5), 0.0, 1.0);
    return length(pa - ba * h);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let n = i32(u.params.x);
    let lw = u.params.y;
    let fill_a = u.params.z;
    let target = u.params.w;
    let sz = in.size;
    let p = in.uv * sz;
    let denom = f32(max(n - 1, 1));

    let fx = clamp(in.uv.x, 0.0, 1.0) * denom;
    let i0 = clamp(i32(floor(fx)), 0, max(n - 2, 0));
    let ax = f32(i0) / denom * sz.x;
    let bx = f32(i0 + 1) / denom * sz.x;
    let ay = (1.0 - sample(i0)) * sz.y;
    let by = (1.0 - sample(i0 + 1)) * sz.y;
    let a = vec2<f32>(ax, ay);
    let b = vec2<f32>(bx, by);

    let d = seg_dist(p, a, b);
    let aa = max(fwidth(d), 0.75);
    let stroke = 1.0 - smoothstep(lw * 0.5 - aa, lw * 0.5 + aa, d);

    let line_y = mix(ay, by, clamp((p.x - ax) / max(bx - ax, 1e-4), 0.0, 1.0));
    let fill = smoothstep(line_y - 1.0, line_y + 1.0, p.y) * fill_a;

    // Dot on the latest sample (right edge).
    let last = vec2<f32>(sz.x - 2.0, (1.0 - sample(n - 1)) * sz.y);
    let dot = 1.0 - smoothstep(2.5, 4.0, length(p - last));
    let line_px = max(stroke, dot);

    // Faint horizontal grid lines at 0/¼/½/¾/1 of the height.
    let yy = in.uv.y * 4.0;
    let gd = abs(yy - round(yy)) / 4.0 * sz.y;
    let grid_a = (1.0 - smoothstep(0.0, 1.0, gd)) * 0.16;

    // Optional target line.
    var tgt_a = 0.0;
    if (target >= 0.0) {
        let ty = (1.0 - target) * sz.y;
        let td = abs(p.y - ty);
        let taa = max(fwidth(td), 0.75);
        tgt_a = (1.0 - smoothstep(0.75 - taa, 0.75 + taa, td)) * 0.5;
    }

    // Layer: line/fill on top, then target line, then grid.
    let la = max(line_px, fill) * u.color.a;
    let rem = 1.0 - la;
    let ta = tgt_a * rem;
    let ga = grid_a * (rem - ta);
    let alpha = la + ta + ga;
    if (alpha <= 0.0) {
        discard;
    }
    let rgb = (u.color.rgb * la
        + vec3<f32>(0.39, 0.39, 0.39) * ta
        + vec3<f32>(0.42, 0.42, 0.5) * ga) / max(alpha, 1e-4);
    return vec4<f32>(rgb, alpha);
}
