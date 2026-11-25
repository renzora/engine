//! Yaw-relative WASD movement, with Space and Ctrl for up and down and Shift
//! to sprint. "Forward" follows the entity's own yaw, so it pairs with a
//! mouse-look script that turns the same entity.
//!
//! Setup: attach to the same entity as `fps_controller.rs`.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptInput};

const MOVE_SPEED: f32 = 10.0;
const SPRINT_MULTIPLIER: f32 = 2.0;

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(input) = ctx.get_resource::<ScriptInput>() else { return };
    let held = |a: KeyCode, b: KeyCode| input.is_key_pressed(a) || input.is_key_pressed(b);
    let axis = |pos: bool, neg: bool| pos as i32 as f32 - neg as i32 as f32;
    let forward_in = axis(held(KeyCode::KeyW, KeyCode::ArrowUp), held(KeyCode::KeyS, KeyCode::ArrowDown));
    let right_in = axis(held(KeyCode::KeyD, KeyCode::ArrowRight), held(KeyCode::KeyA, KeyCode::ArrowLeft));
    let up_in = axis(input.is_key_pressed(KeyCode::Space), input.is_key_pressed(KeyCode::ControlLeft));
    let sprint = input.is_key_pressed(KeyCode::ShiftLeft);
    let speed = if sprint { MOVE_SPEED * SPRINT_MULTIPLIER } else { MOVE_SPEED };

    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
    let forward = Vec2::new(-yaw.sin(), -yaw.cos());
    let right = Vec2::new(yaw.cos(), -yaw.sin());
    // Normalized so diagonals are no faster.
    let planar = (forward * forward_in + right * right_in).normalize_or_zero();
    transform.translation += Vec3::new(planar.x, up_in, planar.y) * speed * dt;
}

renzora::script!(update);
