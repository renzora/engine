// RENZORA PATCH: a vendor-neutral spatiotemporal denoiser for Solari's realtime
// lighting, so the image is clean on any GPU with hardware ray tracing and not
// only where DLSS Ray Reconstruction runs.
//
// The signal comes from the lighting passes (`denoise_input.wesl`): diffuse and
// specular lighting written separately, each divided by its material factor and
// exposed. Filtering lighting rather than colour is what keeps texture detail
// sharp, and filtering the lobes apart is what keeps reflections from being
// smeared to the blur a diffuse surface can take.
//
// Three stages, each its own pipeline:
//
//   temporal  reproject last frame's result through the motion vectors, reject it
//             where the surface changed, and blend in the new sample. Tracks a
//             per-pixel history length and luminance moments for a noise
//             estimate. Based on the temporal half of SVGF (Schied et al. 2017).
//   atrous    five passes of an edge-avoiding a-trous wavelet blur (Dammertz et
//             al. 2010) at spacings 1, 2, 4, 8 and 16 pixels, stopped by surface
//             planes, normals and luminance, the last scaled by the noise
//             estimate so converged pixels are left alone. Unlike SVGF, no blur
//             is fed back into the history: measured against a converged path
//             trace, feeding it back compounded the blur's leaks across edges
//             frame after frame, and accumulating the unblurred signal was more
//             accurate both standing still and moving.
//   composite multiply the material factors back in and add emissive.
//
// Written from the published papers; no code is taken from any denoiser SDK.

import bevy_core_pipeline::tonemapping::tonemapping_luminance as luminance;
import bevy_render::view::View;
import bevy_render::utils::octahedral_decode;
import bevy_pbr::deferred::types::unpack_24bit_normal;
import bevy_pbr::prepass::bindings::PreviousViewUniforms;
import package::realtime::gbuffer_utils::gpixel_resolve;
import package::realtime::denoise_utils::{demodulation_factors, signal_moments};
import package::scene::bindings::ResolvedMaterial;

@group(0) @binding(0) var<uniform> view: View;

@if(DENOISE_TEMPORAL || DENOISE_COMPOSITE) {
@group(0) @binding(1) var gbuffer: texture_2d<u32>;
@group(0) @binding(2) var depth_buffer: texture_depth_2d;
}

@if(DENOISE_TEMPORAL) {
@group(0) @binding(3) var previous_gbuffer: texture_2d<u32>;
@group(0) @binding(4) var previous_depth_buffer: texture_depth_2d;
@group(0) @binding(5) var motion_vectors: texture_2d<f32>;
@group(0) @binding(6) var<uniform> previous_view: PreviousViewUniforms;
@group(0) @binding(7) var noisy_diffuse: texture_2d<f32>;
@group(0) @binding(8) var noisy_specular: texture_2d<f32>;
@group(0) @binding(9) var history_diffuse_in: texture_2d<f32>;
@group(0) @binding(10) var history_specular_in: texture_2d<f32>;
@group(0) @binding(11) var moments_in: texture_2d<f32>;
@group(0) @binding(12) var history_diffuse_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(13) var history_specular_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(14) var moments_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(15) var filtered_diffuse: texture_storage_2d<rgba16float, write>;
@group(0) @binding(16) var filtered_specular: texture_storage_2d<rgba16float, write>;
@group(0) @binding(17) var<uniform> settings: DenoiserSettings;
@group(0) @binding(18) var guide_out: texture_storage_2d<rgba32uint, write>;
}

@if(DENOISE_ATROUS) {
@group(0) @binding(1) var guide: texture_2d<u32>;
@group(0) @binding(2) var source_diffuse: texture_2d<f32>;
@group(0) @binding(3) var source_specular: texture_2d<f32>;
@group(0) @binding(4) var destination_diffuse: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var destination_specular: texture_storage_2d<rgba16float, write>;
}

@if(DENOISE_COMPOSITE) {
@group(0) @binding(3) var final_diffuse: texture_2d<f32>;
@group(0) @binding(4) var final_specular: texture_2d<f32>;
@group(0) @binding(5) var view_output: texture_storage_2d<rgba16float, write>;
}

