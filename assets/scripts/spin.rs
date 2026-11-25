//! Rotates the entity continuously around its Y axis. The step is scaled by
//! the frame delta so the spin speed is frame-rate independent (degrees per
//! second), not tied to raw frame count.
//!
//! Setup: attach to anything with a Transform.

use bevy::prelude::*;
use renzora::ScriptCtx;

/// Degrees per second around Y.
const SPEED: f32 = 90.0;

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.rotation *= Quat::from_rotation_y((SPEED * dt).to_radians());
    }
}

renzora::script!(update);
