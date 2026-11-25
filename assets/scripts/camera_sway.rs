//! Gentle camera sway: a slow, non-repeating drift for an idle or cinematic
//! camera. It reads as a boat deck at low intensity and a handheld shot at
//! high. Written for looking at the ocean, where a perfectly still camera is
//! the thing that gives the water away.
//!
//! Every channel is the sum of TWO sine waves whose frequencies share no common
//! multiple. One sine is a metronome: the eye locks onto the period within a
//! few seconds. Two incommensurate ones take hours to line up again, which is
//! what makes the motion read as drift rather than animation.
//!
//! FOV is part of it twice over. The lens breathes as a seventh channel, and
//! the rotation amplitudes are scaled by it: a 0.8 degree roll is a shrug at 90
//! degrees of FOV and a lurch at 25, so holding the *angle* constant makes the
//! sway feel different on every lens. Scaling by FOV holds the on-screen motion
//! constant instead, which is what "gentle" actually means.
//!
//! The script OWNS this entity's transform and FOV: it writes absolute
//! position, rotation and field of view every frame, relative to the pose
//! captured when it became ready. Do not attach it to a camera something else
//! also drives (an FPS controller, a follow rig, an aim-down-sights zoom); it
//! will fight them.
//!
//! Setup: attach to a camera entity.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};
use std::f32::consts::TAU;

/// Master multiplier over every amplitude below. 0 is perfectly still.
const INTENSITY: f32 = 1.0;
/// Time scale. Below 1 reads as heavier, calmer water.
const SPEED: f32 = 1.0;
/// Rise and fall, metres.
const HEAVE: f32 = 0.10;
/// Fore/aft drift, metres.
const SURGE: f32 = 0.06;
/// Left/right drift, metres.
const DRIFT: f32 = 0.08;
/// Nose up/down, degrees.
const PITCH_ANGLE: f32 = 0.45;
/// Horizon tilt, degrees.
const ROLL_ANGLE: f32 = 0.80;
/// Heading drift, degrees.
const YAW_ANGLE: f32 = 0.30;
/// Lens breathing, degrees of FOV. 0 leaves the lens alone.
const FOV_BREATH: f32 = 0.40;
/// Scale the rotation amplitudes by FOV so the on-screen sway looks the same on
/// any lens.
const FOV_COMPENSATE: bool = true;
/// The lens the amplitudes were tuned against. Anything narrower sways
/// proportionally less in degrees to cover the same arc of screen.
const REFERENCE_FOV: f32 = 60.0;

/// The rest pose, captured on ready. Sway is an offset from here, so the camera
/// always returns to where you placed it.
#[derive(Component)]
struct Sway {
    t: f32,
    base: Vec3,
    /// Rest pitch, yaw and roll, in degrees.
    base_pitch: f32,
    base_yaw: f32,
    base_roll: f32,
    /// Rest FOV in degrees, or `None` for an orthographic camera.
    base_fov: Option<f32>,
}

/// Two sines at incommensurate frequencies, weighted 2:1 so the slower one
/// carries the motion and the faster one keeps it from ever repeating.
fn wave(t: f32, f1: f32, f2: f32, phase: f32) -> f32 {
    (t * f1 * TAU + phase).sin() * 0.66 + (t * f2 * TAU + phase * 1.7).sin() * 0.34
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(mut sway) = ctx.get_mut::<Sway>() else { return };
    sway.t += dt * SPEED;
    let t = sway.t;
    let (base, base_pitch, base_yaw, base_roll, base_fov) =
        (sway.base, sway.base_pitch, sway.base_yaw, sway.base_roll, sway.base_fov);
    drop(sway);

    // Frequencies in Hz. Ocean swell heaves at roughly 0.1 to 0.2 Hz, so the
    // position channels sit there. The rotation channels run a little faster:
    // a hull tips as it meets a wave face slightly before it rises, and
    // borrowing that ordering is most of what sells the motion.
    let offset_up = wave(t, 0.11, 0.17, 0.0) * HEAVE;
    let offset_right = wave(t, 0.09, 0.15, 1.3) * DRIFT;
    let offset_forward = wave(t, 0.07, 0.13, 2.7) * SURGE;

    // 0 FOV (orthographic) falls back to the reference lens, keeping the
    // angular scaling at 1x rather than dividing by zero.
    let lens = if FOV_COMPENSATE { base_fov.unwrap_or(REFERENCE_FOV) / REFERENCE_FOV } else { 1.0 };
    let pitch = wave(t, 0.13, 0.21, 0.6) * PITCH_ANGLE * lens;
    let roll = wave(t, 0.08, 0.19, 2.1) * ROLL_ANGLE * lens;
    let yaw = wave(t, 0.05, 0.11, 4.2) * YAW_ANGLE * lens;

    // The lens breathes on its own slow pair, unrelated to the rest, so a zoom
    // never lines up with a pitch and reads as one deliberate move.
    let breath = wave(t, 0.06, 0.09, 3.4) * FOV_BREATH;

    // Drift along the camera's own axes rather than the world's, so "forward"
    // means into the view whichever way the camera was left pointing. Yaw is
    // the rest yaw: the sway's own is too small to matter and would couple the
    // channels. Bevy cameras look down local -Z.
    let (sin_yaw, cos_yaw) = base_yaw.to_radians().sin_cos();
    let world_x = offset_right * cos_yaw - offset_forward * sin_yaw;
    let world_z = -offset_right * sin_yaw - offset_forward * cos_yaw;

    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation = base + Vec3::new(world_x, offset_up, world_z) * INTENSITY;
        transform.rotation = Quat::from_euler(
            EulerRot::YXZ,
            (base_yaw + yaw * INTENSITY).to_radians(),
            (base_pitch + pitch * INTENSITY).to_radians(),
            (base_roll + roll * INTENSITY).to_radians(),
        );
    }
    if FOV_BREATH != 0.0 {
        if let (Some(base_fov), Some(mut projection)) = (base_fov, ctx.get_mut::<Projection>()) {
            if let Projection::Perspective(perspective) = &mut *projection {
                perspective.fov = (base_fov + breath * INTENSITY).to_radians();
            }
        }
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let Some(transform) = ctx.get::<Transform>() else { return };
        let base = transform.translation;
        let (yaw, pitch, roll) = transform.rotation.to_euler(EulerRot::YXZ);
        let base_fov = match ctx.get::<Projection>().as_deref() {
            Some(Projection::Perspective(p)) if p.fov > 0.0 => Some(p.fov.to_degrees()),
            _ => None,
        };
        ctx.insert(Sway {
            t: 0.0,
            base,
            base_pitch: pitch.to_degrees(),
            base_yaw: yaw.to_degrees(),
            base_roll: roll.to_degrees(),
            base_fov,
        });
    }
}

renzora::script!(update, hooks = hooks);
