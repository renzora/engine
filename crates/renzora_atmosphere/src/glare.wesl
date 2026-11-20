// Sun glare: the haze a camera sees spreading from the sun when it is in view.
//
// Bloom cannot draw it. Bevy's bloom treats any pixel far brighter than its
// neighbours as a firefly and caps what it adds to the glow, so a sun disc a
// hundred thousand times brighter than the sky blooms exactly as much as a
// bright cloud. This pass adds the glare to the HDR frame directly, before
// tonemapping, so auto exposure and the tone curve treat it like any other
// light in the picture.
//
// It measures, rather than assumes, how much of the sun can be seen: a few
// texels inside the disc are compared with a ring of sky just outside it. A
// visible disc is orders of magnitude brighter than the sky around it; one
// behind a cloud or a wall is not. A ratio keeps that test independent of the
// camera's exposure and of how much the atmosphere has dimmed the sun, which
// near sunset is a lot. The ring's brightness also sets how strong the glare
// is, so it stays in proportion to the sky it sits in.
//
// The measurement is its own entry point, `measure`, drawn into a 1x1 texture
// per view that keeps a running average across frames (the blend does the
// averaging, see `glare.rs`). Measured fresh every frame it flickered under
// Solari: the disc is only a few pixels across, and the denoiser's noise and
// the camera jitter moved those pixels about from one frame to the next, so
// the glare jumped with them. `fragment` then reads the settled value.

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

struct Glare {
    clip_from_view: mat4x4<f32>,
    view_from_clip: mat4x4<f32>,
    // xyz = unit vector toward the sun in view space, w = the disc's angular
    // radius in radians.
    sun: vec4<f32>,
    // rgb = the sun's colour, a = strength (0 draws nothing).
    tint: vec4<f32>,
    // x = how far the glare spreads, in radians. yzw unused.
    params: vec4<f32>,
}

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var<uniform> glare: Glare;
// x = how much of the sun can be seen (0 to 1), y = the sky's brightness
// around it, both averaged over recent frames. Written by `measure`.
@group(0) @binding(3) var history: texture_2d<f32>;

const TAU: f32 = 6.28318530718;
const RING_SAMPLES: u32 = 8u;
const DISC_SAMPLES: u32 = 7u;
// Disc-to-sky brightness ratios over which a texel counts as going from
// "covered" to "the sun". A clear sun is thousands of times the sky; the
// brightest cloud edge beside it is a few times.
const COVERED_RATIO: f32 = 4.0;
const VISIBLE_RATIO: f32 = 40.0;

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn on_screen(p: vec2<f32>, size: vec2<f32>) -> bool {
    return all(p >= vec2<f32>(0.0)) && all(p < size);
}

// The brightest value this pass lets through: under the 65504 a 16-bit float
// holds, with room for the glare added on top.
const MAX_HDR: f32 = 32000.0;

// One channel brought into range. An exponent of all ones is an infinity
// (zero mantissa) or a NaN (anything else); checked on the bits because a
// compiler may fold `x != x` away.
fn finite(x: f32) -> f32 {
    let bits = bitcast<u32>(x);
    if (bits & 0x7f800000u) == 0x7f800000u {
        return select(0.0, MAX_HDR, (bits & 0x007fffffu) == 0u && x > 0.0);
    }
    return clamp(x, 0.0, MAX_HDR);
}

// The sun disc is far brighter than a 16-bit float holds at a daylight
// exposure: 40,000 lux over its half-degree comes out near 5e5 at EV 9.7, and
// the GPU stores that as infinity. Bloom's first pass scales each pixel by
// `1 / (1 + luma)`, which for an infinity is zero, and `inf * 0` is NaN; the
// NaN then spread through every bloom mip. Whether a disc texel overflowed
// moved with the camera jitter, so the bloom flashed every few frames. This
// pass runs before bloom and hands it a finite frame.
fn sanitize(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(finite(c.r), finite(c.g), finite(c.b));
}

