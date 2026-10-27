// RENZORA PATCH: diagnostics for Solari's realtime lighting, so its quality can be
// measured and its failures seen directly rather than guessed at from the final
// image. Driven by `SolariDebug` on a camera; see `debug.rs`.
//
// Four passes, each its own pipeline, all reading the lighting's own bindings
// (group 1) and the ray-traced scene (group 0):
//
//   snapshot_raw     copy the lighting as written, before the denoiser, so the
//                    raw estimate can be shown and measured beside the result.
//   stats            per pixel: history length, rejection, ReSTIR confidence,
//                    grain, brightness histograms, pixels the traced scene does
//                    not match, and error against the reference. Summed into
//                    the counter buffer the CPU reads back.
//   debug_view       replace the image with the chosen visualization.
//   reference_trace  one more sample of a conventional path tracer into a
//                    running average: the converged image every measurement is
//                    taken against. Its loop is upstream's offline path tracer
//                    (`pathtracer/pathtracer.wesl`), kept in step with it.

import bevy_core_pipeline::tonemapping::tonemapping_luminance as luminance;
import bevy_pbr::render::utils::{rand_f, rand_vec2f};
import package::realtime::bindings::{constants, depth_buffer, gbuffer, view, view_output};
@if(RESTIR) import package::realtime::bindings::reservoirs_a;
import package::realtime::gbuffer_utils::{gpixel_resolve, reconstruct_world_position};
import package::realtime::world_cache_query::{query_world_cache, WORLD_CACHE_CELL_LIFETIME};
import package::realtime::debug_layout::{
    debug_add, flush_ray_counts, HISTOGRAM_BINS, HISTOGRAM_MIN_LOG2, HISTOGRAM_STOPS_PER_BIN,
    SLOT_CONFIDENCE_SUM, SLOT_COVERED, SLOT_GRAIN_DENOISED, SLOT_GRAIN_RAW, SLOT_HISTOGRAM_DENOISED,
    SLOT_HISTOGRAM_RAW, SLOT_HISTORY_SHORT, SLOT_HISTORY_SUM, SLOT_NOT_TRACED, SLOT_RAYS_DEBUG,
    SLOT_REFERENCE_ERROR, SLOT_REFERENCE_PIXELS, SLOT_REJECTED,
};
import package::scene::brdf::{evaluate_brdf, evaluate_and_sample_brdf, brdf_pdf, F_AB};
import package::scene::sampling::{sample_random_light, random_emissive_light_pdf, power_heuristic, sample_environment_map_light};
import package::scene::bindings::{trace_ray, resolve_ray_hit_full, RAY_T_MIN, RAY_T_MAX, MIRROR_ROUGHNESS_THRESHOLD};

enable wgpu_ray_query;

@group(2) @binding(0) var history_diffuse: texture_2d<f32>;
@group(2) @binding(1) var moments: texture_2d<f32>;
@group(2) @binding(2) var raw_image: texture_storage_2d<rgba16float, read_write>;
@group(2) @binding(3) var reference: texture_storage_2d<rgba32float, read_write>;
@group(2) @binding(4) var<uniform> debug: DebugSettings;

/// Must match `DebugUniforms` in `debug.rs`.
struct DebugSettings {
    view: u32,
    flags: u32,
    split: f32,
    /// Samples already in the reference; the next is averaged in with weight
    /// 1 / (n + 1), and 0 starts it over.
    reference_samples: u32,
}

// `DebugSettings::view`. Must match `SolariDebugView` in `debug.rs`.
const VIEW_RAW_SPLIT: u32 = 1u;
const VIEW_HISTORY_LENGTH: u32 = 2u;
const VIEW_REJECTED: u32 = 3u;
const VIEW_NOISE_ESTIMATE: u32 = 4u;
const VIEW_RESERVOIR_CONFIDENCE: u32 = 5u;
const VIEW_WORLD_CACHE: u32 = 6u;
const VIEW_NOT_TRACED: u32 = 7u;
const VIEW_REFERENCE: u32 = 8u;
const VIEW_REFERENCE_ERROR: u32 = 9u;

