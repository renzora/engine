//! The simplest correct jump: it is only allowed when the physics mirror says
//! the body is actually grounded. Reading `PhysicsReadState.grounded` rather
//! than guessing from Y avoids double jumps off slopes and mid-air re-jumps.
//!
//! Setup: attach to a dynamic rigid body (anything with physics gets a
//! `PhysicsReadState`); Space jumps.

use bevy::prelude::*;
use renzora::core::reflection::get_reflected_field;
use renzora::script_bridge::{PropertyValue, ScriptAction};
use renzora::{ScriptCtx, ScriptInput};

const JUMP_IMPULSE: f32 = 7.0;

#[derive(Component, Default)]
struct JumpGate {
    was_grounded: bool,
}

fn update(ctx: &mut ScriptCtx) {
    if !ctx.has::<JumpGate>() {
        ctx.insert(JumpGate::default());
    }
    let me = ctx.entity();
    let grounded = matches!(
        get_reflected_field(ctx.world(), me, "PhysicsReadState", "grounded"),
        Some(PropertyValue::Bool(true))
    );
    let jump = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(KeyCode::Space));

    // Gate: a fresh press and feet on the ground.
    if grounded && jump {
        ScriptAction::new("apply_impulse", me)
            .arg("x", 0.0)
            .arg("y", JUMP_IMPULSE)
            .arg("z", 0.0)
            .send(ctx.world());
        ScriptAction::new("play_sound", me)
            .arg("path", "jump")
            .arg("volume", 1.0)
            .arg("looping", false)
            .arg("bus", "Sfx")
            .send(ctx.world());
    }

    let Some(mut gate) = ctx.get_mut::<JumpGate>() else { return };
    let landed = grounded && !gate.was_grounded;
    gate.was_grounded = grounded;
    drop(gate);
    // Logged on the landing edge only, not every grounded frame.
    if landed {
        renzora::clog_info!("Script", "landed");
    }
}

renzora::script!(update);
