// RENZORA PATCH: the layout of the diagnostics' counter buffer, shared by the
// lighting passes (which count their rays) and the debug pass (which gathers the
// frame's statistics). `debug.rs` reads it back and must agree with every value
// here.
//
// Each counter is spread over `DEBUG_LANES` slots, picked by pixel or cell, and
// summed on the CPU. A single slot would have every thread on the GPU adding to
// one address at once, which serializes them and would distort the very timings
// the diagnostics show.

import package::realtime::bindings::{constants, debug_counters};
import package::scene::bindings::rays_traced;
import package::scene::sampling::shadow_rays_traced;

const DEBUG_LANES: u32 = 64u;

/// `SolariLightingSettings::debug_flags`: count the rays each pass traces.
const DEBUG_COUNT_RAYS: u32 = 1u;

// Rays, in pairs: every ray a pass traced, then how many of them were shadow
// (visibility) rays. The rest followed a light path.
const SLOT_RAYS_INITIAL: u32 = 0u;
const SLOT_RAYS_REUSE: u32 = 2u;
const SLOT_RAYS_WORLD_CACHE: u32 = 4u;
const SLOT_RAYS_DEBUG: u32 = 6u;

// The debug pass's statistics.
const SLOT_COVERED: u32 = 8u;
const SLOT_HISTORY_SUM: u32 = 9u;
const SLOT_HISTORY_SHORT: u32 = 10u;
const SLOT_REJECTED: u32 = 11u;
/// Confidence in sixteenths.
const SLOT_CONFIDENCE_SUM: u32 = 12u;
/// Grain in 1024ths, raw then denoised.
const SLOT_GRAIN_RAW: u32 = 13u;
const SLOT_GRAIN_DENOISED: u32 = 14u;
const SLOT_NOT_TRACED: u32 = 15u;
/// Relative error against the reference in 1024ths, and the pixels it covers.
const SLOT_REFERENCE_ERROR: u32 = 16u;
const SLOT_REFERENCE_PIXELS: u32 = 17u;
/// Brightness histograms, `HISTOGRAM_BINS` each, raw then denoised.
const SLOT_HISTOGRAM_RAW: u32 = 18u;
const SLOT_HISTOGRAM_DENOISED: u32 = 50u;
const SLOT_COUNT: u32 = 82u;

const HISTOGRAM_BINS: u32 = 32u;
/// The exposed luminance the first bin starts at, as a power of two, and how
/// many stops each bin spans: 32 bins of half a stop cover 2^-12 to 2^4, from
/// near-black to well past what the tonemapper shows as white.
const HISTOGRAM_MIN_LOG2: f32 = -12.0;
const HISTOGRAM_STOPS_PER_BIN: f32 = 0.5;

fn debug_add(slot: u32, lane_seed: u32, value: u32) {
    if value != 0u {
        atomicAdd(&debug_counters[slot * DEBUG_LANES + lane_seed % DEBUG_LANES], value);
    }
}

/// Add this thread's rays to the pass's counters, when they are being counted.
/// Called once at the end of every pass that traces, since a counter per ray
/// would cost an atomic per ray.
fn flush_ray_counts(slot: u32, lane_seed: u32) {
    if (constants.debug_flags & DEBUG_COUNT_RAYS) == 0u { return; }
    debug_add(slot, lane_seed, rays_traced);
    debug_add(slot + 1u, lane_seed, shadow_rays_traced);
    rays_traced = 0u;
    shadow_rays_traced = 0u;
}
