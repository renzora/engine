//! An arcade car controller that also drives a chase camera. W/S accelerate
//! and reverse, A/D steer, Space is the handbrake. The camera trails behind the
//! heading, eases toward its spot with frame-rate independent smoothing, and
//! aims a little above the car's origin.
//!
//! Setup: attach to the car; the camera entity is named "camera". The car mesh
//! faces -Z.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
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

/// The camera entity this script drives.
const CAMERA: &str = "camera";
/// Trail distance behind the car.
const CAM_DISTANCE: f32 = 8.0;
/// Height above the car.
const CAM_HEIGHT: f32 = 3.5;
/// Aim point height above the car's origin.
const LOOK_HEIGHT: f32 = 1.0;
/// Position smoothing; higher is snappier.
const FOLLOW_SMOOTH: f32 = 6.0;
/// Flip if the camera ends up on the wrong side of the car.
const INVERT_FORWARD: bool = true;
/// Extra yaw applied to the heading for camera placement, in degrees.
const YAW_OFFSET: f32 = 0.0;

#[derive(Component)]
struct Car {
    /// Signed speed along the heading.
    velocity: f32,
    /// Heading, in degrees.
    yaw: f32,
    /// Smoothed camera position; `None` until the first frame places it, so it
    /// does not sweep in from the origin.
    cam: Option<Vec3>,
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
    let cam = car.cam;
    drop(car);

    // The mesh faces -Z, so forward is the negated heading vector.
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.rotation = Quat::from_rotation_y(yaw);
    transform.translation += Vec3::new(-yaw.sin(), 0.0, -yaw.cos()) * v * dt;
    let car_pos = transform.translation;
    drop(transform);

    let cam_yaw = yaw + YAW_OFFSET.to_radians();
    let sign = if INVERT_FORWARD { -1.0 } else { 1.0 };
    let cam_forward = Vec3::new(cam_yaw.sin(), 0.0, cam_yaw.cos()) * sign;
    let desired = car_pos - cam_forward * CAM_DISTANCE + Vec3::Y * CAM_HEIGHT;

    let t = 1.0 - (-FOLLOW_SMOOTH * dt).exp();
    let cam = cam.unwrap_or(desired).lerp(desired, t);
    if let Some(mut car) = ctx.get_mut::<Car>() {
        car.cam = Some(cam);
    }

    // Yaw then pitch, no roll: the camera stays level however the car moves.
    let to_target = car_pos + Vec3::Y * LOOK_HEIGHT - cam;
    let look_yaw = (-to_target.x).atan2(-to_target.z);
    let look_pitch = to_target.y.atan2(Vec2::new(to_target.x, to_target.z).length());

    let Some(camera) = find_named(ctx.world(), CAMERA) else { return };
    if let Some(mut cam_transform) = ctx.get_mut_on::<Transform>(camera) {
        cam_transform.translation = cam;
        cam_transform.rotation = Quat::from_euler(EulerRot::YXZ, look_yaw, look_pitch, 0.0);
    }
}

/// Bleed `v` toward zero by `amount` without overshooting.
fn toward_zero(v: f32, amount: f32) -> f32 {
    if v > 0.0 {
        (v - amount).max(0.0)
    } else {
        (v + amount).min(0.0)
    }
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let yaw = ctx
            .get::<Transform>()
            .map(|t| t.rotation.to_euler(EulerRot::YXZ).0.to_degrees())
            .unwrap_or(0.0);
        ctx.insert(Car { velocity: 0.0, yaw, cam: None });
        let me = ctx.entity();
        ScriptAction::new("lock_cursor", me).send(ctx.world());
    }
}

renzora::script!(update, hooks = hooks);
