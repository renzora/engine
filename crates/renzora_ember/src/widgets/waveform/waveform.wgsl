// Audio waveform: up to 32 amplitude samples (0..1) drawn as a symmetric
// envelope mirrored around the vertical center line.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct WaveUniforms {
    data: array<vec4<f32>, 8>, // 32 amplitudes packed 4-per-vec4
    color: vec4<f32>,
    params: vec4<f32>,         // x = sample count
};

@group(1) @binding(0)
var<uniform> u: WaveUniforms;

fn sample(i: i32) -> f32 {
    let v = u.data[i >> 2u];
    let c = i & 3;
    if (c == 0) { return v.x; }
    if (c == 1) { return v.y; }
    if (c == 2) { return v.z; }
    return v.w;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let n = i32(u.params.x);
    let denom = f32(max(n - 1, 1));
    let fx = clamp(in.uv.x, 0.0, 1.0) * denom;
    let i0 = clamp(i32(floor(fx)), 0, max(n - 2, 0));
    let a = mix(sample(i0), sample(i0 + 1), fract(fx));

    let dist = abs(in.uv.y - 0.5) * 2.0; // 0 at center, 1 at edges
    let aa = max(fwidth(dist), 0.01);
    let fill = 1.0 - smoothstep(a - aa, a + aa, dist);

    let center = 1.0 - smoothstep(0.0, max(fwidth(in.uv.y) * 1.5, 0.004), abs(in.uv.y - 0.5));

    let fa = fill * u.color.a;
    let ca = center * 0.3 * (1.0 - fa);
    let alpha = fa + ca;
    if (alpha <= 0.0) {
        discard;
    }
    let col = (u.color.rgb * fa + vec3<f32>(0.45, 0.45, 0.53) * ca) / max(alpha, 1e-4);
    return vec4<f32>(col, alpha);
}
