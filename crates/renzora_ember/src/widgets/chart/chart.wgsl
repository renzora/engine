// Line/area chart: up to 32 normalized samples (0..1) drawn as an antialiased
// polyline with a soft fill below it. `uv * size` is the fragment pixel inside
// the chart node.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct ChartUniforms {
    // 32 samples packed 4-per-vec4
    data: array<vec4<f32>, 8>,
    color: vec4<f32>,
    // x = sample count, y = line width (px), z = fill alpha
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

    let alpha = max(stroke, fill);
    if (alpha <= 0.0) {
        discard;
    }
    return vec4<f32>(u.color.rgb, u.color.a * alpha);
}
