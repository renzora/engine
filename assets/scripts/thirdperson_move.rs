//! Camera-relative third-person movement. Raw WASD feels wrong in 3D, so the
//! input is turned into the active camera's frame: forward always means "away
//! from the camera", the standard third-person expectation.
//!
//! The Lua version rotated by `camera_yaw`, which nothing ever filled in, so it
//! moved in world axes. This reads the active 3D camera's orientation instead,
//! and falls back to world axes (forward is -Z) when there is no camera.
//!
//! Setup: attach to the character; works with an orbit or follow camera.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptInput};

/// Units per second.
const SPEED: f32 = 6.0;

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let axis = ctx
        .get_resource::<ScriptInput>()
        .map(|input| input.get_movement_vector())
        .unwrap_or(Vec2::ZERO);
    if axis == Vec2::ZERO {
        return;
    }
    let (right, forward) = camera_basis(ctx.world());
    let step = (right * axis.x + forward * axis.y) * SPEED * dt;
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.translation += step;
}

/// The active 3D camera's right and forward directions, flattened onto the XZ
/// plane, so looking down at the character does not slow it.
fn camera_basis(world: &mut World) -> (Vec3, Vec3) {
    let mut cameras = world.query_filtered::<(&Camera, &GlobalTransform), With<Camera3d>>();
    let Some((_, camera)) = cameras.iter(world).find(|(c, _)| c.is_active) else {
        return (Vec3::X, Vec3::NEG_Z);
    };
    let forward = Vec3::new(camera.forward().x, 0.0, camera.forward().z).normalize_or_zero();
    if forward == Vec3::ZERO {
        return (Vec3::X, Vec3::NEG_Z);
    }
    // Right of a flat forward, so it stays horizontal even if the camera rolls.
    (forward.cross(Vec3::Y), forward)
}

renzora::script!(update);