/// Per-frame state. Must match `DenoiserUniforms` in `prepare.rs`.
struct DenoiserSettings {
    /// Nonzero on a frame whose history must not be used: the first frame, a
    /// resize, a camera cut or a lighting change big enough to reset Solari.
    reset: u32,
}

// ── Tuning ──────────────────────────────────────────────────────────────────

/// Most frames a diffuse pixel averages over. More is smoother and slower to
/// follow a lighting change.
const DIFFUSE_MAX_HISTORY: f32 = 32.0;

/// Most frames a specular pixel averages over, for a mirror and for a rough
/// surface. A reflection moves across its surface as the camera does, and
/// reprojecting it by the surface's motion puts it in the wrong place, so a
/// sharp one must forget fast or it smears.
const SPECULAR_MIRROR_MAX_HISTORY: f32 = 4.0;
const SPECULAR_ROUGH_MAX_HISTORY: f32 = 32.0;

/// Perceptual roughness at and above which a reflection counts as rough.
const SPECULAR_ROUGH_ROUGHNESS: f32 = 0.4;

/// The lowest perceptual roughness Solari shades with (`MIN_PERCEPTUAL_ROUGHNESS`).
const SPECULAR_MIRROR_ROUGHNESS: f32 = 0.089;

/// How much of a reflection's lobe width the view direction may turn in a
/// frame before its history starts to shrink. Its reflection moves twice the
/// turn, and a quarter of the lobe is below what the eye picks out.
const PARALLAX_LOBE_FRACTION: f32 = 0.25;

/// A history tap is the same surface when it lies within this fraction of its
/// distance from the camera of the current pixel's plane.
const REPROJECTION_PLANE_TOLERANCE: f32 = 0.01;

/// A history tap is the same surface when its normal is within about 25 degrees.
const REPROJECTION_NORMAL_COS: f32 = 0.9;

/// Below this history length the moments are too few to estimate noise from, and
/// a 3x3 neighbourhood estimate is used instead.
const SHORT_HISTORY: f32 = 4.0;

/// How much brighter than the brightest of its eight neighbours an accumulated
/// pixel may be shown. See `suppress_speck`.
const SPECK_LIMIT: f32 = 1.5;

/// How much brighter than its history a new sample may be once the history is
/// established: only a guard against a numerical blow-up. A clamp near the
/// history's own brightness (8x, as this once was) cuts real light, the rare
/// bright samples indirect lighting is made of, and left indirectly lit
/// surfaces about a tenth too dark against a converged path trace; even 64x
/// cost measurable accuracy. The temporal average and the blur already spread
/// a lone bright sample thin.
const FIREFLY_LIMIT: f32 = 1024.0;

/// How strongly the blur stops at a luminance difference, in multiples of the
/// estimated noise. Higher blurs more.
const LUMINANCE_EDGE_SIGMA: f32 = 4.0;

/// How sharply the blur stops at a normal difference.
const DIFFUSE_NORMAL_POWER: f32 = 64.0;
const SPECULAR_NORMAL_POWER: f32 = 128.0;

/// How sharply the blur stops when a tap is off the centre pixel's plane, per
/// unit of plane distance relative to camera distance.
const PLANE_EDGE_SHARPNESS: f32 = 100.0;

/// How sharply the specular blur stops at a roughness difference.
const ROUGHNESS_EDGE_SHARPNESS: f32 = 20.0;

/// The noise estimate never falls below this fraction of the pixel's own
/// luminance. ReSTIR shares samples between neighbours, which makes its noise
/// look smooth over a few pixels; trusted at face value, that reads as "no
/// noise" and the blur stops while blotches remain.
const RELATIVE_NOISE_FLOOR: f32 = 0.05;

// ── Shared ──────────────────────────────────────────────────────────────────

fn view_size() -> vec2<u32> {
    return vec2<u32>(view.main_pass_viewport.zw);
}

fn in_view(pixel: vec2<i32>) -> bool {
    return all(pixel >= vec2(0)) && all(pixel < vec2<i32>(view_size()));
}

