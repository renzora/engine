//! Twin-stick control: the left stick moves on the XZ plane and the right
//! stick aims the body independently of travel. The body is only re-aimed when
//! the right stick is past the deadzone, so letting go keeps the last facing
//! instead of snapping back to zero. Holding the right trigger while aiming
//! fires bolts named "ts_bolt".
//!
//! Stick up is screen forward (-Z) and the body's forward (-Z) faces the aim.
//! The Lua original mapped stick up to +Z, which ran toward the camera.
//!
//! Setup: attach to a top-down character; a connected gamepad drives it.

use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::{ScriptCtx, ScriptInput};

const MOVE_SPEED: f32 = 6.0;
const DEADZONE: f32 = 0.25;
/// Seconds between bolts while the trigger is held.
const FIRE_COOLDOWN: f32 = 0.15;
const FIRE_PREFIX: &str = "ts_bolt";

#[derive(Component, Default)]
struct TwinStick {
    cooldown: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<TwinStick>() {
        ctx.insert(TwinStick::default());
    }
    let Some((left, right, trigger)) = ctx.get_resource::<ScriptInput>().and_then(|input| {
        let pad = input.first_gamepad()?;
        Some((
            input.get_gamepad_left_stick(pad),
            input.get_gamepad_right_stick(pad),
            input.get_gamepad_trigger(pad, false),
        ))
    }) else {
        return;
    };

    let Some(mut state) = ctx.get_mut::<TwinStick>() else { return };
    state.cooldown = (state.cooldown - dt).max(0.0);
    let aiming = right.length() > DEADZONE;
    let fire = aiming && trigger > 0.5 && state.cooldown <= 0.0;
    if fire {
        state.cooldown = FIRE_COOLDOWN;
    }
    drop(state);

    let mx = if left.x.abs() < DEADZONE { 0.0 } else { left.x };
    let my = if left.y.abs() < DEADZONE { 0.0 } else { left.y };
    let aim = Vec3::new(right.x, 0.0, -right.y).normalize_or_zero();
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.translation += Vec3::new(mx, 0.0, -my) * MOVE_SPEED * dt;
    if aiming {
        transform.rotation = Quat::from_rotation_y((-aim.x).atan2(-aim.z));
    }
    let muzzle = transform.translation + aim * 0.8;
    drop(transform);

    if fire {
        ctx.world().spawn((
            Name::new(FIRE_PREFIX),
            Transform::from_translation(muzzle),
            Visibility::default(),
            MeshPrimitive("sphere".into()),
            MeshColor(Color::srgb(1.0, 0.9, 0.3)),
        ));
    }
}

renzora::script!(update);
