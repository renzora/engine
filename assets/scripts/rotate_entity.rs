//! Spins the entity around any combination of axes. Speeds are degrees per
//! second scaled by delta, so the rate is the same at 30 or 240 FPS. An axis
//! at 0 stays still; all three together tumble.
//!
//! Setup: attach to anything with a Transform.

use bevy::prelude::*;
use renzora::ScriptCtx;

/// Degrees per second around X (pitch), Y (yaw) and Z (roll).
const SPEED: Vec3 = Vec3::new(0.0, 90.0, 0.0);
/// False freezes the rotation.
const ENABLED: bool = true;

fn update(ctx: &mut ScriptCtx) {
    if !ENABLED {
        return;
    }
    let step = SPEED * ctx.delta();
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.rotation *= Quat::from_euler(
            EulerRot::YXZ,
            step.y.to_radians(),
            step.x.to_radians(),
            step.z.to_radians(),
        );
    }
}

renzora::script!(update);
