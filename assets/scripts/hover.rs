//! Holds the entity at a fixed hover offset above its start. It eases up to
//! the offset once on spawn (no oscillation) and settles at a raised resting
//! height, which suits floating pickups or UI props.
//!
//! Setup: attach to anything with a Transform.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};

/// Hover height above the start.
const OFFSET: f32 = 1.0;
/// Fraction of the rise covered per second.
const RISE_SPEED: f32 = 2.0;

#[derive(Component)]
struct Hover {
    base_y: f32,
    /// 0 at the start height, 1 at the full offset.
    progress: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(mut hover) = ctx.get_mut::<Hover>() else { return };
    hover.progress = (hover.progress + RISE_SPEED * dt).clamp(0.0, 1.0);
    let y = hover.base_y + OFFSET * hover.progress;
    drop(hover);
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation.y = y;
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let base_y = ctx.get::<Transform>().map_or(0.0, |t| t.translation.y);
        ctx.insert(Hover { base_y, progress: 0.0 });
    }
}

renzora::script!(update, hooks = hooks);
