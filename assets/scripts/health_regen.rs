//! Regenerates health over time, capped at the maximum. Regen accumulates
//! with delta, so the rate is exact at any frame rate.
//!
//! The engine has no `Health` component, so this script keeps the health
//! value itself, in its own `Health` component on the entity; other Rust
//! scripts can read or damage it from there.
//!
//! Setup: attach to anything that should heal.

use bevy::prelude::*;
use renzora::ScriptCtx;

const START_HEALTH: f32 = 100.0;
const MAX_HEALTH: f32 = 100.0;
/// Health points per second.
const REGEN_PER_SEC: f32 = 5.0;

#[derive(Component)]
struct Health {
    current: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Health>() {
        ctx.insert(Health { current: START_HEALTH });
    }
    let Some(mut health) = ctx.get_mut::<Health>() else { return };
    if health.current >= MAX_HEALTH {
        return;
    }
    health.current = (health.current + REGEN_PER_SEC * dt).clamp(0.0, MAX_HEALTH);
}

renzora::script!(update);
