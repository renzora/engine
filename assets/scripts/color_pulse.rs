//! Pulses the material brightness between two greys. The sine is remapped
//! from -1..1 to 0..1 before lerping the channel value; otherwise half the
//! cycle would clamp to black.
//!
//! Setup: attach to an entity with a mesh and material.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::ScriptCtx;
use std::f32::consts::TAU;

/// Darkest grey.
const LOW: f32 = 0.1;
/// Brightest grey.
const HIGH: f32 = 0.9;
/// Cycles per second.
const FREQUENCY: f32 = 1.0;

fn update(ctx: &mut ScriptCtx) {
    let t = (ctx.elapsed() * FREQUENCY * TAU).sin() * 0.5 + 0.5;
    let v = LOW.lerp(HIGH, t);
    let me = ctx.entity();
    ScriptAction::new("set_material_color", me)
        .arg("r", v)
        .arg("g", v)
        .arg("b", v)
        .arg("a", 1.0)
        .send(ctx.world());
}

renzora::script!(update);
