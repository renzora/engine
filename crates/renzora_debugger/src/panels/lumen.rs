//! Lumen diagnostics state.
//!
//! Two layers:
//!   - **CPU bake** — `bake_mesh_samples` runs each frame on the main
//!     world, throttled to `MAX_BAKES_PER_FRAME` entities. We capture
//!     last/avg/max wall-clock plus sample-emission counts so the user
//!     can tell when the throttle is the bottleneck on first-time scene
//!     load (or after a large mesh change).
//!   - **Per-camera state** — `VoxelCacheView` flags (`inject_active`,
//!     `debug_active`) tell you which camera is actually driving the
//!     GI passes, and the count of `MeshVoxelSamples` entities tells
//!     you how much geometry the cache currently has coverage for.
//!
//! Rendered by the native (ember) Lumen panel in [`crate::native`].

use renzora_lumen::LumenBakeStats;

/// Per-frame snapshot updated by `update_lumen_diag_state`. Holds
/// everything renderable in the panel so the UI side stays a pure
/// reader.
#[derive(bevy::prelude::Resource, Default, Clone)]
pub struct LumenDiagState {
    pub cameras: Vec<LumenCameraEntry>,
    pub mesh_voxel_samples_entities: usize,
    pub has_sky_cubemap: bool,
    pub bake: LumenBakeStats,
}

#[derive(Clone)]
pub struct LumenCameraEntry {
    pub camera_name: String,
    pub inject_active: bool,
    pub debug_active: bool,
}
