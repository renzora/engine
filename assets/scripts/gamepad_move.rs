//! Moves with the left analog stick, with a deadzone. Sticks rest slightly off
//! centre, so small magnitudes are ignored to stop drift; past the deadzone,
//! speed scales with how far the stick is pushed.
//!
//! Setup: attach to the entity to move and connect a gamepad.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptInput};

/// Units per second at full deflection.
const SPEED: f32 = 6.0;
/// Stick magnitudes below this are ignored.
const DEADZONE: f32 = 0.15;

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(input) = ctx.get_resource::<ScriptInput>() else { return };
    let Some(pad) = input.first_gamepad() else { return };
    let stick = input.get_gamepad_left_stick(pad);
    if stick.length() < DEADZONE {
        return;
    }
    // Stick up (positive Y) moves forward, which is -Z.
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.translation += Vec3::new(stick.x, 0.0, -stick.y) * SPEED * dt;
}

renzora::script!(update);
