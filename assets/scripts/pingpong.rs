//! Moves back and forth along X between two extremes. A triangle wave (the
//! asin of a sine) gives constant-speed travel with sharp turnarounds, unlike
//! a plain sine, which eases at the ends.
//!
//! Setup: attach to anything with a Transform.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};
use std::f32::consts::{FRAC_PI_2, TAU};

/// Half the travel distance.
const DISTANCE: f32 = 2.0;
/// Cycles per second.
const SPEED: f32 = 1.0;

/// The authored X the motion is centred on.
#[derive(Component)]
struct PingPongBase {
    x: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let Some(base) = ctx.get::<PingPongBase>().map(|b| b.x) else { return };
    let tri = (ctx.elapsed() * SPEED * TAU).sin().asin() / FRAC_PI_2;
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation.x = base + tri * DISTANCE;
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let x = ctx.get::<Transform>().map_or(0.0, |t| t.translation.x);
        ctx.insert(PingPongBase { x });
    }
}

renzora::script!(update, hooks = hooks);