// `DebugSettings::flags`. Must match `debug.rs`.
const FLAG_DENOISER_ACTIVE: u32 = 1u;
const FLAG_REFERENCE_VALID: u32 = 2u;

/// The denoiser's longest diffuse history (`DIFFUSE_MAX_HISTORY`), the top of
/// the history-length scale.
const MAX_HISTORY: f32 = 32.0;

/// A history this short is still mostly noise: the denoiser's own threshold for
/// trusting its moments (`SHORT_HISTORY`).
const SHORT_HISTORY: f32 = 4.0;

/// Relative noise at which the noise view saturates. One sample of indirect
/// light indoors is commonly noisier than the light itself, so a scale topping
/// out at 1 showed every such surface the same red.
const NOISE_VIEW_MAX: f32 = 4.0;

/// Relative error against the reference at which the error view saturates.
const ERROR_VIEW_MAX: f32 = 0.5;

/// The most a pixel adds to the grain and error sums, as a relative value, so
/// one black or blown-out pixel cannot swamp a frame's average.
const MAX_RELATIVE: f32 = 4.0;

fn view_size() -> vec2<u32> {
    return vec2<u32>(view.main_pass_viewport.zw);
}

fn pixel_index(pixel_id: vec2<u32>) -> u32 {
    return pixel_id.x + pixel_id.y * view_size().x;
}

/// Blue (0) through cyan, green and yellow to red (1).
fn heat(t: f32) -> vec3<f32> {
    let x = saturate(t);
    return saturate(vec3(1.5 - abs(4.0 * x - 3.0), 1.5 - abs(4.0 * x - 2.0), 1.5 - abs(4.0 * x - 1.0)));
}

/// The image under an overlay: its brightness, dimmed, so a tint reads clearly.
fn backdrop(color: vec3<f32>) -> vec3<f32> {
    return vec3(luminance(color) * 0.4);
}

/// Whether the ray-traced scene disagrees with what was drawn at this pixel: a
/// ray from the camera through it misses, or stops well short of or beyond the
/// drawn surface. That is a mesh left out of the traced scene (no UVs, an
/// unsupported material, a hidden or excluded entity) or traced differently
/// (an alpha-tested leaf traced as a solid card), and it is why a surface can
/// look right and still cast no shadow or catch light it should not.
fn traced_mismatch(pixel_id: vec2<u32>, depth: f32) -> bool {
    let drawn = reconstruct_world_position(pixel_id, depth, view.main_pass_viewport.zw, view.world_from_clip);
    let to_surface = drawn - view.world_position;
    let distance_to_surface = length(to_surface);
    let ray = trace_ray(view.world_position, to_surface / distance_to_surface, 0.0, RAY_T_MAX, RAY_FLAG_NONE);
    if ray.kind == RAY_QUERY_INTERSECTION_NONE {
        return true;
    }
    // Generous enough for virtual geometry's simplified levels, drawn up to a
    // pixel or two off the traced full detail.
    let tolerance = max(0.02 * distance_to_surface, 0.05);
    return abs(ray.t - distance_to_surface) > tolerance;
}

/// Relative difference of a pixel from the mean of its 3x3 neighbourhood, in
/// whichever image `raw` picks: the grain the eye sees.
fn local_grain(pixel_id: vec2<u32>, center: f32, raw: bool) -> f32 {
    var sum = 0.0;
    var count = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let tap = vec2<i32>(pixel_id) + vec2(x, y);
            if any(tap < vec2(0)) || any(tap >= vec2<i32>(view_size())) { continue; }
            let tap_id = vec2<u32>(tap);
            if textureLoad(depth_buffer, tap_id, 0) == 0.0 { continue; }
            var color: vec3<f32>;
            if raw {
                color = textureLoad(raw_image, tap_id).rgb;
            } else {
                color = textureLoad(view_output, tap_id).rgb;
            }
            sum += luminance(color);
            count += 1.0;
        }
    }
    let mean = sum / max(count, 1.0);
    return min(abs(center - mean) / max(mean, 1e-4), MAX_RELATIVE);
}

