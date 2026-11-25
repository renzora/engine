//! A moving platform that rises, pauses at the top, falls back to its rest
//! position, pauses at the bottom, and repeats. Smoothstep easing on the
//! travel segments keeps starts and stops gentle so a rider is not launched.
//!
//! Setup: attach to the platform root. Its rest position is captured on the
//! first frame.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};

/// How far up the platform travels, in units.
const TRAVEL_HEIGHT: f32 = 3.0;
/// Seconds for each of the rise and the fall.
const TRAVEL_TIME: f32 = 1.5;
const DWELL_TOP: f32 = 0.75;
const DWELL_BOTTOM: f32 = 0.75;
/// Seconds before the first rise.
const START_DELAY: f32 = 0.0;

#[derive(Component)]
struct Platform {
    t: f32,
    base: Vec3,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(mut platform) = ctx.get_mut::<Platform>() else { return };
    platform.t += dt;
    let (t, base) = (platform.t, platform.base);
    drop(platform);

    let cycle = TRAVEL_TIME * 2.0 + DWELL_TOP + DWELL_BOTTOM;
    let phase = t.max(0.0) % cycle;
    let k = if phase < TRAVEL_TIME {
        phase / TRAVEL_TIME
    } else if phase < TRAVEL_TIME + DWELL_TOP {
        1.0
    } else if phase < TRAVEL_TIME * 2.0 + DWELL_TOP {
        1.0 - (phase - TRAVEL_TIME - DWELL_TOP) / TRAVEL_TIME
    } else {
        0.0
    };
    let s = k * k * (3.0 - 2.0 * k);
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation = base + Vec3::Y * (s * TRAVEL_HEIGHT);
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if matches!(hook, ScriptHook::Ready) {
        let base = ctx.get::<Transform>().map_or(Vec3::ZERO, |t| t.translation);
        ctx.insert(Platform { t: -START_DELAY, base });
    }
}

renzora::script!(update, hooks = hooks);