fn specular_max_history(perceptual_roughness: f32) -> f32 {
    let t = saturate((perceptual_roughness - SPECULAR_MIRROR_ROUGHNESS) / (SPECULAR_ROUGH_ROUGHNESS - SPECULAR_MIRROR_ROUGHNESS));
    return mix(SPECULAR_MIRROR_MAX_HISTORY, SPECULAR_ROUGH_MAX_HISTORY, t);
}

/// How much of the spatial blur a reflection takes: none for a mirror, whose
/// detail is real, all of it from a rough surface up.
fn specular_blur_strength(perceptual_roughness: f32) -> f32 {
    return saturate((perceptual_roughness - SPECULAR_MIRROR_ROUGHNESS) / (SPECULAR_ROUGH_ROUGHNESS - SPECULAR_MIRROR_ROUGHNESS));
}

// ── Guide ───────────────────────────────────────────────────────────────────
//
// One rgba32uint texel per pixel holding exactly what the a-trous edge-stops
// read: the world position as f32 bits in xyz, and in w the G-buffer's own
// 24-bit octahedral normal (copied, not re-encoded), 7 bits of perceptual
// roughness and a top bit marking a pixel Solari shaded. The sky is all zero,
// so the top bit doubles as the depth test.

/// Set in a guide's w for a shaded pixel.
const GUIDE_COVERED: u32 = 0x80000000u;

struct GuidePixel {
    world_position: vec3<f32>,
    world_normal: vec3<f32>,
    perceptual_roughness: f32,
}

@if(DENOISE_TEMPORAL)
fn pack_guide(world_position: vec3<f32>, packed_normal: u32, perceptual_roughness: f32) -> vec4<u32> {
    let roughness = u32(round(saturate(perceptual_roughness) * 127.0));
    return vec4(bitcast<vec3<u32>>(world_position), GUIDE_COVERED | (roughness << 24u) | (packed_normal & 0xFFFFFFu));
}

@if(DENOISE_ATROUS)
fn guide_covered(texel: vec4<u32>) -> bool {
    return (texel.w & GUIDE_COVERED) != 0u;
}

@if(DENOISE_ATROUS)
fn unpack_guide(texel: vec4<u32>) -> GuidePixel {
    return GuidePixel(
        bitcast<vec3<f32>>(texel.xyz),
        octahedral_decode(unpack_24bit_normal(texel.w)),
        f32((texel.w >> 24u) & 0x7Fu) / 127.0,
    );
}

// ── Temporal ────────────────────────────────────────────────────────────────

