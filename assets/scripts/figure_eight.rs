//! Traces a lemniscate (figure eight) in the XZ plane. X follows sin(t) and Z
//! follows sin(2t); the 2:1 frequency ratio is what crosses the path through
//! the centre to make the eight. Positions are absolute, derived from elapsed
//! time, so the path never drifts.
//!
//! Setup: attach to any entity; it moves around the world origin.

use bevy::prelude::*;
use renzora::ScriptCtx;

/// Path extent along X; Z spans half of it.
const SIZE: f32 = 2.0;
/// Loops per second.
const SPEED: f32 = 0.4;
const HEIGHT: f32 = 1.0;

fn update(ctx: &mut ScriptCtx) {
    let a = ctx.elapsed() * SPEED * std::f32::consts::TAU;
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.translation = Vec3::new(a.sin() * SIZE, HEIGHT, (a * 2.0).sin() * SIZE * 0.5);
}

renzora::script!(update);
