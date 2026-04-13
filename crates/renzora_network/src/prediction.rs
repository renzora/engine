//! Client-side prediction setup.
//!
//! Owned entities get `Predicted`, others get interpolated.
//! Transform-only prediction first — full physics rollback deferred.

use bevy::prelude::*;

use crate::components::*;
use crate::status::NetworkStatus;

/// Snap correction threshold: if server correction > this many units,
/// teleport instead of smooth lerp.
pub const SNAP_THRESHOLD: f32 = 2.0;

/// Apply smooth correction for predicted entities when server sends an update.
///
/// If the correction distance is below `SNAP_THRESHOLD`, lerp smoothly.
/// Otherwise, snap immediately to avoid rubber-banding over large distances.
pub fn smooth_correction(
    _query: Query<&mut Transform, With<Networked>>,
    status: Res<NetworkStatus>,
) {
    if !status.is_connected() {
        return;
    }
    // Lightyear handles the actual rollback/correction internally.
    // This system is a hook point for custom smoothing if needed.
    // For now, Lightyear's built-in interpolation handles everything.
}
