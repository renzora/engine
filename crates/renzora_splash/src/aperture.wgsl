// Spectral iris — the splash → loading transition, and (played in reverse) the
// editor's power-on reveal.
//
// A circular aperture closes over the frame: everything outside it is opaque black,
// the closing edge carries a thin spectrum whose hue runs around the ring, and the
// last of the light collapses to a point and flashes out. It is the same iris in
// both directions, which is the reason it's one shader — `progress` 0 → 1 closes,
// 1 → 0 opens.
//
// This is an *overlay*: it cannot read the frame underneath, so "the image survives"
// is expressed as alpha 0 inside the aperture, not by sampling anything.
//
// params.x = progress 0..1, .y = active (0 = idle → fully transparent),
// .z = aspect (w/h, used to keep the iris circular rather than stretched).

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct ApertureUniforms {
    params: vec4<f32>,
};

@group(1) @binding(0) var<uniform> u: ApertureUniforms;

// Cosine palette — one full trip around the spectrum for t in 0..1.
fn spectrum(t: f32) -> vec3<f32> {
    return 0.5 + 0.5 * cos(6.28318530718 * (vec3<f32>(0.0, 0.33, 0.67) + t));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    if (u.params.y < 0.5) {
        return vec4<f32>(0.0); // idle: invisible, lets the splash through
    }

    let p = clamp(u.params.x, 0.0, 1.0);
    let aspect = max(u.params.z, 0.01);

    // Distance from centre in units where 0.5 = half the frame height. The corners
    // of a 16:9 frame sit at ~1.02, so a radius of 1.3 is genuinely wide open.
    let d = length((in.uv - vec2<f32>(0.5)) * vec2<f32>(aspect, 1.0));
    let a = atan2(in.uv.y - 0.5, (in.uv.x - 0.5) * aspect);

    // Ease so the iris starts unhurried and slams shut at the end — a linear close
    // reads mechanical, and the whole move only lasts half a second.
    let eased = p * p * (3.0 - 2.0 * p);
    let radius = mix(1.3, 0.0, pow(eased, 0.75));

    // The ring thickens as it closes, so the light appears to pile up at the edge
    // instead of just shrinking away.
    let ring_w = 0.006 + 0.05 * eased;

    var col = vec3<f32>(0.0);
    // Opaque outside the aperture, fully transparent inside it.
    var alpha = smoothstep(radius - ring_w * 0.5, radius + ring_w * 0.5, d);

    // ── The spectral edge ──
    // Hue runs around the ring, offset by how far the iris has closed, so the
    // colours rotate as it shuts. The glow reaches *inward* only — outside is
    // already black and lifting it would grey the whole surround.
    let edge = 1.0 - smoothstep(0.0, ring_w * 3.0, abs(d - radius));
    let inner = 1.0 - smoothstep(0.0, ring_w * 9.0, max(radius - d, 0.0));
    let hue = a * 0.159154943 + eased * 0.6;
    let tint = spectrum(hue);
    let glow = edge * 1.0 + inner * 0.35;
    col = col + tint * glow * (0.7 + 1.6 * eased);
    alpha = max(alpha, clamp(glow, 0.0, 1.0));

    // ── The collapse ──
    // Once the aperture is nearly shut, a white core swells at the centre and then
    // fades, so the transition ends on light rather than on a hole closing.
    let flash = smoothstep(0.80, 0.97, p) * (1.0 - smoothstep(0.94, 1.0, p));
    let core = 1.0 - smoothstep(0.0, 0.09 + 0.04 * flash, d);
    col = col + mix(vec3<f32>(1.0), tint, 0.35) * core * flash * 2.2;
    alpha = max(alpha, core * flash);

    // Fully black by the end so the loading screen can take over cleanly.
    alpha = max(alpha, smoothstep(0.93, 1.0, p));

    return vec4<f32>(col, clamp(alpha, 0.0, 1.0));
}
