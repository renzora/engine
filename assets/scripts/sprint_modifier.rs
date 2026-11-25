//! WASD movement with a Shift sprint modifier. Sprint checks the held state,
//! not the just-pressed edge, so it stays on for as long as Shift is down.
//!
//! Setup: attach to the entity to move; hold left Shift to sprint.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptInput};

const WALK_SPEED: f32 = 4.0;
const SPRINT_MULT: f32 = 2.0;

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(input) = ctx.get_resource::<ScriptInput>() else { return };
    let axis = input.get_movement_vector();
    if axis == Vec2::ZERO {
        return;
    }
    let speed = if input.is_key_pressed(KeyCode::ShiftLeft) { WALK_SPEED * SPRINT_MULT } else { WALK_SPEED };
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.translation += Vec3::new(axis.x, 0.0, -axis.y) * speed * dt;
}

renzora::script!(update);
