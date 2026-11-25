//! Toggles visibility on a fixed interval. The on/off state is derived from
//! elapsed time with a modulo instead of a timer, so the blink stays in
//! lockstep with wall-clock time even after a hitch.
//!
//! Setup: attach to anything visible.

use bevy::prelude::*;
use renzora::ScriptCtx;

/// Seconds per on (or off) half of the cycle.
const PERIOD: f32 = 0.5;

fn update(ctx: &mut ScriptCtx) {
    let shown = ctx.elapsed() % (PERIOD * 2.0) < PERIOD;
    let want = if shown { Visibility::Inherited } else { Visibility::Hidden };
    match ctx.get_mut::<Visibility>() {
        // Written only on a flip so change detection does not fire every frame.
        Some(mut visibility) => {
            if *visibility != want {
                *visibility = want;
            }
        }
        None => ctx.insert(want),
    }
}

renzora::script!(update);
