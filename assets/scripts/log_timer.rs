//! A repeating timer that logs a heartbeat line each time it fires. The timer
//! is a countdown on the entity; it is topped up by the interval rather than
//! reset to it, so a long frame does not make the cadence drift.
//!
//! Setup: attach to any entity; its name appears in the log line.

use bevy::prelude::*;
use renzora::ScriptCtx;

/// Seconds between log lines.
const INTERVAL: f32 = 2.0;

#[derive(Component)]
struct Heartbeat {
    remaining: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Heartbeat>() {
        ctx.insert(Heartbeat { remaining: INTERVAL });
    }
    let Some(mut timer) = ctx.get_mut::<Heartbeat>() else { return };
    timer.remaining -= dt;
    if timer.remaining > 0.0 {
        return;
    }
    timer.remaining += INTERVAL;
    drop(timer);

    let name = ctx.name().unwrap_or("unnamed").to_string();
    renzora::clog_info!("Script", "heartbeat from {name}");
}

renzora::script!(update);