@if(DENOISE_TEMPORAL)
@compute @workgroup_size(8, 8, 1)
fn temporal(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel_id = global_id.xy;
    if any(pixel_id >= view_size()) { return; }

    let depth = textureLoad(depth_buffer, pixel_id, 0);
    if depth == 0.0 {
        textureStore(history_diffuse_out, pixel_id, vec4(0.0));
        textureStore(history_specular_out, pixel_id, vec4(0.0));
        textureStore(moments_out, pixel_id, vec4(0.0));
        textureStore(filtered_diffuse, pixel_id, vec4(0.0));
        textureStore(filtered_specular, pixel_id, vec4(0.0));
        textureStore(guide_out, pixel_id, vec4(0u));
        return;
    }
    let gpixel = textureLoad(gbuffer, pixel_id, 0);
    let surface = gpixel_resolve(gpixel, depth, pixel_id, view.main_pass_viewport.zw, view.world_from_clip);
    let view_distance = max(length(view.world_position - surface.world_position), 0.0001);
    textureStore(guide_out, pixel_id, pack_guide(surface.world_position, gpixel.a, surface.material.perceptual_roughness));

    var diffuse = textureLoad(noisy_diffuse, pixel_id, 0).rgb;
    var specular = textureLoad(noisy_specular, pixel_id, 0).rgb;

    // Bilinear reprojection, each of the four taps tested against the current
    // surface, so an edge does not pull in history from what was behind it.
    var history_diffuse = vec4(0.0);
    var history_specular = vec4(0.0);
    var history_moments = vec4(0.0);
    var history_weight = 0.0;
    // The range of the valid central taps, which the sharper resample below is
    // held within.
    var diffuse_min = vec3(1e30);
    var diffuse_max = vec3(0.0);
    var specular_min = vec3(1e30);
    var specular_max = vec3(0.0);
    var base = vec2(0);
    var f = vec2(0.0);
    if settings.reset == 0u {
        let motion_vector = textureLoad(motion_vectors, pixel_id, 0).xy;
        let previous_position = vec2<f32>(pixel_id) + 0.5 - motion_vector * view.main_pass_viewport.zw - 0.5;
        base = vec2<i32>(floor(previous_position));
        f = fract(previous_position);
        let bilinear = array<f32, 4>((1.0 - f.x) * (1.0 - f.y), f.x * (1.0 - f.y), (1.0 - f.x) * f.y, f.x * f.y);
        let offsets = array<vec2<i32>, 4>(vec2(0, 0), vec2(1, 0), vec2(0, 1), vec2(1, 1));
        for (var i = 0u; i < 4u; i++) {
            let tap = base + offsets[i];
            if !in_view(tap) || bilinear[i] <= 0.0 { continue; }
            let tap_id = vec2<u32>(tap);
            let tap_depth = textureLoad(previous_depth_buffer, tap_id, 0);
            if tap_depth == 0.0 { continue; }
            let previous_surface = gpixel_resolve(textureLoad(previous_gbuffer, tap_id, 0), tap_depth, tap_id, view.main_pass_viewport.zw, previous_view.world_from_clip);
            let plane_distance = abs(dot(surface.world_normal, previous_surface.world_position - surface.world_position));
            if plane_distance > REPROJECTION_PLANE_TOLERANCE * view_distance { continue; }
            if dot(surface.world_normal, previous_surface.world_normal) < REPROJECTION_NORMAL_COS { continue; }

            let tap_diffuse = textureLoad(history_diffuse_in, tap_id, 0);
            let tap_specular = textureLoad(history_specular_in, tap_id, 0);
            history_diffuse += tap_diffuse * bilinear[i];
            history_specular += tap_specular * bilinear[i];
            history_moments += textureLoad(moments_in, tap_id, 0) * bilinear[i];
            history_weight += bilinear[i];
            diffuse_min = min(diffuse_min, tap_diffuse.rgb);
            diffuse_max = max(diffuse_max, tap_diffuse.rgb);
            specular_min = min(specular_min, tap_specular.rgb);
            specular_max = max(specular_max, tap_specular.rgb);
        }
    }

    // At a silhouette the camera's sub-pixel jitter keeps changing which
    // surface a pixel sees, so the four texels around where it was last frame
    // can all belong to the other surface and the history would be thrown
    // away every few frames: thin edges, a chair's legs and lace curtains
    // stayed at one or two frames of history and fizzed. Before giving up,
    // look one texel further for any that are this surface.
    var searched = false;
    if settings.reset == 0u && history_weight <= 0.01 {
        let center = base + vec2<i32>(round(f));
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                let tap = center + vec2(x, y);
                if !in_view(tap) { continue; }
                let tap_id = vec2<u32>(tap);
                let tap_depth = textureLoad(previous_depth_buffer, tap_id, 0);
                if tap_depth == 0.0 { continue; }
                let previous_surface = gpixel_resolve(textureLoad(previous_gbuffer, tap_id, 0), tap_depth, tap_id, view.main_pass_viewport.zw, previous_view.world_from_clip);
                let plane_distance = abs(dot(surface.world_normal, previous_surface.world_position - surface.world_position));
                if plane_distance > REPROJECTION_PLANE_TOLERANCE * view_distance { continue; }
                if dot(surface.world_normal, previous_surface.world_normal) < REPROJECTION_NORMAL_COS { continue; }
                history_diffuse += textureLoad(history_diffuse_in, tap_id, 0);
                history_specular += textureLoad(history_specular_in, tap_id, 0);
                history_moments += textureLoad(moments_in, tap_id, 0);
                history_weight += 1.0;
            }
        }
        // The average of whichever matched, each worth a whole texel; the
        // weight is brought back to one below, so the history length is kept.
        if history_weight > 0.0 {
            history_diffuse /= history_weight;
            history_specular /= history_weight;
            history_moments /= history_weight;
            history_weight = 1.0;
            searched = true;
        }
    }

    // While the camera moves the history is read between texels, and bilinear
    // filtering softens it a little every frame; over a 32-frame history that
    // compounds into a visible smear of sharp shadow edges. Where the whole
    // footprint is on this surface, read it again with a Catmull-Rom filter,
    // which keeps edges, and hold the result inside the central texels' range
    // so its overshoot cannot ring.
    let whole_footprint = history_weight > 0.999 && !searched;
    if whole_footprint {
        history_diffuse = vec4(clamp(catmull_rom(history_diffuse_in, base, f), diffuse_min, diffuse_max), history_diffuse.a);
        history_specular = vec4(clamp(catmull_rom(history_specular_in, base, f), specular_min, specular_max), history_specular.a);
    }

    var diffuse_length = 0.0;
    var specular_length = 0.0;
    if history_weight > 0.01 {
        // The Catmull-Rom values are already normalized; only the lengths
        // still carry the weight.
        let w = select(history_weight, 1.0, whole_footprint);
        history_diffuse = vec4(history_diffuse.rgb / w, history_diffuse.a / history_weight);
        history_specular = vec4(history_specular.rgb / w, history_specular.a / history_weight);
        history_moments /= history_weight;
        // A footprint only partly on the same surface is worth less than a
        // whole one.
        diffuse_length = history_diffuse.a * saturate(history_weight);
        specular_length = history_specular.a * saturate(history_weight);
    }

    // A guard against a numerical blow-up, once there is history. Fireflies
    // are dealt with in the blur's first pass (`suppress_speck`).
    if diffuse_length >= SHORT_HISTORY {
        diffuse = clamp_firefly(diffuse, history_diffuse.rgb);
    }
    if specular_length >= SHORT_HISTORY {
        specular = clamp_firefly(specular, history_specular.rgb);
    }

    diffuse_length = min(diffuse_length + 1.0, DIFFUSE_MAX_HISTORY);
    specular_length = min(specular_length + 1.0, specular_history_limit(surface.world_position, surface.material));
    let diffuse_alpha = 1.0 / diffuse_length;
    let specular_alpha = 1.0 / specular_length;

    let accumulated_diffuse = mix(history_diffuse.rgb, diffuse, diffuse_alpha);
    let accumulated_specular = mix(history_specular.rgb, specular, specular_alpha);
    let diffuse_moments = mix(history_moments.xy, signal_moments(diffuse), diffuse_alpha);
    let specular_moments = mix(history_moments.zw, signal_moments(specular), specular_alpha);

    var diffuse_variance = max(diffuse_moments.y - diffuse_moments.x * diffuse_moments.x, 0.0);
    var specular_variance = max(specular_moments.y - specular_moments.x * specular_moments.x, 0.0);
    if diffuse_length < SHORT_HISTORY || specular_length < SHORT_HISTORY {
        let spatial = spatial_variance(pixel_id);
        if diffuse_length < SHORT_HISTORY {
            diffuse_variance = spatial.x * (SHORT_HISTORY / diffuse_length);
        }
        if specular_length < SHORT_HISTORY {
            specular_variance = spatial.y * (SHORT_HISTORY / specular_length);
        }
    }

    textureStore(history_diffuse_out, pixel_id, vec4(accumulated_diffuse, diffuse_length));
    textureStore(history_specular_out, pixel_id, vec4(accumulated_specular, specular_length));
    textureStore(moments_out, pixel_id, vec4(diffuse_moments, specular_moments));
    // What the blur filters is the average of `length` frames, not one frame's
    // sample, so the noise it has to remove is one frame's variance over the
    // frames averaged. The moments measure one frame's; handed on as they
    // were, a converged pixel's luminance edge-stop was some five times too
    // loose, and a noisy dark floor pulled in the sunlit patch beside it as a
    // wide glowing halo and blurred a thin post's shadow away.
    textureStore(filtered_diffuse, pixel_id, vec4(accumulated_diffuse, diffuse_variance / diffuse_length));
    textureStore(filtered_specular, pixel_id, vec4(accumulated_specular, specular_variance / specular_length));
}

