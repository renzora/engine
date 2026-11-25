//! Slow constant drift along a fixed direction. The step is scaled by the
//! frame delta so the entity glides at a steady world-units-per-second pace
//! regardless of frame rate; nothing pulls it back, so it wanders off.
//!
//! Setup: attach to anything with a Transform.

use bevy::prelude::*;
use renzora::ScriptCtx;

/// Drift in world units per second.
const DIRECTION: Vec3 = Vec3::new(0.1, 0.0, 0.05);

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation += DIRECTION * dt;
    }
}

renzora::script!(update);
