//! Pulses material alpha to fade in and out. RGB stays white and only the
//! alpha channel animates, so the entity ghosts in and out without shifting
//! hue.
//!
//! Setup: attach to an entity whose material can blend (an alpha mode other
//! than Opaque).

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::ScriptCtx;
use std::f32::consts::TAU;

/// Most transparent.
const MIN_ALPHA: f32 = 0.0;
/// Most opaque.
const MAX_ALPHA: f32 = 1.0;
/// Cycles per second.
const FREQUENCY: f32 = 0.5;

fn update(ctx: &mut ScriptCtx) {
    let t = (ctx.elapsed() * FREQUENCY * TAU).sin() * 0.5 + 0.5;
    let me = ctx.entity();
    ScriptAction::new("set_material_color", me)
        .arg("r", 1.0)
        .arg("g", 1.0)
        .arg("b", 1.0)
        .arg("a", MIN_ALPHA.lerp(MAX_ALPHA, t))
        .send(ctx.world());
}

renzora::script!(update);