/// The most frames a pixel's reflection may average this frame.
///
/// A sharp reflection must forget fast while the camera moves, because it
/// slides across its surface and reprojecting by the surface's own motion puts
/// it in the wrong place ([`specular_max_history`]). But it only slides when the
/// direction it is seen from changes, and with the camera still it does not
/// change at all: capping it at a few frames regardless left glossy floors
/// sparkling in a still view that should settle as smooth as a matte wall. So
/// the cap rises to the full history as the view direction's change since last
/// frame (the parallax) falls below what the reflection's own blur hides, which
/// is about its lobe's width, its linear roughness in radians.
@if(DENOISE_TEMPORAL)
fn specular_history_limit(world_position: vec3<f32>, material: ResolvedMaterial) -> f32 {
    let previous_camera_homogeneous = previous_view.world_from_clip * (previous_view.clip_from_view * vec4(0.0, 0.0, 0.0, 1.0));
    let previous_camera = previous_camera_homogeneous.xyz / previous_camera_homogeneous.w;
    let now = normalize(view.world_position - world_position);
    let before = normalize(previous_camera - world_position);
    let parallax = acos(clamp(dot(now, before), -1.0, 1.0));
    let still = 1.0 - saturate(parallax / (PARALLAX_LOBE_FRACTION * material.roughness + 0.0001));
    return mix(specular_max_history(material.perceptual_roughness), SPECULAR_ROUGH_MAX_HISTORY, still);
}

