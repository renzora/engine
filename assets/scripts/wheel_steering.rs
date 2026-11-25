//! Turns the front wheels from the left gamepad stick's X axis, eased toward
//! the target angle so they do not snap each frame. A deadzone keeps a
//! controller at rest from drifting them off centre.
//!
//! Setup: attach to the car root. The wheels must be DIRECT children named
//! "wheel_frontleft" and "wheel_frontright"; deeper descendants are not
//! searched. If the import left names like `wheel_frontleft.Mesh`, change
//! `FRONT_LEFT` / `FRONT_RIGHT`.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptInput};

const FRONT_LEFT: &str = "wheel_frontleft";
const FRONT_RIGHT: &str = "wheel_frontright";
/// Wheel turn at full stick deflection, in degrees.
const MAX_ANGLE_DEG: f32 = 30.0;
/// Stick magnitude under this counts as centred.
const DEADZONE: f32 = 0.10;
/// Higher eases to the target angle faster.
const SMOOTHING: f32 = 12.0;
/// Flip if the wheels turn the wrong way.
const INVERT: bool = false;

/// The current smoothed angle, in degrees.
#[derive(Component, Default)]
struct Steer(f32);

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Steer>() {
        ctx.insert(Steer::default());
    }
    let mut x = ctx
        .get_resource::<ScriptInput>()
        .and_then(|input| input.first_gamepad().map(|pad| input.get_gamepad_left_stick(pad).x))
        .unwrap_or(0.0);
    if x.abs() < DEADZONE {
        x = 0.0;
    }
    let sign = if INVERT { -1.0 } else { 1.0 };
    let target = x * MAX_ANGLE_DEG * sign;

    let Some(mut steer) = ctx.get_mut::<Steer>() else { return };
    steer.0 += (target - steer.0) * (1.0 - (-SMOOTHING * dt).exp());
    let rotation = Quat::from_rotation_y(steer.0.to_radians());
    drop(steer);

    for child in ctx.children() {
        let is_wheel = ctx
            .get_on::<Name>(child)
            .is_some_and(|n| n.as_str() == FRONT_LEFT || n.as_str() == FRONT_RIGHT);
        if !is_wheel {
            continue;
        }
        if let Some(mut transform) = ctx.get_mut_on::<Transform>(child) {
            transform.rotation = rotation;
        }
    }
}

renzora::script!(update);
