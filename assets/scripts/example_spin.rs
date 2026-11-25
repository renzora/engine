//! Spins the entity continuously about one local axis, scaled by the frame
//! delta so the rate is the same at any frame rate.
//!
//! Setup: attach to any 3D entity; pick the rate and axis below.

use bevy::prelude::*;
use renzora::ScriptCtx;

/// Degrees per second.
const SPEED: f32 = 45.0;
/// The local axis to spin about.
const AXIS: Vec3 = Vec3::Y;

fn update(ctx: &mut ScriptCtx) {
    let step = (SPEED * ctx.delta()).to_radians();
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.rotation *= Quat::from_axis_angle(AXIS, step);
    }
}

renzora::script!(update);