/// `history`'s colour at `base + f` (in texels, `f` in 0..1) through a 4x4
/// Catmull-Rom filter, taps clamped to the view.
@if(DENOISE_TEMPORAL)
fn catmull_rom(history: texture_2d<f32>, base: vec2<i32>, f: vec2<f32>) -> vec3<f32> {
    let wx = catmull_rom_weights(f.x);
    let wy = catmull_rom_weights(f.y);
    let last = vec2<i32>(view_size()) - 1;
    var sum = vec3(0.0);
    for (var y = 0; y < 4; y++) {
        for (var x = 0; x < 4; x++) {
            let tap = clamp(base + vec2(x - 1, y - 1), vec2(0), last);
            sum += textureLoad(history, vec2<u32>(tap), 0).rgb * wx[x] * wy[y];
        }
    }
    return sum;
}

@if(DENOISE_TEMPORAL)
fn catmull_rom_weights(t: f32) -> vec4<f32> {
    return vec4(
        t * (-0.5 + t * (1.0 - 0.5 * t)),
        1.0 + t * t * (-2.5 + 1.5 * t),
        t * (0.5 + t * (2.0 - 1.5 * t)),
        t * t * (-0.5 + 0.5 * t),
    );
}

@if(DENOISE_TEMPORAL)
fn clamp_firefly(sample: vec3<f32>, history: vec3<f32>) -> vec3<f32> {
    let sample_luminance = luminance(sample);
    let limit = luminance(history) * FIREFLY_LIMIT + 0.01;
    return select(sample, sample * (limit / sample_luminance), sample_luminance > limit);
}

/// Luminance variance of the new diffuse (x) and specular (y) samples over a 3x3
/// neighbourhood, for pixels with too little history to have moments.
@if(DENOISE_TEMPORAL)
fn spatial_variance(pixel_id: vec2<u32>) -> vec2<f32> {
    var sum = vec4(0.0);
    var count = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let tap = vec2<i32>(pixel_id) + vec2(x, y);
            if !in_view(tap) { continue; }
            let tap_id = vec2<u32>(tap);
            if textureLoad(depth_buffer, tap_id, 0) == 0.0 { continue; }
            let d = signal_moments(textureLoad(noisy_diffuse, tap_id, 0).rgb);
            let s = signal_moments(textureLoad(noisy_specular, tap_id, 0).rgb);
            sum += vec4(d, s);
            count += 1.0;
        }
    }
    let mean = sum / max(count, 1.0);
    return vec2(max(mean.y - mean.x * mean.x, 0.0), max(mean.w - mean.z * mean.z, 0.0));
}

