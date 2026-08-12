// Splash lens/film pass — the last thing that happens to the Light Chamber before
// the launcher UI is drawn over it.
//
// This samples the offscreen background render (written by the splash post camera)
// at the window's physical resolution, so everything here is display-referred: the
// artefacts a *lens and a sensor* would add, not anything about the scene. The
// scene-referred spectral work — dispersion, thin-film, anamorphic streaks — already
// happened in `chamber.wgsl`; keep the two separate or they fight over the same
// pixels and the frame goes muddy.
//
// Deliberately no scanlines. The chamber is a clean, modern, volumetric shot and a
// CRT overlay reads as a different decade.
//
// params.x = time, .y = width(px), .z = height(px).

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct PostUniforms {
    params: vec4<f32>,
};

@group(1) @binding(0) var<uniform> u: PostUniforms;
@group(1) @binding(1) var tex: texture_2d<f32>;
@group(1) @binding(2) var tex_sampler: sampler;

/// Integer hash (PCG-style) → white noise.
///
/// This replaced the usual `fract(sin(dot(p, k)) * 43758.5)` trick, which does not
/// survive being fed pixel coordinates. That hash relies on `sin` of a large value
/// being chaotic, but at f32 precision and inputs in the thousands it stops being
/// chaotic and becomes *periodic* — the grain collapsed into a diagonal moiré
/// weave across the entire frame, which read as wavy lines rather than as noise.
/// An integer hash has no such failure mode: it's exact at every pixel.
fn pcg_hash(x: u32) -> u32 {
    let state = x * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

/// Per-pixel, per-frame white noise in 0..1.
fn grain_noise(px: vec2<u32>, frame: u32) -> f32 {
    let h = pcg_hash(px.x * 374761393u + px.y * 668265263u + frame * 1442695041u);
    return f32(h & 0x00FFFFFFu) / 16777216.0;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let t = u.params.x;
    let res = vec2<f32>(max(u.params.y, 1.0), max(u.params.z, 1.0));
    let texel = 1.0 / res;
    let dir = uv - vec2<f32>(0.5);
    let r2 = dot(dir, dir);

    // ── Lateral chromatic aberration ──
    // Zero at the optical centre and growing quartically toward the corners, which
    // is how a real lens fails. The centre column carries the launcher's text, so
    // it has to stay perfectly registered there.
    let amt = r2 * r2 * 0.010 + r2 * 0.0016;
    var col = vec3<f32>(
        textureSample(tex, tex_sampler, uv + dir * amt).r,
        textureSample(tex, tex_sampler, uv).g,
        textureSample(tex, tex_sampler, uv - dir * amt).b,
    );

    // ── Halation ──
    // Wide, soft, and only off the brightest cores — the glow that bleeds around a
    // hot highlight on film. This is a *lens* bloom on top of the renderer's
    // physical bloom, so the threshold is high and the radius much wider; overlap
    // the two and every shaft turns into a single smear.
    var halo = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var i = -2; i <= 2; i = i + 1) {
        for (var j = -2; j <= 2; j = j + 1) {
            let fo = vec2<f32>(f32(i), f32(j));
            let w = exp(-dot(fo, fo) * 0.35);
            let s = textureSample(tex, tex_sampler, uv + fo * texel * 7.0).rgb;
            halo = halo + max(s - vec3<f32>(0.55), vec3<f32>(0.0)) * w;
            wsum = wsum + w;
        }
    }
    col = col + (halo / wsum) * 1.35;

    // ── Grade ──
    // Contrast about a low pivot (this is a dark frame — a 0.5 pivot would crush
    // everything that matters), then a cool tint pushed into the shadows only, so
    // the blacks read as air in a dark room rather than as dead pixels.
    //
    // Keep that tint *tiny*. The output is written to an sRGB target, so a lift of
    // 0.026 in linear lands near 0.18 after encoding — it washed the entire frame to
    // a flat navy, buried the shafts, and left the vignette with nothing to darken
    // because no pixel was near black any more. A third of that is all it takes to
    // read as air instead of as dead pixels.
    col = max((col - 0.16) * 1.14 + 0.16, vec3<f32>(0.0));
    let shadow = 1.0 - smoothstep(0.0, 0.35, luma(col));
    col = col + shadow * vec3<f32>(0.002, 0.004, 0.009);

    // ── Vignette ──
    // Heavy and static. Three things it has to get right, all of which the first
    // attempts got wrong:
    //
    // * **It has to reach the light.** A vignette only multiplies, so one confined
    //   to the corners of a near-black frame does nothing visible — it's modulating
    //   zero. This one starts falling off almost immediately outside the centre, so
    //   it crosses the shafts, which are the only pixels bright enough for a
    //   multiply to show at all.
    // * **`smoothstep` was the wrong curve.** It's flat at both ends, so almost all
    //   of its travel happened out past the corners where there was nothing to
    //   darken. `pow` falls off continuously from the centre outward, which is both
    //   what a real lens does and what you can actually see.
    // * **The floor has to be genuinely low.** 0.05 at the corner sounds brutal
    //   written down; on a dark frame it is the difference between a lens and a flat
    //   backdrop.
    //
    // Darkening the frame *behind* the launcher is fine and slightly helps — the UI
    // is drawn on top of this pass, so text only gains contrast from it.
    //
    // `vd` is normalised so 0 = centre and 1 = corner, independent of window shape.
    let vd = length(dir) * 1.4142136;
    let vig = pow(clamp(1.0 - vd * 0.88, 0.0, 1.0), 1.7);
    col = col * mix(0.05, 1.0, vig);

    // A trace of cool colour left in the darkened edge — the vignette isn't just an
    // absence of light, it's the far air of the chamber closing in. Same warning as
    // the shadow tint above: anything larger than this and the corners stop being
    // dark, which is the entire point of the vignette.
    col = col + (1.0 - vig) * vec3<f32>(0.001, 0.003, 0.007);

    // ── Film grain ──
    // Hashed from the integer pixel coordinate and a frame counter, so it's true
    // per-pixel noise that resamples every frame. Applied equally to all three
    // channels — film grain is a density variation, not colour noise, and per-channel
    // noise reads as a broken sensor instead.
    //
    // Scaled with darkness the way film grain actually behaves, and it doubles as
    // the dither that hides any residual banding from the volumetric raymarch
    // (which runs without jitter — see `native_chamber.rs`).
    //
    // The amplitude is deliberately low. The chamber is mostly near-black, and grain
    // scaled up for the shadows there covers most of the frame — at the old 0.055 it
    // read as a noisy image rather than as film. The shadow figure keeps a healthy
    // margin over the ~1/255 needed to break up banding, which is the floor this
    // can't drop below without the raymarch's rings showing through.
    let px = vec2<u32>(in.position.xy);
    let g = grain_noise(px, u32(t * 60.0)) - 0.5;
    col = col + g * mix(0.022, 0.007, smoothstep(0.0, 0.5, luma(col)));

    return vec4<f32>(col, 1.0);
}
