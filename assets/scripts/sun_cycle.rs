//! Day and night cycle: the sun's angles come straight from elapsed time.
//! Elapsed time is monotonic, so deriving the angle from it (rather than
//! accumulating a delta) is drift-free.
//!
//! Setup: attach to any entity; the scene needs a sun light.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::ScriptCtx;

/// Seconds per full day.
const DAY_LENGTH: f32 = 60.0;
/// Highest elevation in degrees, reached at midday.
const PEAK_ELEVATION: f32 = 80.0;

fn update(ctx: &mut ScriptCtx) {
    // One full revolution per day; elevation rides a sine so the sun rises,
    // peaks, and sets below the horizon.
    let frac = ctx.elapsed().rem_euclid(DAY_LENGTH) / DAY_LENGTH;
    let azimuth = frac * 360.0;
    let elevation = (frac * std::f32::consts::TAU).sin() * PEAK_ELEVATION;
    let me = ctx.entity();
    ScriptAction::new("set_sun_angles", me)
        .arg("azimuth", azimuth)
        .arg("elevation", elevation)
        .send(ctx.world());
}

renzora::script!(update);
