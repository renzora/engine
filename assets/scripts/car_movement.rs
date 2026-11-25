//! Arcade-style car controller with momentum and steering. W/S accelerate and
//! reverse, A/D steer, Space is the handbrake. Steering scales with speed and
//! flips in reverse, so the car cannot turn on the spot.
//!
//! Setup: attach to the car. It drives the Transform directly, no physics.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

const MAX_SPEED: f32 = 20.0;
const MAX_REVERSE: f32 = 8.0;
/// Units per second squared.
const ACCELERATION: f32 = 12.0;
/// Deceleration when S is held while moving forward.
const BRAKE_FORCE: f32 = 20.0;
/// Passive deceleration while coasting.
const FRICTION: f32 = 3.0;
/// Maximum turn rate, degrees per second.
const TURN_SPEED: f32 = 90.0;
const HANDBRAKE: f32 = 35.0;

#[derive(Component)]
struct Car {
    /// Signed speed along the heading.
    velocity: f32,
    /// Heading, in degrees.
    yaw: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let (throttle, steer, handbrake) = match ctx.get_resource::<ScriptInput>() {
        Some(input) => {
            let axis = |pos: [KeyCode; 2], neg: [KeyCode; 2]| {
                let p = pos.iter().any(|k| input.is_key_pressed(*k)) as i32;
                let n = neg.iter().any(|k| input.is_key_pressed(*k)) as i32;
                (p - n) as f32
            };
            (
                axis([KeyCode::KeyW, KeyCode::ArrowUp], [KeyCode::KeyS, KeyCode::ArrowDown]),
                axis([KeyCode::KeyA, KeyCode::ArrowLeft], [KeyCode::KeyD, KeyCode::ArrowRight]),
                input.is_key_pressed(KeyCode::Space),
            )
        }
        None => (0.0, 0.0, false),
    };

    let Some(mut car) = ctx.get_mut::<Car>() else { return };
    let mut v = car.velocity;
    if throttle > 0.0 {
        v += ACCELERATION * dt;
    } else if throttle < 0.0 {
        v -= if v > 0.0 { BRAKE_FORCE } else { ACCELERATION } * dt;
    } else {
        v = toward_zero(v, FRICTION * dt);
    }
    if handbrake {
        v = toward_zero(v, HANDBRAKE * dt);
    }
    v = v.clamp(-MAX_REVERSE, MAX_SPEED);

    let speed_ratio = (v.abs() / MAX_SPEED).min(1.0);
    let direction = if v < 0.0 { -1.0 } else { 1.0 };
    car.yaw += steer * TURN_SPEED * speed_ratio * direction * dt;
    car.velocity = v;
    let yaw = car.yaw.to_radians();
    drop(car);

    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.rotation = Quat::from_rotation_y(yaw);
    transform.translation += Vec3::new(yaw.sin(), 0.0, yaw.cos()) * v * dt;
}

/// Bleed `v` toward zero by `amount` without overshooting.
fn toward_zero(v: f32, amount: f32) -> f32 {
    if v > 0.0 {
        (v - amount).max(0.0)
    } else {
        (v + amount).min(0.0)
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let yaw = ctx
            .get::<Transform>()
            .map(|t| t.rotation.to_euler(EulerRot::YXZ).0.to_degrees())
            .unwrap_or(0.0);
        ctx.insert(Car { velocity: 0.0, yaw });
    }
}

renzora::script!(update, hooks = hooks);
