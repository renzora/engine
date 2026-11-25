//! Gentle single-axis sway, like grass or a hanging sign. Only the Z rotation
//! is animated, about the angle it had when play started, giving a calm
//! pendulum feel; kept low-amplitude and low-frequency on purpose.
//!
//! Setup: attach to the swaying entity.

use bevy::prelude::*;
use renzora::ScriptCtx;

/// Sway amplitude in degrees either side of the starting angle.
const AMPLITUDE: f32 = 6.0;
/// Sways per second.
const FREQUENCY: f32 = 0.6;

#[derive(Component)]
struct Sway {
    /// The Z rotation in degrees when the script started.
    base_z: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let elapsed = ctx.elapsed();
    let Some(rotation) = ctx.get::<Transform>().map(|t| t.rotation) else { return };
    let (y, x, z) = rotation.to_euler(EulerRot::YXZ);
    if !ctx.has::<Sway>() {
        ctx.insert(Sway { base_z: z.to_degrees() });
    }
    let Some(base_z) = ctx.get::<Sway>().map(|s| s.base_z) else { return };

    let rz = base_z + (elapsed * FREQUENCY * std::f32::consts::TAU).sin() * AMPLITUDE;
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.rotation = Quat::from_euler(EulerRot::YXZ, y, x, rz.to_radians());
}

renzora::script!(update);
