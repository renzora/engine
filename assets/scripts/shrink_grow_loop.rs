//! Slow uniform shrink and grow that loops forever. A cosine starts the cycle
//! at its peak (cos(0) = 1), so the entity begins at the largest size and
//! eases down first instead of popping on spawn.
//!
//! Setup: attach to any entity.

use bevy::prelude::*;
use renzora::ScriptCtx;

const MIN_SCALE: f32 = 0.5;
const MAX_SCALE: f32 = 1.5;
/// Cycles per second.
const FREQUENCY: f32 = 0.25;

fn update(ctx: &mut ScriptCtx) {
    let t = (ctx.elapsed() * FREQUENCY * std::f32::consts::TAU).cos() * 0.5 + 0.5;
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.scale = Vec3::splat(MIN_SCALE.lerp(MAX_SCALE, t));
}

renzora::script!(update);