fn histogram_bin(exposed_luminance: f32) -> u32 {
    let stops = (log2(max(exposed_luminance, 1e-8)) - HISTOGRAM_MIN_LOG2) / HISTOGRAM_STOPS_PER_BIN;
    return u32(clamp(stops, 0.0, f32(HISTOGRAM_BINS - 1u)));
}

/// The reference at this pixel, exposed like the live image. It is stored
/// unexposed so it survives an exposure change.
fn reference_color(pixel_id: vec2<u32>) -> vec3<f32> {
    return textureLoad(reference, pixel_id).rgb * view.exposure;
}

/// Exposed luminance under which a difference from the reference is measured
/// against this floor rather than against the reference itself: near black, a
/// tiny difference would otherwise read as hundreds of percent.
const REFERENCE_ERROR_FLOOR: f32 = 0.02;

/// How far the image is from the reference at this pixel, relative to the
/// reference's brightness. Taken over the 3x3 neighbourhood of both, because a
/// reference only a few hundred samples in is still grainy pixel by pixel, and
/// compared pixel by pixel its own noise would be most of the error measured.
///
/// `from_copy` reads the image from `raw_image` rather than `view_output`: the
/// error view overwrites `view_output` as it goes, so a neighbour read from it
/// could already be a heat colour, and the node copies the finished image there
/// first.
fn reference_error(pixel_id: vec2<u32>, from_copy: bool) -> f32 {
    var image = 0.0;
    var expected = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let tap = vec2<u32>(clamp(vec2<i32>(pixel_id) + vec2(x, y), vec2(0), vec2<i32>(view_size()) - 1));
            if from_copy {
                image += luminance(textureLoad(raw_image, tap).rgb);
            } else {
                image += luminance(textureLoad(view_output, tap).rgb);
            }
            expected += luminance(reference_color(tap));
        }
    }
    return min(abs(image - expected) / 9.0 / (expected / 9.0 + REFERENCE_ERROR_FLOOR), MAX_RELATIVE);
}

/// The largest ReSTIR confidence a reservoir holds after the spatial merge: the
/// temporal history is capped, and the merge adds a neighbour's capped history
/// and new sample on top.
fn max_confidence() -> f32 {
    return 2.0 * max(constants.confidence_weight_cap, 1.0) + 1.0;
}

@compute @workgroup_size(8, 8, 1)
fn snapshot_raw(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel_id = global_id.xy;
    if any(pixel_id >= view_size()) { return; }
    textureStore(raw_image, pixel_id, textureLoad(view_output, pixel_id));
}

@compute @workgroup_size(8, 8, 1)
fn stats(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel_id = global_id.xy;
    if any(pixel_id >= view_size()) { return; }
    let depth = textureLoad(depth_buffer, pixel_id, 0);
    if depth == 0.0 { return; }
    let lane = pixel_index(pixel_id);

    debug_add(SLOT_COVERED, lane, 1u);

    if (debug.flags & FLAG_DENOISER_ACTIVE) != 0u {
        let history = textureLoad(history_diffuse, pixel_id, 0).a;
        debug_add(SLOT_HISTORY_SUM, lane, u32(round(history)));
        debug_add(SLOT_HISTORY_SHORT, lane, u32(history < SHORT_HISTORY));
        debug_add(SLOT_REJECTED, lane, u32(history <= 1.0));
    }

    @if(RESTIR) {
        let confidence = reservoirs_a[lane].confidence_weight;
        debug_add(SLOT_CONFIDENCE_SUM, lane, u32(round(confidence * 16.0)));
    }

    let raw = luminance(textureLoad(raw_image, pixel_id).rgb);
    let denoised = luminance(textureLoad(view_output, pixel_id).rgb);
    debug_add(SLOT_HISTOGRAM_RAW + histogram_bin(raw), lane, 1u);
    debug_add(SLOT_HISTOGRAM_DENOISED + histogram_bin(denoised), lane, 1u);
    debug_add(SLOT_GRAIN_RAW, lane, u32(round(local_grain(pixel_id, raw, true) * 1024.0)));
    debug_add(SLOT_GRAIN_DENOISED, lane, u32(round(local_grain(pixel_id, denoised, false) * 1024.0)));

    if traced_mismatch(pixel_id, depth) {
        debug_add(SLOT_NOT_TRACED, lane, 1u);
    }

    if (debug.flags & FLAG_REFERENCE_VALID) != 0u {
        debug_add(SLOT_REFERENCE_ERROR, lane, u32(round(reference_error(pixel_id, false) * 1024.0)));
        debug_add(SLOT_REFERENCE_PIXELS, lane, 1u);
    }

    flush_ray_counts(SLOT_RAYS_DEBUG, lane);
}