fn load_luminance(p: vec2<f32>) -> f32 {
    // Sanitized too: one NaN folded into the running average would stay in it
    // for good.
    return luminance(sanitize(textureLoad(screen, vec2<i32>(p), 0).rgb));
}

/// How much of the sun is in view this frame, and how bright the sky around
/// it is: `vec4(visible, sky, 0, 1)`. Drawn into the 1x1 history texture,
/// whose blend averages it with what was there.
@fragment
fn measure(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // Where the sun lands on screen. A direction has w = 0, so for a
    // perspective camera clip w is the depth in front of it: at or below zero
    // the sun is behind the camera, and an orthographic camera never sees it.
    let clip = glare.clip_from_view * vec4<f32>(glare.sun.xyz, 0.0);
    if clip.w <= 1e-5 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let size = vec2<f32>(textureDimensions(screen));
    let ndc = clip.xy / clip.w;
    let sun_px = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * size;
    // Pixels per radian near the centre of the view, vertically.
    let px_per_rad = glare.clip_from_view[1][1] * 0.5 * size.y;
    let disc_px = max(glare.sun.w * px_per_rad, 1.0);
    let ring_px = max(disc_px * 4.0, disc_px + 6.0);

    // The sky just outside the disc.
    var sky = 0.0;
    var sky_samples = 0.0;
    for (var i = 0u; i < RING_SAMPLES; i += 1u) {
        let a = f32(i) * TAU / f32(RING_SAMPLES);
        let p = sun_px + vec2<f32>(cos(a), sin(a)) * ring_px;
        if on_screen(p, size) {
            sky += load_luminance(p);
            sky_samples += 1.0;
        }
    }
    if sky_samples < 1.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    sky = max(sky / sky_samples, 1e-4);

    // How much of the disc is the sun. A texel off screen counts as covered,
    // so the glare fades out as the sun leaves the frame instead of popping.
    var visible = 0.0;
    for (var i = 0u; i < DISC_SAMPLES; i += 1u) {
        var p = sun_px;
        if i > 0u {
            let a = f32(i - 1u) * TAU / f32(DISC_SAMPLES - 1u);
            p += vec2<f32>(cos(a), sin(a)) * disc_px * 0.6;
        }
        if on_screen(p, size) {
            visible += smoothstep(COVERED_RATIO, VISIBLE_RATIO, load_luminance(p) / sky);
        }
    }
    visible /= f32(DISC_SAMPLES);
    return vec4<f32>(visible, sky, 0.0, 1.0);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // A load, not a sample: filtering would blend an infinity into the texels
    // around it before `sanitize` could catch it.
    let raw = textureLoad(screen, vec2<i32>(in.position.xy), 0);
    let scene = vec4<f32>(sanitize(raw.rgb), raw.a);
    let settled = textureLoad(history, vec2<i32>(0, 0), 0);
    let visible = settled.x;
    let sky = settled.y;
    if glare.tint.a <= 0.0 || visible <= 1e-3 {
        return scene;
    }

    // The angle between this pixel's view ray and the sun, so the glare is
    // round however wide the lens or the viewport. NDC z = 1 is the near plane
    // under Bevy's reversed depth.
    let pixel_ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let v = glare.view_from_clip * vec4<f32>(pixel_ndc, 1.0, 1.0);
    let ray = normalize(v.xyz / v.w);
    let angle = acos(clamp(dot(ray, glare.sun.xyz), -1.0, 1.0));

    // Three falloffs: a hot core hugging the disc, the halo most of the glare
    // lives in, and a faint wide wash that lifts the whole sunward sky.
    let spread = max(glare.params.x, 1e-3);
    let core = exp(-pow(angle / (spread * 0.08), 2.0)) * 6.0;
    let halo = pow(1.0 + pow(angle / (spread * 0.25), 2.0), -1.5) * 1.5;
    let wash = exp(-angle / spread) * 0.3;
    let glow = (core + halo + wash) * glare.tint.a * visible * sky;

    return vec4<f32>(scene.rgb + glare.tint.rgb * glow, scene.a);
}
