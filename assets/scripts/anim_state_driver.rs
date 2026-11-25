//! Decouples the animation graph from gameplay code. Instead of playing clips
//! directly, it feeds the animator its parameters (a Speed float, Grounded and
//! Moving bools, Jump and Land triggers) and lets the state machine pick.
//!
//! Setup: attach to a character whose animation state machine has those
//! parameters. Grounded comes from its physics body; without one it is
//! treated as always grounded.

use bevy::prelude::*;
use renzora::core::reflection::get_reflected_field;
use renzora::script_bridge::{PropertyValue, ScriptAction};
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

/// Speed parameter at full input.
const MOVE_SPEED: f32 = 4.0;
/// How quickly Speed follows the input, per second.
const SPEED_SMOOTHING: f32 = 10.0;

#[derive(Component)]
struct AnimDriver {
    prev_grounded: bool,
    smoothed_speed: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let me = ctx.entity();
    if !ctx.has::<AnimDriver>() {
        ctx.insert(AnimDriver { prev_grounded: true, smoothed_speed: 0.0 });
    }
    let (mag, jump) = match ctx.get_resource::<ScriptInput>() {
        Some(input) => (
            input.get_movement_vector().length().min(1.0),
            input.is_key_just_pressed(KeyCode::Space),
        ),
        None => (0.0, false),
    };
    let grounded = grounded(ctx.world(), me).unwrap_or(true);

    let Some(mut driver) = ctx.get_mut::<AnimDriver>() else { return };
    // Smoothed so the blend tree does not pop on twitchy input.
    let target = mag * MOVE_SPEED;
    driver.smoothed_speed += (target - driver.smoothed_speed) * (dt * SPEED_SMOOTHING).clamp(0.0, 1.0);
    let speed = driver.smoothed_speed;
    let landed = grounded && !driver.prev_grounded;
    driver.prev_grounded = grounded;
    drop(driver);

    let world = ctx.world();
    ScriptAction::new("set_anim_param", me).arg("name", "Speed").arg("value", speed).send(world);
    ScriptAction::new("set_anim_bool", me).arg("name", "Moving").arg("value", mag > 0.05).send(world);
    ScriptAction::new("set_anim_bool", me).arg("name", "Grounded").arg("value", grounded).send(world);
    if grounded && jump {
        ScriptAction::new("trigger_anim", me).arg("name", "Jump").send(world);
    }
    if landed {
        ScriptAction::new("trigger_anim", me).arg("name", "Land").send(world);
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let me = ctx.entity();
        let world = ctx.world();
        ScriptAction::new("set_anim_bool", me).arg("name", "Grounded").arg("value", true).send(world);
        ScriptAction::new("set_anim_param", me).arg("name", "Speed").arg("value", 0.0).send(world);
    }
}

/// Whether the physics body is on the ground, read from the engine's
/// `PhysicsReadState` mirror; `None` when the entity has no body.
fn grounded(world: &World, entity: Entity) -> Option<bool> {
    match get_reflected_field(world, entity, "PhysicsReadState", "grounded") {
        Some(PropertyValue::Bool(grounded)) => Some(grounded),
        _ => None,
    }
}

renzora::script!(update, hooks = hooks);
