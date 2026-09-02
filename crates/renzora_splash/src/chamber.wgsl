// Spectral finishing pass for the Light Chamber render (see `chamber.rs`).
//
// The 3D pass mixes the iridescence *in the air* — three differently-hued keys
// whose shafts overlap into secondaries. This pass does the part that only exists
// in a lens: dispersion along beam edges, a thin-film sheen over the lit areas, and
// an anamorphic streak off the brightest shafts. It runs scene-referred; the film
// grade (aberration, halation, grain, vignette) is the *next* pass, `post.wgsl`.
//
// Everything here keys off `beam` — a mask of what is actually lit — so none of it
// can lift the black of the chamber, which has to stay black for the corridor to
// read as deep.
//
// params.x = time (s), .y = width(px), .z = height(px).

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct ChamberUniforms {
    params: vec4<f32>,
};

@group(1) @binding(0) var<uniform> u: ChamberUniforms;
@group(1) @binding(1) var tex: texture_2d<f32>;
@group(1) @binding(2) var tex_sampler: sampler;

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Cosine palette: one smooth trip around the spectrum for t in 0..1. Used for the
// thin-film tint, where the phase stands in for film thickness.
fn spectrum(t: f32) -> vec3<f32> {
    return 0.5 + 0.5 * cos(6.28318530718 * (vec3<f32>(0.0, 0.33, 0.67) + t));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let t = u.params.x;
    let res = vec2<f32>(max(u.params.y, 1.0), max(u.params.z, 1.0));
    let texel = 1.0 / res;

    let base = textureSample(tex, tex_sampler, uv).rgb;

    // ── Local luminance gradient ──
    // The direction light is changing fastest, which for this shot is always across
    // a shaft's edge. Both the dispersion and the film phase ride on it, so the
    // colour lands on the edges of beams rather than smeared over the whole frame.
    let e = texel * 2.0;
    let lx = luma(textureSample(tex, tex_sampler, uv + vec2<f32>(e.x, 0.0)).rgb)
           - luma(textureSample(tex, tex_sampler, uv - vec2<f32>(e.x, 0.0)).rgb);
    let ly = luma(textureSample(tex, tex_sampler, uv + vec2<f32>(0.0, e.y)).rgb)
           - luma(textureSample(tex, tex_sampler, uv - vec2<f32>(0.0, e.y)).rgb);
    let grad = vec2<f32>(lx, ly);
    let grad_mag = length(grad);
    var grad_dir = vec2<f32>(0.0, 0.0);
    if (grad_mag > 1e-5) {
        grad_dir = grad / grad_mag;
    }

    // ── Dispersion ──
    // Split R and B across the edge, scaled by how hard the edge is. A soft fog
    // gradient barely splits; a slat's shadow boundary splits visibly.
    let split = (0.6 + 2.4 * clamp(grad_mag * 3.0, 0.0, 1.0)) * texel.x * 1.6;
    var col = vec3<f32>(
        textureSample(tex, tex_sampler, uv + grad_dir * split).r,
        base.g,
        textureSample(tex, tex_sampler, uv - grad_dir * split).b,
    );

    // How lit this pixel is. Everything below is multiplied by it.
    let lum = luma(col);
    let beam = smoothstep(0.02, 0.30, lum);

    // ── Thin-film sheen ──
    // Phase = brightness (stands in for film thickness) + edge angle + a slow
    // crawl, so the sheen slides along a shaft instead of sitting on it as a
    // fixed stripe.
    let angle = atan2(grad_dir.y, grad_dir.x) * 0.159154943; // → 0..1 over a turn
    let phase = lum * 1.35 + angle * 0.5 + grad_mag * 2.0 + t * 0.035;
    let film = spectrum(phase);
    // Mixed toward, not added on: adding would just brighten every lit pixel and
    // wash the frame out. This recolours what's already there.
    col = mix(col, col * (0.55 + 1.15 * film), beam * 0.55);

    // ── Anamorphic streak ──
    // Only the genuinely bright cores (a shaft's spine, a mote caught in a beam)
    // clear the threshold, so the streaks read as lens artefacts on the few hot
    // spots rather than a horizontal blur over everything.
    var streak = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var i = 1; i <= 8; i = i + 1) {
        let d = f32(i) * texel.x * 6.0;
        let w = 1.0 / f32(i);
        let a = textureSample(tex, tex_sampler, uv + vec2<f32>(d, 0.0)).rgb;
        let b = textureSample(tex, tex_sampler, uv - vec2<f32>(d, 0.0)).rgb;
        streak = streak + (max(a - vec3<f32>(0.62), vec3<f32>(0.0))
                        +  max(b - vec3<f32>(0.62), vec3<f32>(0.0))) * w;
        wsum = wsum + w * 2.0;
    }
    // Cool-tinted, as a real anamorphic flare is.
    col = col + (streak / wsum) * vec3<f32>(0.55, 0.85, 1.25) * 1.6;

    // ── Haze lift ──
    // A trace of the beam colour bled into the surrounding black, so shafts sit in
    // the air rather than being cut out of it. Kept tiny — this is the one thing
    // here that touches the blacks.
    col = col + col * beam * 0.06;

    return vec4<f32>(col, 1.0);
}
