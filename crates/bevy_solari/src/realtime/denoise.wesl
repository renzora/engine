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
//             estimate so converged pixels are left alone. The first pass's
//             output becomes next frame's history, as in SVGF, which feeds the
//             blur back into the accumulation.
//   composite multiply the material factors back in and add emissive.
//
// Written from the published papers; no code is taken from any denoiser SDK.

import bevy_core_pipeline::tonemapping::tonemapping_luminance as luminance;
import bevy_render::view::View;
import bevy_pbr::prepass::bindings::PreviousViewUniforms;
import package::realtime::gbuffer_utils::gpixel_resolve;
import package::realtime::denoise_utils::{demodulation_factors, signal_moments};

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var gbuffer: texture_2d<u32>;
@group(0) @binding(2) var depth_buffer: texture_depth_2d;

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
}

@if(DENOISE_ATROUS) {
@group(0) @binding(3) var source_diffuse: texture_2d<f32>;
@group(0) @binding(4) var source_specular: texture_2d<f32>;
@group(0) @binding(5) var destination_diffuse: texture_storage_2d<rgba16float, write>;
@group(0) @binding(6) var destination_specular: texture_storage_2d<rgba16float, write>;
}
@if(DENOISE_ATROUS && DENOISE_FEEDBACK) {
@group(0) @binding(7) var history_diffuse: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(8) var history_specular: texture_storage_2d<rgba16float, read_write>;
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

/// A history tap is the same surface when it lies within this fraction of its
/// distance from the camera of the current pixel's plane.
const REPROJECTION_PLANE_TOLERANCE: f32 = 0.01;

/// A history tap is the same surface when its normal is within about 25 degrees.
const REPROJECTION_NORMAL_COS: f32 = 0.9;

/// Below this history length the moments are too few to estimate noise from, and
/// a 3x3 neighbourhood estimate is used instead.
const SHORT_HISTORY: f32 = 4.0;

/// How much brighter than its history a new sample may be once the history is
/// established. A single ray that finds a tiny bright light makes a pixel
/// hundreds of times too bright for one frame; averaged in, that stays visible
/// as a speck for dozens of frames.
const FIREFLY_LIMIT: f32 = 8.0;

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
        return;
    }
    let surface = gpixel_resolve(textureLoad(gbuffer, pixel_id, 0), depth, pixel_id, view.main_pass_viewport.zw, view.world_from_clip);
    let view_distance = max(length(view.world_position - surface.world_position), 0.0001);

    var diffuse = textureLoad(noisy_diffuse, pixel_id, 0).rgb;
    var specular = textureLoad(noisy_specular, pixel_id, 0).rgb;

    // Bilinear reprojection, each of the four taps tested against the current
    // surface, so an edge does not pull in history from what was behind it.
    var history_diffuse = vec4(0.0);
    var history_specular = vec4(0.0);
    var history_moments = vec4(0.0);
    var history_weight = 0.0;
    if settings.reset == 0u {
        let motion_vector = textureLoad(motion_vectors, pixel_id, 0).xy;
        let previous_position = vec2<f32>(pixel_id) + 0.5 - motion_vector * view.main_pass_viewport.zw - 0.5;
        let base = vec2<i32>(floor(previous_position));
        let f = fract(previous_position);
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

            history_diffuse += textureLoad(history_diffuse_in, tap_id, 0) * bilinear[i];
            history_specular += textureLoad(history_specular_in, tap_id, 0) * bilinear[i];
            history_moments += textureLoad(moments_in, tap_id, 0) * bilinear[i];
            history_weight += bilinear[i];
        }
    }

    var diffuse_length = 0.0;
    var specular_length = 0.0;
    if history_weight > 0.01 {
        history_diffuse /= history_weight;
        history_specular /= history_weight;
        history_moments /= history_weight;
        // A footprint only partly on the same surface is worth less than a
        // whole one.
        diffuse_length = history_diffuse.a * saturate(history_weight);
        specular_length = history_specular.a * saturate(history_weight);
    }

    // Firefly clamp, once there is history to compare with.
    if diffuse_length >= SHORT_HISTORY {
        diffuse = clamp_firefly(diffuse, history_diffuse.rgb);
    }
    if specular_length >= SHORT_HISTORY {
        specular = clamp_firefly(specular, history_specular.rgb);
    }

    diffuse_length = min(diffuse_length + 1.0, DIFFUSE_MAX_HISTORY);
    specular_length = min(specular_length + 1.0, specular_max_history(surface.material.perceptual_roughness));
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
    textureStore(filtered_diffuse, pixel_id, vec4(accumulated_diffuse, diffuse_variance));
    textureStore(filtered_specular, pixel_id, vec4(accumulated_specular, specular_variance));
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

    let depth = textureLoad(depth_buffer, pixel_id, 0);
    if depth == 0.0 {
        textureStore(destination_diffuse, pixel_id, vec4(0.0));
        textureStore(destination_specular, pixel_id, vec4(0.0));
        return;
    }
    let surface = gpixel_resolve(textureLoad(gbuffer, pixel_id, 0), depth, pixel_id, view.main_pass_viewport.zw, view.world_from_clip);
    let view_distance = max(length(view.world_position - surface.world_position), 0.0001);
    let roughness = surface.material.perceptual_roughness;
    let specular_strength = specular_blur_strength(roughness);

    let center_diffuse = textureLoad(source_diffuse, pixel_id, 0);
    let center_specular = textureLoad(source_specular, pixel_id, 0);
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

            let tap_diffuse = textureLoad(source_diffuse, tap_id, 0);
            let tap_specular = textureLoad(source_specular, tap_id, 0);

            var diffuse_weight = k;
            var specular_weight = k;
            if x != 0 || y != 0 {
                let tap_depth = textureLoad(depth_buffer, tap_id, 0);
                if tap_depth == 0.0 { continue; }
                let tap_surface = gpixel_resolve(textureLoad(gbuffer, tap_id, 0), tap_depth, tap_id, view.main_pass_viewport.zw, view.world_from_clip);

                let plane_distance = abs(dot(surface.world_normal, tap_surface.world_position - surface.world_position)) / view_distance;
                let plane_weight = exp(-plane_distance * PLANE_EDGE_SHARPNESS);
                let normal_cos = max(dot(surface.world_normal, tap_surface.world_normal), 0.0);

                diffuse_weight *= plane_weight
                    * pow(normal_cos, DIFFUSE_NORMAL_POWER)
                    * exp(-abs(center_diffuse_luminance - luminance(tap_diffuse.rgb)) / diffuse_sigma);

                specular_weight *= plane_weight
                    * pow(normal_cos, SPECULAR_NORMAL_POWER)
                    * exp(-abs(center_specular_luminance - luminance(tap_specular.rgb)) / specular_sigma)
                    * exp(-abs(roughness - tap_surface.material.perceptual_roughness) * ROUGHNESS_EDGE_SHARPNESS)
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

    // Next frame accumulates onto the once-filtered result rather than the raw
    // average, which is what keeps the temporal stage from re-learning the same
    // noise every frame. The history length stays what the temporal pass wrote.
    @if(DENOISE_FEEDBACK) {
        let diffuse_length = textureLoad(history_diffuse, pixel_id).a;
        let specular_length = textureLoad(history_specular, pixel_id).a;
        textureStore(history_diffuse, pixel_id, vec4(filtered_diffuse.rgb, diffuse_length));
        textureStore(history_specular, pixel_id, vec4(filtered_specular.rgb, specular_length));
    }
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
