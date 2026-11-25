//! Pulses uniform scale with a sine wave. Centred on 1.0 so the entity grows
//! and shrinks symmetrically around its authored size rather than only ever
//! growing.
//!
//! Setup: attach to anything with a Transform.

use bevy::prelude::*;
use renzora::ScriptCtx;
use std::f32::consts::TAU;

/// Scale variation either side of 1.0.
const AMOUNT: f32 = 0.2;
/// Cycles per second.
const FREQUENCY: f32 = 1.5;

fn update(ctx: &mut ScriptCtx) {
    let s = 1.0 + (ctx.elapsed() * FREQUENCY * TAU).sin() * AMOUNT;
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.scale = Vec3::splat(s);
    }
}

renzora::script!(update);
