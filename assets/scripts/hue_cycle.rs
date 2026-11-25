//! Cycles the material through the full hue wheel. Rather than a real
//! HSV-to-RGB conversion, one sine is phase-shifted by 120 degrees per
//! channel, which is the cheapest way to sweep hue without a lookup table.
//!
//! Setup: attach to an entity with a material.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::ScriptCtx;
use std::f32::consts::TAU;

/// Hue revolutions per second.
const SPEED: f32 = 0.2;

fn update(ctx: &mut ScriptCtx) {
    let a = ctx.elapsed() * SPEED * TAU;
    let third = TAU / 3.0;
    let me = ctx.entity();
    ScriptAction::new("set_material_color", me)
        .arg("r", channel(a))
        .arg("g", channel(a - third))
        .arg("b", channel(a - third * 2.0))
        .arg("a", 1.0)
        .send(ctx.world());
}

/// A sine moved into 0..1; the three channels peak a third of a turn apart.
fn channel(phase: f32) -> f32 {
    (phase.sin() * 0.5 + 0.5).clamp(0.0, 1.0)
}

renzora::script!(update);
