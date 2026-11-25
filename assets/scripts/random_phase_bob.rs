//! Vertical bob with a per-instance random phase. The phase is seeded once on
//! the first frame, so many copies of this script bob out of sync, breaking
//! the "everything moves as one" look without per-entity tuning.
//!
//! Setup: attach to anything with a Transform.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};
use std::f32::consts::TAU;

/// Bob height.
const AMPLITUDE: f32 = 0.4;
/// Cycles per second.
const FREQUENCY: f32 = 1.0;

#[derive(Component)]
struct RandomBob {
    base_y: f32,
    /// Radians added to the wave, fixed for this instance.
    phase: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let Some((base_y, phase)) = ctx.get::<RandomBob>().map(|b| (b.base_y, b.phase)) else {
        return;
    };
    let y = (ctx.elapsed() * FREQUENCY * TAU + phase).sin() * AMPLITUDE;
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation.y = base_y + y;
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let base_y = ctx.get::<Transform>().map_or(0.0, |t| t.translation.y);
        // Seeded from the entity id so each instance is distinct, plus the
        // start time so a re-spawned entity does not repeat its old phase.
        let seed = ctx.entity().to_bits() ^ ((ctx.elapsed() * 1000.0) as u64);
        let phase = random01(seed) * TAU;
        ctx.insert(RandomBob { base_y, phase });
    }
}

/// A uniform value in 0..1 from a seed (splitmix64 finaliser).
fn random01(seed: u64) -> f32 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

renzora::script!(update, hooks = hooks);
