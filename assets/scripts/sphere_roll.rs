//! Camera-relative arrow-key rolling with momentum. Velocity approaches the
//! input direction exponentially while a key is held and decays to rest when
//! released, so direction changes arc instead of snapping. The sphere spins
//! by distance over radius, so it rolls without slipping.
//!
//! Setup: attach to the sphere. The camera that defines "forward" is named
//! "camera"; without it, forward is -Z.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptInput};

const CAMERA: &str = "camera";
/// Match the mesh for no-slip rolling. The engine's primitive sphere is 0.5;
/// multiply by its Y scale if scaled.
const RADIUS: f32 = 0.5;
/// Units per second.
const MAX_SPEED: f32 = 6.0;
/// How fast velocity catches up with input, per second.
const ACCEL_RATE: f32 = 8.0;
/// How fast velocity decays with no input, per second; lower rolls farther.
const FRICTION_RATE: f32 = 2.5;

#[derive(Component, Default)]
struct Roll {
    /// XZ velocity, as (x, z).
    velocity: Vec2,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Roll>() {
        ctx.insert(Roll::default());
    }
    let Some(input) = ctx.get_resource::<ScriptInput>() else { return };
    let axis = |pos: KeyCode, neg: KeyCode| {
        input.is_key_pressed(pos) as i32 as f32 - input.is_key_pressed(neg) as i32 as f32
    };
    let raw = Vec2::new(
        axis(KeyCode::ArrowRight, KeyCode::ArrowLeft),
        axis(KeyCode::ArrowUp, KeyCode::ArrowDown),
    );
    let held = raw != Vec2::ZERO;
    let dir_in = raw.normalize_or_zero();

    let yaw = find_named(ctx.world(), CAMERA)
        .and_then(|cam| ctx.get_on::<Transform>(cam).map(|t| t.rotation.to_euler(EulerRot::YXZ).0))
        .unwrap_or(0.0);

    // Camera forward (-Z) and right (+X), flattened onto the ground.
    let forward = Vec2::new(-yaw.sin(), -yaw.cos());
    let right = Vec2::new(yaw.cos(), -yaw.sin());
    let dir = right * dir_in.x + forward * dir_in.y;

    // Different rates for push and coast: quick to respond, slow to stop.
    let rate = if held { ACCEL_RATE } else { FRICTION_RATE };
    let k = 1.0 - (-rate * dt).exp();
    let Some(mut roll) = ctx.get_mut::<Roll>() else { return };
    let velocity = roll.velocity;
    roll.velocity = velocity + (dir * MAX_SPEED - velocity) * k;
    let step = roll.velocity * dt;
    drop(roll);
    if step == Vec2::ZERO {
        return;
    }

    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.translation += Vec3::new(step.x, 0.0, step.y);
    // omega = v / R: +Z motion pitches about +X, +X motion rolls about -Z.
    if RADIUS > 0.0 {
        transform.rotation *= Quat::from_euler(EulerRot::YXZ, 0.0, step.y / RADIUS, -step.x / RADIUS);
    }
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

renzora::script!(update);
