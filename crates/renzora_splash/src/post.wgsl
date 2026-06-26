// Splash post-process: samples the offscreen background render (sky + terrain
// composited, written by the splash post camera) and applies a *real* full-frame
// pass — thresholded bloom on the bright neon, radial chromatic aberration,
// subtle scanlines and a vignette. This is the genuine post the UI-overlay
// approach couldn't do (an overlay can't read what's behind it; here we sample the
// rendered texture directly). params.x = time, .y = width(px), .z = height(px).

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct PostUniforms {
    params: vec4<f32>,
};

@group(1) @binding(0) var<uniform> u: PostUniforms;
@group(1) @binding(1) var tex: texture_2d<f32>;
@group(1) @binding(2) var tex_sampler: sampler;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let t = u.params.x;
    let res = vec2<f32>(max(u.params.y, 1.0), max(u.params.z, 1.0));
    let texel = 1.0 / res;
    let dir = uv - vec2<f32>(0.5);

    // ── Chromatic aberration: split R/B radially, stronger toward the edges ──
    let amt = 0.0015 + dot(dir, dir) * 0.006;
    var col = vec3<f32>(
        textureSample(tex, tex_sampler, uv + dir * amt).r,
        textureSample(tex, tex_sampler, uv).g,
        textureSample(tex, tex_sampler, uv - dir * amt).b,
    );

    // ── Bloom: blur the bright (thresholded) areas and add them back ──
    var bloom = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var i = -2; i <= 2; i = i + 1) {
        for (var j = -2; j <= 2; j = j + 1) {
            let fo = vec2<f32>(f32(i), f32(j));
            let w = exp(-dot(fo, fo) * 0.4);
            let s = textureSample(tex, tex_sampler, uv + fo * texel * 3.5).rgb;
            bloom = bloom + max(s - vec3<f32>(0.5), vec3<f32>(0.0)) * w;
            wsum = wsum + w;
        }
    }
    col = col + (bloom / wsum) * 2.4;

    // ── Wavy scanlines: a dark line every ~3 device pixels, rippling horizontally
    // over time. Stable because this is a 1:1 pass over the rendered texture. ──
    let wave = sin(uv.x * 13.0 + t * 1.6) * 2.5 + sin(uv.x * 31.0 - t * 0.9) * 1.0;
    let s = 0.5 + 0.5 * cos((uv.y * res.y + wave) * 2.0943951); // 2π/3 → 3px period
    col = col * mix(0.74, 1.0, s);

    // ── Vignette ──
    let vig = smoothstep(1.1, 0.35, length(dir));
    col = col * mix(0.72, 1.0, vig);

    return vec4<f32>(col, 1.0);
}
