// Analog-stick visualizer: outer ring + crosshairs + deadzone circle + a line
// from center to the stick position + a dot (green when outside the deadzone).
// Faithful to the egui painter version. params.xy = stick axes (-1..1).

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct StickUniforms {
    params: vec4<f32>, // x, y
};

@group(1) @binding(0)
var<uniform> u: StickUniforms;

fn seg_d(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-5), 0.0, 1.0);
    return length(pa - ba * h);
}

fn over(dst: vec4<f32>, src_rgb: vec3<f32>, src_a: f32) -> vec4<f32> {
    let a = src_a + dst.a * (1.0 - src_a);
    if (a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let rgb = (src_rgb * src_a + dst.rgb * dst.a * (1.0 - src_a)) / a;
    return vec4<f32>(rgb, a);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let sz = in.size;
    let p = in.uv * sz;
    let c = sz * 0.5;
    let r = sz.x * 0.5 - 4.0;
    let aa = 1.0;

    let g40 = vec3<f32>(40.0 / 255.0);
    let g50 = vec3<f32>(50.0 / 255.0);
    let g60 = vec3<f32>(60.0 / 255.0);
    let g150 = vec3<f32>(150.0 / 255.0);
    let blue = vec3<f32>(100.0 / 255.0, 150.0 / 255.0, 200.0 / 255.0);
    let green = vec3<f32>(100.0 / 255.0, 200.0 / 255.0, 100.0 / 255.0);
    let white = vec3<f32>(1.0);

    var out = vec4<f32>(0.0);

    // Crosshairs.
    let dh = seg_d(p, vec2<f32>(c.x - r, c.y), vec2<f32>(c.x + r, c.y));
    out = over(out, g40, 1.0 - smoothstep(0.5, 0.5 + aa, dh));
    let dv = seg_d(p, vec2<f32>(c.x, c.y - r), vec2<f32>(c.x, c.y + r));
    out = over(out, g40, 1.0 - smoothstep(0.5, 0.5 + aa, dv));

    // Outer ring.
    let doc = abs(length(p - c) - r);
    out = over(out, g60, 1.0 - smoothstep(0.5, 0.5 + aa, doc));

    // Deadzone (10%).
    let ddz = abs(length(p - c) - r * 0.1);
    out = over(out, g50, 1.0 - smoothstep(0.5, 0.5 + aa, ddz));

    // Stick position (Y inverted so up is up).
    let pos = vec2<f32>(c.x + u.params.x * r, c.y - u.params.y * r);

    // Line center -> position.
    let dl = seg_d(p, c, pos);
    out = over(out, blue, 1.0 - smoothstep(1.0, 1.0 + aa, dl));

    // Dot.
    let moved = abs(u.params.x) > 0.1 || abs(u.params.y) > 0.1;
    let dot_col = select(g150, green, moved);
    let dd = length(p - pos);
    out = over(out, dot_col, 1.0 - smoothstep(6.0 - aa, 6.0, dd));
    out = over(out, white, 1.0 - smoothstep(0.5, 0.5 + aa, abs(dd - 6.0)));

    if (out.a <= 0.0) {
        discard;
    }
    return vec4<f32>(pow(out.rgb, vec3<f32>(2.2)), out.a);
}
