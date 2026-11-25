//! Camera juice on landing. The grounded mirror is watched for an airborne to
//! grounded edge, and the screen shake is scaled by how fast the body was
//! falling (tracked from the velocity mirror). Only the edge triggers, so
//! moving about on the ground stays calm.
//!
//! Setup: attach to a character with physics (it gets a `PhysicsReadState`);
//! the animator has a "Land" trigger.

use bevy::prelude::*;
use renzora::core::reflection::get_reflected_field;
use renzora::script_bridge::{PropertyValue, ScriptAction};
use renzora::ScriptCtx;

/// Falls slower than this make no shake.
const MIN_SPEED: f32 = 3.0;
/// The shake saturates at this fall speed.
const MAX_SPEED: f32 = 18.0;
const MAX_INTENSITY: f32 = 0.6;

#[derive(Component)]
struct Landing {
    was_grounded: bool,
    /// The fastest downward speed seen while airborne.
    fall_speed: f32,
}

fn update(ctx: &mut ScriptCtx) {
    if !ctx.has::<Landing>() {
        ctx.insert(Landing { was_grounded: true, fall_speed: 0.0 });
    }
    let me = ctx.entity();
    // A missing mirror reads as grounded, so nothing fires without physics.
    let grounded = !matches!(
        get_reflected_field(ctx.world(), me, "PhysicsReadState", "grounded"),
        Some(PropertyValue::Bool(false))
    );
    let vy = match get_reflected_field(ctx.world(), me, "PhysicsReadState", "velocity.y") {
        Some(PropertyValue::Float(v)) => v,
        _ => 0.0,
    };

    let Some(mut landing) = ctx.get_mut::<Landing>() else { return };
    if !grounded && vy < 0.0 {
        landing.fall_speed = landing.fall_speed.max(-vy);
    }
    let impact = (grounded && !landing.was_grounded).then_some(landing.fall_speed);
    if impact.is_some() {
        landing.fall_speed = 0.0;
    }
    landing.was_grounded = grounded;
    drop(landing);

    let Some(speed) = impact.filter(|s| *s > MIN_SPEED) else { return };
    let f = ((speed - MIN_SPEED) / (MAX_SPEED - MIN_SPEED)).clamp(0.0, 1.0);
    let world = ctx.world();
    ScriptAction::new("screen_shake", me)
        .arg("intensity", MAX_INTENSITY * f)
        .arg("duration", 0.25 + 0.2 * f)
        .send(world);
    ScriptAction::new("play_sound", me)
        .arg("path", "land_thud")
        .arg("volume", 1.0)
        .arg("looping", false)
        .arg("bus", "Sfx")
        .send(world);
    ScriptAction::new("trigger_anim", me).arg("name", "Land").send(world);
}

renzora::script!(update);
