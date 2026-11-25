//! Vertical bobbing around a remembered base height. The start Y is captured
//! on the first frame so the bob oscillates about the entity's authored
//! position instead of drifting away from the origin.
//!
//! Setup: attach to anything with a Transform.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};
use std::f32::consts::TAU;

/// Bob height.
const AMPLITUDE: f32 = 0.5;
/// Cycles per second.
const FREQUENCY: f32 = 1.0;

/// The authored height the bob is centred on.
#[derive(Component)]
struct BobBase {
    y: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let Some(base) = ctx.get::<BobBase>().map(|b| b.y) else { return };
    let offset = (ctx.elapsed() * FREQUENCY * TAU).sin() * AMPLITUDE;
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation.y = base + offset;
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let y = ctx.get::<Transform>().map_or(0.0, |t| t.translation.y);
        ctx.insert(BobBase { y });
    }
}

renzora::script!(update, hooks = hooks);
