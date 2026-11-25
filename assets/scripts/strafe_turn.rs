//! Tank controls: forward and back drive along the body's facing, left and
//! right turn it. The body turns first, then drives along the new facing.
//!
//! The Lua version drove along +Z at yaw 0, which is backwards for a Bevy
//! entity (its forward is -Z); this drives along the entity's real forward.
//!
//! Setup: attach to the vehicle or character; WASD or the arrow keys.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptInput};

/// Forward units per second.
const MOVE_SPEED: f32 = 5.0;
/// Degrees per second.
const TURN_SPEED: f32 = 120.0;

#[derive(Component)]
struct Tank {
    /// Heading in degrees.
    yaw: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Tank>() {
        let Some(rotation) = ctx.get::<Transform>().map(|t| t.rotation) else { return };
        let (y, _, _) = rotation.to_euler(EulerRot::YXZ);
        ctx.insert(Tank { yaw: y.to_degrees() });
    }
    let axis = ctx
        .get_resource::<ScriptInput>()
        .map(|input| input.get_movement_vector())
        .unwrap_or(Vec2::ZERO);

    let Some(mut tank) = ctx.get_mut::<Tank>() else { return };
    tank.yaw -= axis.x * TURN_SPEED * dt;
    let yaw = tank.yaw;
    drop(tank);

    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.rotation = Quat::from_rotation_y(yaw.to_radians());
    if axis.y != 0.0 {
        let forward = transform.forward();
        transform.translation += forward * axis.y * MOVE_SPEED * dt;
    }
}

renzora::script!(update);
