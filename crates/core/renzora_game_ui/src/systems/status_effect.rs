//! Ticks status effect timers, removing expired effects.

use bevy::prelude::*;

use crate::components::StatusEffectBarData;

/// Advances elapsed time on each status effect and removes expired ones.
///
/// Effects with `duration <= 0.0` are treated as permanent and never expire.
pub fn status_effect_system(time: Res<Time>, mut bars: Query<&mut StatusEffectBarData>) {
    let dt = time.delta_secs();
    for mut data in &mut bars {
        data.effects.retain_mut(|effect| {
            if effect.duration <= 0.0 {
                return true; // permanent
            }
            effect.elapsed += dt;
            effect.elapsed < effect.duration
        });
    }
}
