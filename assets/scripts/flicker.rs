//! Random on/off flicker, like a failing light or fire. Visibility is rerolled
//! each frame against a probability, so the pattern is irregular rather than
//! the even cadence of a timed blink.
//!
//! Setup: attach to any visible entity.

use bevy::prelude::*;
use renzora::ScriptCtx;

/// Probability the entity is visible on any given frame.
const ON_CHANCE: f32 = 0.8;

fn update(ctx: &mut ScriptCtx) {
    let roll = random01(ctx.elapsed(), ctx.entity());
    let wanted = if roll < ON_CHANCE { Visibility::Inherited } else { Visibility::Hidden };
    if let Some(mut visibility) = ctx.get_mut::<Visibility>() {
        // Only write on a change, so a steady state does not trip change detection.
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// A cheap 0..1 pseudo-random value from the frame time and the entity, so two
/// flickering lights in one scene do not flicker in lockstep.
fn random01(elapsed: f32, entity: Entity) -> f32 {
    let mut x = (elapsed.to_bits() as u64) ^ entity.to_bits().wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    x = x.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    x ^= x >> 33;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

renzora::script!(update);
