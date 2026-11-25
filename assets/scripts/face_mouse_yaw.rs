//! Turns smoothly to face a yaw driven by horizontal mouse movement. The mouse
//! moves a target yaw, and the current yaw eases toward it so turning eases in
//! instead of snapping; the difference is wrapped so crossing +/-180 degrees
//! takes the short way round.
//!
//! Setup: attach to the entity that should turn; move the mouse left and right.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptInput};

/// Degrees per mouse unit.
const SENSITIVITY: f32 = 0.2;
/// Higher is a snappier turn.
const SMOOTHING: f32 = 8.0;

#[derive(Component)]
struct Facing {
    current: f32,
    target: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Facing>() {
        let Some(rotation) = ctx.get::<Transform>().map(|t| t.rotation) else { return };
        let yaw = rotation.to_euler(EulerRot::YXZ).0.to_degrees();
        ctx.insert(Facing { current: yaw, target: yaw });
    }
    let mouse_x = ctx
        .get_resource::<ScriptInput>()
        .map(|input| input.mouse_delta.x)
        .unwrap_or(0.0);

    let Some(mut facing) = ctx.get_mut::<Facing>() else { return };
    facing.target -= mouse_x * SENSITIVITY;
    let diff = (facing.target - facing.current + 180.0).rem_euclid(360.0) - 180.0;
    facing.current += diff * (SMOOTHING * dt).clamp(0.0, 1.0);
    let yaw = facing.current;
    drop(facing);

    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.rotation = Quat::from_rotation_y(yaw.to_radians());
}

renzora::script!(update);
