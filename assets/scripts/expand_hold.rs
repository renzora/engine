//! Grows from zero to full size once, then holds. A one-shot entrance with no
//! looping: progress marches to 1 and stays, so this is a spawn "pop-in"
//! rather than a repeating pulse.
//!
//! Setup: attach to any entity that should pop in when play starts.

use bevy::prelude::*;
use renzora::ScriptCtx;

/// Final uniform scale.
const TARGET_SCALE: f32 = 1.0;
/// Progress per second; 2.0 reaches full size in half a second.
const GROW_SPEED: f32 = 2.0;

#[derive(Component)]
struct Expand {
    progress: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Expand>() {
        ctx.insert(Expand { progress: 0.0 });
        if let Some(mut transform) = ctx.get_mut::<Transform>() {
            transform.scale = Vec3::ZERO;
        }
        return;
    }
    let Some(mut state) = ctx.get_mut::<Expand>() else { return };
    if state.progress >= 1.0 {
        return;
    }
    state.progress = (state.progress + GROW_SPEED * dt).clamp(0.0, 1.0);
    let progress = state.progress;
    drop(state);

    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.scale = Vec3::splat(TARGET_SCALE * progress);
    }
}

renzora::script!(update);
