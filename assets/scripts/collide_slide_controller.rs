//! A camera-relative character mover. The kinematic slide does the actual
//! collide-and-slide against geometry; this script turns input and the camera
//! heading into a desired displacement, and applies gravity so the body hugs
//! the floor.
//!
//! The Lua version rotated by `camera_yaw`, which nothing filled in, so it
//! moved in world axes (and W went toward +Z). This reads the active 3D
//! camera's heading instead, and falls back to world axes (W is -Z) without one.
//!
//! Setup: attach to a kinematic character body.

use bevy::prelude::*;
use renzora::core::reflection::get_reflected_field;
use renzora::script_bridge::{PropertyValue, ScriptAction};
use renzora::{ScriptCtx, ScriptInput};

const MOVE_SPEED: f32 = 5.0;
const GRAVITY: f32 = -18.0;
/// Downward speed kept while grounded, so slopes stay seated.
const GROUND_STICK: f32 = -2.0;

#[derive(Component)]
struct Mover {
    vertical_velocity: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let me = ctx.entity();
    if !ctx.has::<Mover>() {
        ctx.insert(Mover { vertical_velocity: 0.0 });
    }
    let axis = ctx
        .get_resource::<ScriptInput>()
        .map(|input| input.get_movement_vector())
        .unwrap_or(Vec2::ZERO);
    let (right, forward) = camera_basis(ctx.world());
    let wish = right * axis.x + forward * axis.y;
    let grounded = matches!(
        get_reflected_field(ctx.world(), me, "PhysicsReadState", "grounded"),
        Some(PropertyValue::Bool(true))
    );

    let Some(mut mover) = ctx.get_mut::<Mover>() else { return };
    if grounded && mover.vertical_velocity < 0.0 {
        mover.vertical_velocity = GROUND_STICK;
    } else {
        mover.vertical_velocity += GRAVITY * dt;
    }
    let vy = mover.vertical_velocity;
    drop(mover);

    let step = wish * MOVE_SPEED * dt + Vec3::Y * vy * dt;
    ScriptAction::new("kinematic_slide", me)
        .arg("x", step.x)
        .arg("y", step.y)
        .arg("z", step.z)
        .send(ctx.world());

    // Face the travel direction only while actually moving.
    if wish.x.abs() + wish.z.abs() > 0.01 {
        if let Some(mut transform) = ctx.get_mut::<Transform>() {
            let target = transform.translation + wish;
            transform.look_at(target, Vec3::Y);
        }
    }
}

/// The active 3D camera's right and forward directions, flattened onto the XZ
/// plane, so a camera looking down does not slow the character.
fn camera_basis(world: &mut World) -> (Vec3, Vec3) {
    let mut cameras = world.query_filtered::<(&Camera, &GlobalTransform), With<Camera3d>>();
    let Some((_, camera)) = cameras.iter(world).find(|(c, _)| c.is_active) else {
        return (Vec3::X, Vec3::NEG_Z);
    };
    let forward = Vec3::new(camera.forward().x, 0.0, camera.forward().z).normalize_or_zero();
    if forward == Vec3::ZERO {
        return (Vec3::X, Vec3::NEG_Z);
    }
    (forward.cross(Vec3::Y), forward)
}

renzora::script!(update);