@compute @workgroup_size(8, 8, 1)
fn debug_view(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel_id = global_id.xy;
    if any(pixel_id >= view_size()) { return; }
    let depth = textureLoad(depth_buffer, pixel_id, 0);
    let shaded = textureLoad(view_output, pixel_id).rgb;
    var color = shaded;

    switch debug.view {
        case VIEW_RAW_SPLIT: {
            let split_x = u32(debug.split * f32(view_size().x));
            if pixel_id.x < split_x {
                color = textureLoad(raw_image, pixel_id).rgb;
            }
            if pixel_id.x == split_x {
                color = vec3(1.0);
            }
        }
        case VIEW_HISTORY_LENGTH: {
            if depth != 0.0 {
                color = heat(textureLoad(history_diffuse, pixel_id, 0).a / MAX_HISTORY);
            }
        }
        case VIEW_REJECTED: {
            color = backdrop(shaded);
            if depth != 0.0 && textureLoad(history_diffuse, pixel_id, 0).a <= 1.0 {
                color = vec3(1.0, 0.1, 0.05);
            }
        }
        case VIEW_NOISE_ESTIMATE: {
            if depth != 0.0 {
                let m = textureLoad(moments, pixel_id, 0).xy;
                let deviation = sqrt(max(m.y - m.x * m.x, 0.0));
                color = heat(deviation / max(m.x, 1e-4) / NOISE_VIEW_MAX);
            }
        }
        case VIEW_RESERVOIR_CONFIDENCE: {
            @if(RESTIR) {
                if depth != 0.0 {
                    color = heat(reservoirs_a[pixel_index(pixel_id)].confidence_weight / max_confidence());
                }
            }
        }
        case VIEW_WORLD_CACHE: {
            if depth != 0.0 {
                var rng = pixel_index(pixel_id) + constants.frame_rng;
                let surface = gpixel_resolve(textureLoad(gbuffer, pixel_id, 0), depth, pixel_id, view.main_pass_viewport.zw, view.world_from_clip);
                color = query_world_cache(surface.world_position, surface.world_normal, view.world_position, RAY_T_MAX, WORLD_CACHE_CELL_LIFETIME, &rng) * view.exposure;
            }
        }
        case VIEW_NOT_TRACED: {
            color = backdrop(shaded);
            if depth != 0.0 && traced_mismatch(pixel_id, depth) {
                color = vec3(1.0, 0.0, 1.0);
            }
        }
        case VIEW_REFERENCE: {
            if (debug.flags & FLAG_REFERENCE_VALID) != 0u {
                color = reference_color(pixel_id);
            }
        }
        case VIEW_REFERENCE_ERROR: {
            if (debug.flags & FLAG_REFERENCE_VALID) != 0u {
                color = heat(reference_error(pixel_id, true) / ERROR_VIEW_MAX);
            }
        }
        default: {}
    }

    textureStore(view_output, pixel_id, vec4(color, 1.0));
}

