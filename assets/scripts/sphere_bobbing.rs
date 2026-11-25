//! A gentle vertical bob with a slow yaw drift, for floating spheres. The
//! starting world position seeds the phase, so a field of spheres bobs out of
//! step as a natural group with no per-entity setup.
//!
//! Setup: attach to each sphere.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};
use std::f32::consts::TAU;

/// Vertical amplitude, in units.
const BOB_HEIGHT: f32 = 0.08;
/// Bob frequency, roughly in Hz.
const BOB_SPEED: f32 = 0.6;
/// Slow yaw spin in degrees per second; 0 disables it.
const DRIFT_SPIN: f32 = 6.0;

#[derive(Component)]
struct Bob {
    t: f32,
    base_y: f32,
    phase: f32,
    /// Degrees.
    yaw: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(mut bob) = ctx.get_mut::<Bob>() else { return };
    bob.t += dt;
    bob.yaw += DRIFT_SPIN * dt;
    let (t, base_y, phase, yaw) = (bob.t, bob.base_y, bob.phase, bob.yaw);
    drop(bob);

    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.translation.y = base_y + (t * BOB_SPEED * TAU + phase).sin() * BOB_HEIGHT;
    if DRIFT_SPIN != 0.0 {
        transform.rotation = Quat::from_rotation_y(yaw.to_radians());
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if !matches!(hook, ScriptHook::Ready) {
        return;
    }
    let Some(transform) = ctx.get::<Transform>() else { return };
    let p = transform.translation;
    let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
    let bob = Bob {
        t: 0.0,
        base_y: p.y,
        phase: (p.x * 0.37 + p.z * 0.91).rem_euclid(TAU),
        yaw: yaw.to_degrees(),
    };
    ctx.insert(bob);
}

renzora::script!(update, hooks = hooks);
