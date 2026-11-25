//! Top-down movement from a 2D input vector. Keyboard (WASD or arrows) and the
//! left gamepad stick feed the same vector, so one code path serves both, and
//! scaling by delta keeps the speed frame-rate independent.
//!
//! Setup: attach to the entity to move. X strafes, screen up is -Z.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptInput};

/// Units per second.
const SPEED: f32 = 5.0;
/// Stick magnitudes below this are rest noise, not input.
const STICK_DEADZONE: f32 = 0.15;

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(input) = ctx.get_resource::<ScriptInput>() else { return };
    let mut axis = input.get_movement_vector();
    if axis == Vec2::ZERO {
        if let Some(pad) = input.first_gamepad() {
            let stick = input.get_gamepad_left_stick(pad);
            if stick.length() >= STICK_DEADZONE {
                axis = stick.clamp_length_max(1.0);
            }
        }
    }
    if axis == Vec2::ZERO {
        return;
    }
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.translation += Vec3::new(axis.x, 0.0, -axis.y) * SPEED * dt;
}

renzora::script!(update);
