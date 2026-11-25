//! Continuously faces the world origin. Re-aiming every frame means that if
//! the entity is moved (by a parent, physics, or another script) it keeps
//! pointing at (0, 0, 0) without any manual angle maths.
//!
//! Setup: attach to anything with a Transform.

use bevy::prelude::*;
use renzora::ScriptCtx;

fn update(ctx: &mut ScriptCtx) {
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.look_at(Vec3::ZERO, Vec3::Y);
    }
}

renzora::script!(update);