@compute @workgroup_size(8, 8, 1)
fn reference_trace(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel_id = global_id.xy;
    if any(pixel_id >= view_size()) { return; }

    let samples = debug.reference_samples;
    var rng = pixel_index(pixel_id) + samples * 5782582u;

    // Shoot the first ray from the camera
    let pixel_center = vec2<f32>(pixel_id) + 0.5;
    let jitter = rand_vec2f(&rng) - 0.5;
    let pixel_uv = (pixel_center + jitter) / view.main_pass_viewport.zw;
    let pixel_ndc = (pixel_uv * 2.0) - 1.0;
    let primary_ray_target = view.world_from_clip * vec4(pixel_ndc.x, -pixel_ndc.y, 1.0, 1.0);
    var ray_origin = view.world_position;
    var ray_direction = normalize((primary_ray_target.xyz / primary_ray_target.w) - ray_origin);
    var ray_t_min = 0.0;

    // Path trace
    var radiance = vec3(0.0);
    var throughput = vec3(1.0);
    var p_bounce = 0.0;
    loop {
        let ray = trace_ray(ray_origin, ray_direction, ray_t_min, RAY_T_MAX, RAY_FLAG_NONE);
        if ray.kind != RAY_QUERY_INTERSECTION_NONE {
            let ray_hit = resolve_ray_hit_full(ray);
            let wo = -ray_direction;
            let NdotV = max(dot(ray_hit.world_normal, wo), 0.0001);
            let F_ab = F_AB(ray_hit.material.perceptual_roughness, NdotV);

            // Emissive contribution
            var mis_weight = 1.0;
            if p_bounce != 0.0 { // Not first bounce
                let p_light = random_emissive_light_pdf(ray_hit, ray.t, NdotV);
                mis_weight = power_heuristic(p_bounce, p_light);
            }
            if dot(ray_hit.world_normal, wo) > 0.0 {
                radiance += mis_weight * throughput * ray_hit.material.emissive;
            }

            // Sample direct lighting, but only if the surface is not mirror-like
            let is_perfectly_specular = ray_hit.material.roughness <= MIRROR_ROUGHNESS_THRESHOLD && ray_hit.material.metallic > 0.9999;
            if !is_perfectly_specular {
                let direct_lighting = sample_random_light(ray_hit.world_position, ray_hit.world_normal, &rng);

                mis_weight = 1.0;
                if direct_lighting.brdf_rays_can_hit {
                    let pdf_of_bounce = brdf_pdf(wo, direct_lighting.wi, ray_hit.world_normal, ray_hit.material, F_ab);
                    mis_weight = power_heuristic(1.0 / direct_lighting.inverse_solid_angle_pdf, pdf_of_bounce);
                }

                let direct_lighting_brdf = evaluate_brdf(wo, direct_lighting.wi, ray_hit.world_normal, ray_hit.material, F_ab);
                radiance += mis_weight * throughput * direct_lighting.radiance * direct_lighting.inverse_pdf * direct_lighting_brdf;
            }

            // Sample new ray direction from the material BRDF for next bounce and apply BRDF
            let next_bounce = evaluate_and_sample_brdf(wo, ray_hit.world_normal, ray_hit.material, F_ab, &rng);
            if next_bounce.pdf == 0.0 { break; }
            ray_direction = next_bounce.wi;
            ray_origin = ray_hit.world_position + (ray_hit.geometric_world_normal * RAY_T_MIN);
            ray_t_min = RAY_T_MIN;
            p_bounce = next_bounce.pdf;
            throughput *= next_bounce.throughput;

            // Russian roulette for early termination
            let rr = saturate(luminance(throughput));
            if rand_f(&rng) >= rr { break; }
            throughput /= rr;
        } else {
            radiance += throughput * sample_environment_map_light(ray_direction);
            break;
        }
    }

    // Unexposed, unlike upstream: the editor's auto-exposure moves while the
    // reference converges, and an average of differently exposed samples would
    // match no frame. `reference_color` exposes it when read.
    let previous = select(textureLoad(reference, pixel_id).rgb, vec3(0.0), samples == 0u);
    let averaged = mix(previous, radiance, 1.0 / f32(samples + 1u));
    textureStore(reference, pixel_id, vec4(averaged, 1.0));

    flush_ray_counts(SLOT_RAYS_DEBUG, pixel_index(pixel_id));
}