// ── A-trous ─────────────────────────────────────────────────────────────────

@if(DENOISE_ATROUS)
@compute @workgroup_size(8, 8, 1)
fn atrous(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel_id = global_id.xy;
    if any(pixel_id >= view_size()) { return; }

    let center_guide = textureLoad(guide, pixel_id, 0);
    if !guide_covered(center_guide) {
        textureStore(destination_diffuse, pixel_id, vec4(0.0));
        textureStore(destination_specular, pixel_id, vec4(0.0));
        return;
    }
    let surface = unpack_guide(center_guide);
    let view_distance = max(length(view.world_position - surface.world_position), 0.0001);
    let roughness = surface.perceptual_roughness;
    let specular_strength = specular_blur_strength(roughness);

    var center_diffuse = textureLoad(source_diffuse, pixel_id, 0);
    var center_specular = textureLoad(source_specular, pixel_id, 0);
    if constants::DENOISE_STEP == 1u {
        center_diffuse = vec4(suppress_speck(source_diffuse, pixel_id, center_diffuse.rgb), center_diffuse.a);
        center_specular = vec4(suppress_speck(source_specular, pixel_id, center_specular.rgb), center_specular.a);
    }
    let center_diffuse_luminance = luminance(center_diffuse.rgb);
    let center_specular_luminance = luminance(center_specular.rgb);
    let diffuse_sigma = LUMINANCE_EDGE_SIGMA * noise_deviation(center_diffuse.a, center_diffuse_luminance);
    let specular_sigma = LUMINANCE_EDGE_SIGMA * noise_deviation(center_specular.a, center_specular_luminance);

    // The 5-tap B3-spline that a-trous dilates.
    let kernel = array<f32, 5>(1.0 / 16.0, 1.0 / 4.0, 3.0 / 8.0, 1.0 / 4.0, 1.0 / 16.0);
    let step = i32(constants::DENOISE_STEP);

    var diffuse_sum = vec3(0.0);
    var diffuse_variance_sum = 0.0;
    var diffuse_weight_sum = 0.0;
    var specular_sum = vec3(0.0);
    var specular_variance_sum = 0.0;
    var specular_weight_sum = 0.0;

    for (var y = -2; y <= 2; y++) {
        for (var x = -2; x <= 2; x++) {
            let tap = vec2<i32>(pixel_id) + vec2(x, y) * step;
            if !in_view(tap) { continue; }
            let tap_id = vec2<u32>(tap);
            let k = kernel[x + 2] * kernel[y + 2];

            var tap_diffuse = center_diffuse;
            var tap_specular = center_specular;
            if x != 0 || y != 0 {
                tap_diffuse = textureLoad(source_diffuse, tap_id, 0);
                tap_specular = textureLoad(source_specular, tap_id, 0);
            }

            var diffuse_weight = k;
            var specular_weight = k;
            if x != 0 || y != 0 {
                let tap_guide = textureLoad(guide, tap_id, 0);
                if !guide_covered(tap_guide) { continue; }
                let tap_surface = unpack_guide(tap_guide);

                let plane_distance = abs(dot(surface.world_normal, tap_surface.world_position - surface.world_position)) / view_distance;
                let plane_weight = exp(-plane_distance * PLANE_EDGE_SHARPNESS);
                let normal_cos = max(dot(surface.world_normal, tap_surface.world_normal), 0.0);

                diffuse_weight *= plane_weight
                    * pow(normal_cos, DIFFUSE_NORMAL_POWER)
                    * exp(-abs(center_diffuse_luminance - luminance(tap_diffuse.rgb)) / diffuse_sigma);

                specular_weight *= plane_weight
                    * pow(normal_cos, SPECULAR_NORMAL_POWER)
                    * exp(-abs(center_specular_luminance - luminance(tap_specular.rgb)) / specular_sigma)
                    * exp(-abs(roughness - tap_surface.perceptual_roughness) * ROUGHNESS_EDGE_SHARPNESS)
                    * specular_strength;
            }

            diffuse_sum += tap_diffuse.rgb * diffuse_weight;
            diffuse_variance_sum += tap_diffuse.a * diffuse_weight * diffuse_weight;
            diffuse_weight_sum += diffuse_weight;
            specular_sum += tap_specular.rgb * specular_weight;
            specular_variance_sum += tap_specular.a * specular_weight * specular_weight;
            specular_weight_sum += specular_weight;
        }
    }

    // The centre tap always counts, so neither sum is zero.
    let filtered_diffuse = vec4(diffuse_sum / diffuse_weight_sum, diffuse_variance_sum / (diffuse_weight_sum * diffuse_weight_sum));
    let filtered_specular = vec4(specular_sum / specular_weight_sum, specular_variance_sum / (specular_weight_sum * specular_weight_sum));
    textureStore(destination_diffuse, pixel_id, filtered_diffuse);
    textureStore(destination_specular, pixel_id, filtered_specular);
}

