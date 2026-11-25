//! Orbits the world origin in the XZ plane at a fixed radius. The position is
//! recomputed from elapsed time each frame (absolute, not incremental) so
//! floating-point drift can never let the orbit spiral in or out.
//!
//! Setup: attach to anything with a Transform.

use bevy::prelude::*;
use renzora::ScriptCtx;
use std::f32::consts::TAU;

const RADIUS: f32 = 3.0;
/// Revolutions per second.
const SPEED: f32 = 0.5;
/// Height of the orbit plane.
const HEIGHT: f32 = 1.0;

fn update(ctx: &mut ScriptCtx) {
    let a = ctx.elapsed() * SPEED * TAU;
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation = Vec3::new(a.cos() * RADIUS, HEIGHT, a.sin() * RADIUS);
    }
}

renzora::script!(update);
