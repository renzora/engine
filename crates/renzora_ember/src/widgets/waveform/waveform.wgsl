// Audio waveform: up to 32 amplitude samples (0..1) drawn as a symmetric
// envelope mirrored around the vertical center line.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct WaveUniforms {
    data: array<vec4<f32>, 8>, // 32 amplitudes packed 4-per-vec4
    color: vec4<f32>,
    params: vec4<f32>,         // x = sample count, y = playback progress 0..1
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

    // Playback split: bars left of the playhead are the bright accent, bars to
    // the right are dimmed. As `progress` advances each frame the boundary sweeps
    // across, so a playing clip visibly animates.
    let progress = u.params.y;
    let played = 1.0 - smoothstep(progress - 0.004, progress + 0.004, in.uv.x);
    let dim = vec3<f32>(0.32, 0.34, 0.40);
    let bar_col = mix(dim, u.color.rgb, mix(0.28, 1.0, played));

    // A thin bright playhead marker at the boundary.
    let head = 1.0 - smoothstep(0.0, max(fwidth(in.uv.x) * 1.5, 0.006), abs(in.uv.x - progress));

    let fa = fill * u.color.a;
    let ca = center * 0.3 * (1.0 - fa);
    let base_alpha = fa + ca;
    let alpha = max(base_alpha, head * 0.9);
    if (alpha <= 0.0) {
        discard;
    }
    let env_col = (bar_col * fa + vec3<f32>(0.45, 0.45, 0.53) * ca) / max(base_alpha, 1e-4);
    let col = mix(env_col, u.color.rgb, head);
    return vec4<f32>(col, alpha);
}
