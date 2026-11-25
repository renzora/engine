//! Physics-driven car controller. W/S push the body along its heading with a
//! force, A/D turn the heading, and Space raises the linear damping to act as a
//! handbrake (bleeding speed while gravity still pulls).
//!
//! Setup: attach to an entity with a dynamic rigid body (`PhysicsBodyData`) and
//! a box or capsule collider. Recommended body: mass 1.0, linear damping 0.3,
//! angular damping 1.0, rotation locked on X and Z so it cannot flip.

use bevy::prelude::*;
use renzora::core::reflection::set_reflected_field;
use renzora::script_bridge::{PropertyValue, ScriptAction};
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

const ENGINE_FORCE: f32 = 25.0;
/// Reverse thrust while S is held.
const BRAKE_FORCE: f32 = 40.0;
/// Steering rate, degrees per second.
const TURN_SPEED: f32 = 120.0;
/// Linear damping while the handbrake is held.
const HANDBRAKE_DAMP: f32 = 6.0;
/// Linear damping otherwise.
const NORMAL_DAMP: f32 = 0.3;

#[derive(Component)]
struct Car {
    /// Heading, in degrees.
    yaw: f32,
    /// The handbrake state last written, so the damping is set on change.
    braking: Option<bool>,
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
    car.yaw += steer * TURN_SPEED * dt;
    let yaw = car.yaw.to_radians();
    let damping_changed = car.braking != Some(handbrake);
    car.braking = Some(handbrake);
    drop(car);

    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.rotation = Quat::from_rotation_y(yaw);
    }

    let forward = Vec3::new(yaw.sin(), 0.0, yaw.cos());
    let force = if throttle > 0.0 {
        forward * ENGINE_FORCE
    } else if throttle < 0.0 {
        -forward * BRAKE_FORCE
    } else {
        Vec3::ZERO
    };
    let me = ctx.entity();
    if force != Vec3::ZERO {
        ScriptAction::new("apply_force", me)
            .arg("x", force.x)
            .arg("y", force.y)
            .arg("z", force.z)
            .send(ctx.world());
    }

    if damping_changed {
        let damping = if handbrake { HANDBRAKE_DAMP } else { NORMAL_DAMP };
        set_reflected_field(ctx.world(), me, "PhysicsBodyData", "linear_damping", &PropertyValue::Float(damping));
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let yaw = ctx
            .get::<Transform>()
            .map(|t| t.rotation.to_euler(EulerRot::YXZ).0.to_degrees())
            .unwrap_or(0.0);
        ctx.insert(Car { yaw, braking: None });
    }
}

renzora::script!(update, hooks = hooks);
