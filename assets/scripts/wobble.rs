//! Tilts on X and Z with two slightly detuned sines. Different frequencies
//! per axis keep the wobble from collapsing into a single flat plane of
//! motion, giving an organic, never-quite-repeating sway. Yaw is left alone,
//! so another script may turn the entity while it wobbles.
//!
//! Setup: attach to anything with a Transform.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};

/// Maximum tilt in degrees.
const AMPLITUDE: f32 = 8.0;

/// The authored tilt (radians) the wobble is centred on.
#[derive(Component)]
struct WobbleBase {
    x: f32,
    z: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let Some((base_x, base_z)) = ctx.get::<WobbleBase>().map(|b| (b.x, b.z)) else { return };
    let elapsed = ctx.elapsed();
    let rx = base_x + ((elapsed * 1.3).sin() * AMPLITUDE).to_radians();
    let rz = base_z + ((elapsed * 1.7).sin() * AMPLITUDE).to_radians();
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, rx, rz);
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let (_, x, z) = ctx
            .get::<Transform>()
            .map_or((0.0, 0.0, 0.0), |t| t.rotation.to_euler(EulerRot::YXZ));
        ctx.insert(WobbleBase { x, z });
    }
}

renzora::script!(update, hooks = hooks);
