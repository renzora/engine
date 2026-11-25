//! Steady clockwise yaw from an absolute angle. Setting the yaw directly
//! from elapsed time (rather than accumulating small rotations) keeps the
//! angle exact; the negative sign makes it clockwise seen from above.
//!
//! Setup: attach to anything with a Transform.

use bevy::prelude::*;
use renzora::ScriptCtx;

/// Degrees per second, clockwise.
const SPEED: f32 = 60.0;

fn update(ctx: &mut ScriptCtx) {
    let yaw = (-(ctx.elapsed() * SPEED)).rem_euclid(360.0).to_radians();
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        let (_, pitch, roll) = transform.rotation.to_euler(EulerRot::YXZ);
        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
    }
}

renzora::script!(update);