/// `center` held to at most `SPECK_LIMIT` times the brightest of its eight
/// neighbours in `signal`, the accumulated (not yet blurred) lighting.
///
/// A firefly, a sun glint one ray found through a glossy floor's reflection, is
/// averaged into a pixel's history and stays a bright speck for as long as the
/// history lasts, and the blur's own edge-stop protects it as an edge. Judged
/// here, after accumulation, real light is already smooth and a speck stands
/// out from all its neighbours; judged against raw samples, which are spiky
/// everywhere, the same test cut real light all over. Only what is shown is
/// held down: the history keeps its value, so no light is lost, only hidden
/// until it has been averaged away.
@if(DENOISE_ATROUS)
fn suppress_speck(signal: texture_2d<f32>, pixel_id: vec2<u32>, center: vec3<f32>) -> vec3<f32> {
    var peak = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            if x == 0 && y == 0 { continue; }
            let tap = vec2<i32>(pixel_id) + vec2(x, y);
            if !in_view(tap) { continue; }
            let tap_id = vec2<u32>(tap);
            if !guide_covered(textureLoad(guide, tap_id, 0)) { continue; }
            peak = max(peak, luminance(textureLoad(signal, tap_id, 0).rgb));
        }
    }
    let center_luminance = luminance(center);
    let limit = peak * SPECK_LIMIT + 0.001;
    return select(center, center * (limit / center_luminance), center_luminance > limit);
}

/// Standard deviation for the luminance edge-stop, from a variance, floored
/// relative to the pixel's own brightness. See `RELATIVE_NOISE_FLOOR`.
@if(DENOISE_ATROUS)
fn noise_deviation(variance: f32, pixel_luminance: f32) -> f32 {
    return max(sqrt(max(variance, 0.0)), RELATIVE_NOISE_FLOOR * pixel_luminance) + 0.0001;
}

// ── Composite ───────────────────────────────────────────────────────────────

@if(DENOISE_COMPOSITE)
@compute @workgroup_size(8, 8, 1)
fn composite(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel_id = global_id.xy;
    if any(pixel_id >= view_size()) { return; }

    // Solari does not shade the sky; whatever drew it is left alone.
    let depth = textureLoad(depth_buffer, pixel_id, 0);
    if depth == 0.0 { return; }
    let surface = gpixel_resolve(textureLoad(gbuffer, pixel_id, 0), depth, pixel_id, view.main_pass_viewport.zw, view.world_from_clip);

    let wo = normalize(view.world_position - surface.world_position);
    let factors = demodulation_factors(surface.material, surface.world_normal, wo);
    let lighting = textureLoad(final_diffuse, pixel_id, 0).rgb * factors.diffuse
        + textureLoad(final_specular, pixel_id, 0).rgb * factors.specular;

    // Emissive is added unexposed, as the lighting passes add it (see the
    // emissive patch in restir.wesl). The lighting is already exposed.
    textureStore(view_output, pixel_id, vec4(lighting + surface.material.emissive, 1.0));
}
