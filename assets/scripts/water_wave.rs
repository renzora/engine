//! A slow swell for a water surface: a subtle bob and a faint tilt of the
//! whole plane. It moves the transform, not the vertices, so it reads as
//! swell rather than ripples; pair it with a scrolling normal map in the
//! material for surface detail.
//!
//! Setup: attach to the water plane. Its rest position and yaw are captured on
//! the first frame.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};
use std::f32::consts::TAU;

/// Vertical amplitude, in units.
const SWELL_HEIGHT: f32 = 0.03;
/// Swell frequency, roughly in Hz.
const SWELL_SPEED: f32 = 0.25;
/// Tilt amplitude, in degrees.
const TILT_ANGLE: f32 = 0.4;
const TILT_SPEED: f32 = 0.18;

#[derive(Component)]
struct Swell {
    t: f32,
    base: Vec3,
    /// Radians.
    base_yaw: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(mut swell) = ctx.get_mut::<Swell>() else { return };
    swell.t += dt;
    let (t, base, base_yaw) = (swell.t, swell.base, swell.base_yaw);
    drop(swell);

    let rise = (t * SWELL_SPEED * TAU).sin() * SWELL_HEIGHT;
    let tilt_x = ((t * TILT_SPEED * TAU).sin() * TILT_ANGLE).to_radians();
    let tilt_z = ((t * TILT_SPEED * TAU + 1.7).sin() * TILT_ANGLE).to_radians();
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.translation = base + Vec3::Y * rise;
    transform.rotation = Quat::from_euler(EulerRot::YXZ, base_yaw, tilt_x, tilt_z);
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if !matches!(hook, ScriptHook::Ready) {
        return;
    }
    let Some(transform) = ctx.get::<Transform>() else { return };
    let (base_yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
    let swell = Swell { t: 0.0, base: transform.translation, base_yaw };
    ctx.insert(swell);
}

renzora::script!(update, hooks = hooks);
