//! An interactable door. When the player is within `OPEN_DISTANCE` a UI prompt
//! appears; pressing E (or the gamepad's R1) toggles the door, which swings
//! `OPEN_ANGLE` degrees about its Y axis at a steady rate.
//!
//! Pivot: a door swings about its hinge, so the door mesh's ORIGIN must sit at
//! the hinge edge, not the centre. If it spins about the middle, re-pivot the
//! mesh, or parent it to an empty placed at the hinge and put this script on
//! that parent.
//!
//! Setup: attach to the door. The player entity is named "player"; a UI text
//! named "DoorPrompt" is the prompt. Set `PROMPT_NAME` to "" to skip the UI and
//! use proximity and the key alone.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

const PLAYER_NAME: &str = "player";
/// The UI widget shown as a prompt; "" for none.
const PROMPT_NAME: &str = "DoorPrompt";
const OPEN_TEXT: &str = "Open Door  [E]";
const CLOSE_TEXT: &str = "Close Door [E]";
/// How close the player must be, in world units (horizontal).
const OPEN_DISTANCE: f32 = 2.5;
/// Swing angle, degrees.
const OPEN_ANGLE: f32 = 90.0;
/// Swing speed, degrees per second.
const OPEN_SPEED: f32 = 220.0;
const INTERACT_KEY: KeyCode = KeyCode::KeyE;
/// The gamepad interact button: `RightTrigger` is R1.
const GAMEPAD_BUTTON: GamepadButton = GamepadButton::RightTrigger;

#[derive(Component)]
struct Door {
    /// The closed rotation, as YXZ Euler angles in degrees.
    base_pitch: f32,
    base_yaw: f32,
    base_roll: f32,
    /// Current yaw, degrees.
    yaw: f32,
    open: bool,
    was_near: bool,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Door>() {
        capture(ctx);
    }
    let Some(my_pos) = ctx.get::<Transform>().map(|t| t.translation) else { return };

    let near = match find_named(ctx.world(), PLAYER_NAME).and_then(|p| ctx.get_on::<Transform>(p).map(|t| t.translation)) {
        Some(player) => Vec2::new(player.x - my_pos.x, player.z - my_pos.z).length_squared() < OPEN_DISTANCE * OPEN_DISTANCE,
        None => false,
    };

    let interact = match ctx.get_resource::<ScriptInput>() {
        Some(input) => {
            input.is_key_just_pressed(INTERACT_KEY)
                || input.first_gamepad().is_some_and(|pad| input.is_gamepad_button_just_pressed(pad, GAMEPAD_BUTTON))
        }
        None => false,
    };

    let Some(mut door) = ctx.get_mut::<Door>() else { return };
    let was_near = door.was_near;
    door.was_near = near;
    if near && interact {
        door.open = !door.open;
    }
    let open = door.open;

    let target = door.base_yaw + if open { OPEN_ANGLE } else { 0.0 };
    let diff = target - door.yaw;
    let step = OPEN_SPEED * dt;
    door.yaw = if diff.abs() <= step { target } else { door.yaw + step * diff.signum() };
    let rotation = Quat::from_euler(
        EulerRot::YXZ,
        door.yaw.to_radians(),
        door.base_pitch.to_radians(),
        door.base_roll.to_radians(),
    );
    drop(door);

    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.rotation = rotation;
    }

    if PROMPT_NAME.is_empty() {
        return;
    }
    if near && !was_near {
        prompt(ctx, "ui_show", None);
    }
    if near && (!was_near || interact) {
        prompt(ctx, "ui_set_text", Some(if open { CLOSE_TEXT } else { OPEN_TEXT }));
    } else if !near && was_near {
        prompt(ctx, "ui_hide", None);
    }
}

/// Record the closed pose. `update` also calls it when ready found no
/// Transform, so a door that gains one later still gets its state.
fn capture(ctx: &mut ScriptCtx) {
    let Some(transform) = ctx.get::<Transform>() else { return };
    let (yaw, pitch, roll) = transform.rotation.to_euler(EulerRot::YXZ);
    let (yaw, pitch, roll) = (yaw.to_degrees(), pitch.to_degrees(), roll.to_degrees());
    ctx.insert(Door { base_pitch: pitch, base_yaw: yaw, base_roll: roll, yaw, open: false, was_near: false });
}

fn prompt(ctx: &mut ScriptCtx, action: &str, text: Option<&str>) {
    let me = ctx.entity();
    let mut action = ScriptAction::new(action, me).arg("name", PROMPT_NAME);
    if let Some(text) = text {
        action = action.arg("text", text);
    }
    action.send(ctx.world());
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        capture(ctx);
        if !PROMPT_NAME.is_empty() {
            prompt(ctx, "ui_hide", None);
        }
    }
}

renzora::script!(update, hooks = hooks);
