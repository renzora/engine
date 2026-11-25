//! A sprint with a stamina budget. Holding Shift (or the left trigger) while
//! moving drains stamina and boosts speed; releasing, or emptying the bar,
//! regenerates it after a short delay. Stamina is mirrored to a HUD label
//! whenever its whole-percent value changes.
//!
//! Setup: attach to a mover. A UI text named "Stamina" shows the percentage,
//! and an entity named "stamina_fill" is the bar: its X scale is the fraction.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptInput};

const MAX_STAMINA: f32 = 100.0;
const DRAIN_PER_SEC: f32 = 35.0;
const REGEN_PER_SEC: f32 = 20.0;
/// Seconds after sprinting stops before stamina starts coming back.
const REGEN_DELAY: f32 = 1.0;
const WALK_SPEED: f32 = 4.0;
const SPRINT_MULT: f32 = 1.8;

#[derive(Component)]
struct Stamina {
    value: f32,
    regen_cooldown: f32,
    /// The percentage last written to the label, so it is written on change.
    shown: i32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Stamina>() {
        ctx.insert(Stamina { value: MAX_STAMINA, regen_cooldown: 0.0, shown: -1 });
    }

    let (movement, shift, trigger) = match ctx.get_resource::<ScriptInput>() {
        Some(input) => (
            input.get_movement_vector(),
            input.is_key_pressed(KeyCode::ShiftLeft),
            input.first_gamepad().map(|pad| input.get_gamepad_trigger(pad, true)).unwrap_or(0.0),
        ),
        None => (Vec2::ZERO, false, 0.0),
    };
    let moving = movement.length() > 0.05;

    let Some(mut stamina) = ctx.get_mut::<Stamina>() else { return };
    let sprinting = (shift || trigger > 0.5) && moving && stamina.value > 0.0;
    if sprinting {
        stamina.value = (stamina.value - DRAIN_PER_SEC * dt).max(0.0);
        stamina.regen_cooldown = REGEN_DELAY;
    } else if stamina.regen_cooldown > 0.0 {
        stamina.regen_cooldown -= dt;
    } else {
        stamina.value = (stamina.value + REGEN_PER_SEC * dt).min(MAX_STAMINA);
    }
    let fraction = stamina.value / MAX_STAMINA;
    let percent = (fraction * 100.0).round() as i32;
    let label_changed = percent != stamina.shown;
    stamina.shown = percent;
    drop(stamina);

    let me = ctx.entity();
    ScriptAction::new("set_anim_bool", me)
        .arg("name", "Sprinting")
        .arg("value", sprinting)
        .send(ctx.world());

    // Move on the XZ plane: W is forward (-Z).
    let speed = if sprinting { WALK_SPEED * SPRINT_MULT } else { WALK_SPEED };
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation += Vec3::new(movement.x, 0.0, -movement.y) * speed * dt;
    }

    if let Some(bar) = find_named(ctx.world(), "stamina_fill") {
        if let Some(mut transform) = ctx.get_mut_on::<Transform>(bar) {
            transform.scale.x = fraction;
        }
    }
    if label_changed {
        ScriptAction::new("ui_set_text", me)
            .arg("name", "Stamina")
            .arg("text", format!("Stamina {percent}%"))
            .send(ctx.world());
    }
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

renzora::script!(update);
