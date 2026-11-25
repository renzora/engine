//! Jumps by applying an upward impulse when Space is pressed. An impulse adds
//! velocity instantly, which is the right physics primitive for a jump; a
//! force would ramp up too slowly.
//!
//! Setup: the entity needs a dynamic rigid body and a collider.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptInput};

/// Upward impulse.
const JUMP_FORCE: f32 = 6.0;

fn update(ctx: &mut ScriptCtx) {
    let jump = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(KeyCode::Space));
    if !jump {
        return;
    }
    let me = ctx.entity();
    ScriptAction::new("apply_impulse", me)
        .arg("x", 0.0)
        .arg("y", JUMP_FORCE)
        .arg("z", 0.0)
        .send(ctx.world());
}

renzora::script!(update);
