//! Gentle floating motion: a vertical bob plus pitch and roll. Three sine waves
//! at different frequencies, so the motion never looks perfectly periodic, and
//! pitch is phase-shifted so it does not peak with the roll.
//!
//! Setup: attach to the boat root. It offsets from the pose the entity had when
//! the script became ready.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};
use std::f32::consts::TAU;

/// Vertical bob amplitude, in units.
const BOB_HEIGHT: f32 = 0.04;
/// Vertical bob frequency, in Hz.
const BOB_SPEED: f32 = 0.5;
/// Side-to-side roll amplitude, in degrees.
const ROLL_ANGLE: f32 = 1.2;
const ROLL_SPEED: f32 = 0.35;
/// Fore/aft pitch amplitude, in degrees.
const PITCH_ANGLE: f32 = 0.7;
const PITCH_SPEED: f32 = 0.55;

/// The rest pose and the running clock.
#[derive(Component)]
struct Rocking {
    t: f32,
    base: Vec3,
    /// Rest yaw, in radians.
    base_yaw: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(mut rocking) = ctx.get_mut::<Rocking>() else { return };
    rocking.t += dt;
    let (t, base, base_yaw) = (rocking.t, rocking.base, rocking.base_yaw);
    drop(rocking);

    let bob = (t * BOB_SPEED * TAU).sin() * BOB_HEIGHT;
    let roll = (t * ROLL_SPEED * TAU).sin() * ROLL_ANGLE;
    let pitch = (t * PITCH_SPEED * TAU + 1.3).sin() * PITCH_ANGLE;

    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.translation = base + Vec3::Y * bob;
    transform.rotation = Quat::from_euler(EulerRot::YXZ, base_yaw, pitch.to_radians(), roll.to_radians());
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let Some(transform) = ctx.get::<Transform>() else { return };
        let base = transform.translation;
        let (base_yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        ctx.insert(Rocking { t: 0.0, base, base_yaw });
    }
}

renzora::script!(update, hooks = hooks);
