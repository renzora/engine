//! Tracy profiler plugin for Renzora.
//!
//! Always linked into every editor / runtime / server binary. The
//! `ondemand` flag on `tracy-client` keeps the profiler fully dormant
//! — no background thread, no ring-buffer allocation, no event capture
//! — until a Tracy GUI actually connects. When that happens:
//!
//! - `bevy/trace_tracy` is active, so `TracyLayer` is wired into Bevy's
//!   `LogPlugin`. Every Bevy system, render-graph node, and any
//!   `info_span!` span in renzora crates shows up as a Tracy zone.
//! - `tracy-client` is linked directly so renzora emits custom plots
//!   (entity_count, frame_time_ms) and explicit zones around the Lumen
//!   hot paths (voxel.clear / inject / resolve, lumen.trace,
//!   geometry.extract_samples / inject).
//!
//! Connect Tracy GUI 0.11.x to localhost to start capturing.

use bevy::prelude::*;

mod plots;

#[derive(Default)]
pub struct TracyPlugin;

impl Plugin for TracyPlugin {
    fn build(&self, app: &mut App) {
        info!(
            "[engine] TracyPlugin — dormant until a Tracy GUI 0.11.x connects to localhost"
        );
        plots::register(app);
    }
}

renzora::add!(TracyPlugin);
