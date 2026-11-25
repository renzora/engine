//! Slow, eased "breathing" scale, like a resting chest. A low frequency plus
//! a squared sine gives a longer pause at the small end, reading as an
//! inhale/exhale rhythm rather than a mechanical pulse.
//!
//! Setup: attach to anything with a Transform.

use bevy::prelude::*;
use renzora::ScriptCtx;
use std::f32::consts::TAU;

/// Exhaled size.
const MIN_SCALE: f32 = 0.9;
/// Inhaled size.
const MAX_SCALE: f32 = 1.1;
/// Breaths per second.
const FREQUENCY: f32 = 0.3;

fn update(ctx: &mut ScriptCtx) {
    let s = (ctx.elapsed() * FREQUENCY * TAU).sin();
    let scale = MIN_SCALE.lerp(MAX_SCALE, s * s);
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.scale = Vec3::splat(scale);
    }
}

renzora::script!(update);
