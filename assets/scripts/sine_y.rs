//! Pure sine-wave vertical motion about the start height. Deliberately the
//! most minimal oscillator in the set, kept as a clean reference for the
//! `sin(elapsed) * amplitude` pattern around a captured base.
//!
//! Setup: attach to anything with a Transform.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};

/// Wave height.
const AMPLITUDE: f32 = 1.0;

/// The authored height the wave is centred on.
#[derive(Component)]
struct SineBase {
    y: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let Some(base) = ctx.get::<SineBase>().map(|b| b.y) else { return };
    let y = base + ctx.elapsed().sin() * AMPLITUDE;
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation.y = y;
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let y = ctx.get::<Transform>().map_or(0.0, |t| t.translation.y);
        ctx.insert(SineBase { y });
    }
}

renzora::script!(update, hooks = hooks);
