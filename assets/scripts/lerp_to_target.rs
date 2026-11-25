//! Eases from the spawn point to a target point, then stops. A normalised t
//! advances toward 1 and is clamped; once it reaches 1 the lerp holds the
//! destination exactly, so there is no jitter from overshooting.
//!
//! Setup: attach to anything with a Transform; set `TARGET`.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};

/// Destination in world units.
const TARGET: Vec3 = Vec3::new(0.0, 2.0, 0.0);
/// Fraction of the journey covered per second.
const SPEED: f32 = 0.5;

#[derive(Component)]
struct LerpToTarget {
    start: Vec3,
    t: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(mut state) = ctx.get_mut::<LerpToTarget>() else { return };
    if state.t >= 1.0 {
        return;
    }
    state.t = (state.t + SPEED * dt).clamp(0.0, 1.0);
    let position = state.start.lerp(TARGET, state.t);
    drop(state);
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation = position;
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let start = ctx.get::<Transform>().map_or(Vec3::ZERO, |t| t.translation);
        ctx.insert(LerpToTarget { start, t: 0.0 });
    }
}

renzora::script!(update, hooks = hooks);
