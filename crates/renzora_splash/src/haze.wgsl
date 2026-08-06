// Loading-screen backdrop — the Light Chamber's air, without the chamber.
//
// The iris closes on a corridor of spectral shafts and drifting dust, and this is
// what's on the other side of it: the same shafts and the same dust, seen from
// somewhere deeper in the fog, with the geometry gone. Purely 2D — there is no
// scene to render at this point, and the loading screen must not compete for the
// GPU with the project that's decoding behind it.
//
// params.x = time, .y = width px, .z = height px.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct HazeUniforms {
    params: vec4<f32>,
};
@group(1) @binding(0) var<uniform> u: HazeUniforms;

const SHAFTS: i32 = 3;

fn hash21(p: vec2<f32>) -> f32 {
    var h = fract(p * vec2<f32>(0.1031, 0.1030));
    h = h + dot(h, h.yx + 33.33);
    return fract((h.x + h.y) * h.x);
}

// Cosine palette — the same one the chamber and the iris use, so all three
// screens draw their colour from one spectrum.
fn spectrum(t: f32) -> vec3<f32> {
    return 0.5 + 0.5 * cos(6.28318530718 * (vec3<f32>(0.0, 0.33, 0.67) + t));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let t = u.params.x;
    let res = max(vec2<f32>(u.params.y, u.params.z), vec2<f32>(1.0));
    let aspect = res.x / res.y;
    // Aspect-corrected so the shafts hold their angle instead of shearing with the
    // window, and the dust stays round.
    let p = (in.uv - vec2<f32>(0.5)) * vec2<f32>(aspect, 1.0);

    var col = vec3<f32>(0.006, 0.007, 0.013);

    // ── Shafts ──
    // Each is a soft band through the origin, slowly rotating at its own rate so
    // they scissor past each other and their crossings keep moving.
    for (var i = 0; i < SHAFTS; i = i + 1) {
        let fi = f32(i);
        let ang = fi * 1.1 + t * (0.021 + fi * 0.011);
        let n = vec2<f32>(-sin(ang), cos(ang));      // band normal
        let d = abs(dot(p, n));
        let width = 0.10 + 0.045 * sin(t * 0.19 + fi * 2.1);
        var band = exp(-(d * d) / (width * width));
        // Fade along the shaft so it reads as light arriving from somewhere rather
        // than as a stripe pinned across the frame.
        let along = dot(p, vec2<f32>(cos(ang), sin(ang)));
        band = band * (0.35 + 0.65 * smoothstep(-0.9, 0.5, along));
        col = col + spectrum(fi * 0.33 + t * 0.028) * band * 0.16;
    }

    // ── Dust ──
    // Three layers at different scales drifting at different speeds — the parallax
    // between them is what gives a flat gradient any depth. Each speck is only
    // visible in proportion to how much light is on it, so the dust appears and
    // disappears as the shafts sweep over it.
    let lit = clamp(dot(col, vec3<f32>(1.4, 1.4, 1.4)), 0.0, 1.0);
    for (var l = 0; l < 3; l = l + 1) {
        let fl = f32(l);
        let scale = 14.0 + fl * 11.0;
        let drift = vec2<f32>(t * (0.012 + fl * 0.009), t * (-0.020 - fl * 0.011));
        let g = (p + drift) * scale;
        let cell = floor(g);
        let jitter = vec2<f32>(hash21(cell), hash21(cell + 17.0));
        // Only a fraction of cells hold a mote, else it reads as a regular grid.
        if (hash21(cell + 91.0) > 0.55) {
            let d = length(fract(g) - jitter);
            let speck = 1.0 - smoothstep(0.0, 0.055 + fl * 0.02, d);
            let twinkle = 0.55 + 0.45 * sin(t * (0.9 + fl * 0.5) + hash21(cell) * 12.0);
            col = col + vec3<f32>(0.75, 0.85, 1.0) * speck * twinkle * lit * 0.5;
        }
    }

    // ── Vignette ──
    // Matches the splash's (`post.wgsl`) so the framing doesn't change across the
    // iris, and steeper still, because the loading terminal sits in the middle of
    // this and needs the edges to drop away behind it.
    let vd = length(p) * 1.4142136;
    let vig = 1.0 - smoothstep(0.22, 1.02, vd);
    col = col * mix(0.10, 1.0, vig);

    return vec4<f32>(col, 1.0);
}
