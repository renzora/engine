//! A live counter UI templates can bind with `{{ EntityName.score }}`. It
//! accumulates fractional points with the frame delta and only adds whole
//! ones, so the score climbs at the same rate on any frame rate.
//!
//! A Rust script has no variables of its own, so the score is published into
//! this script's entry of the entity's `ScriptComponent`, which is what the
//! markup binding reads.
//!
//! Setup: name the entity (for example "scoreboard"), then in a template:
//! `<text>Score: {{ scoreboard.score }}</text>`.

use bevy::prelude::*;
use renzora::component::ScriptComponent;
use renzora::{ScriptCtx, ScriptValue};

const SCRIPT_FILE: &str = "example_counter.rs";
/// Points added per second.
const PER_SECOND: i32 = 10;

#[derive(Component, Default)]
struct Counter {
    score: i32,
    accumulator: f32,
    /// Whether the score has been published yet, so the binding shows 0 from
    /// the first frame rather than nothing.
    published: bool,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Counter>() {
        ctx.insert(Counter::default());
    }
    let Some(mut counter) = ctx.get_mut::<Counter>() else { return };
    counter.accumulator += PER_SECOND as f32 * dt;
    let add = counter.accumulator.floor();
    let changed = add >= 1.0 || !counter.published;
    if add >= 1.0 {
        counter.score += add as i32;
        counter.accumulator -= add;
    }
    counter.published = true;
    let score = counter.score;
    drop(counter);

    // Written on change only: the live link compares this component's text.
    if !changed {
        return;
    }
    let Some(mut scripts) = ctx.get_mut::<ScriptComponent>() else { return };
    for entry in scripts.scripts.iter_mut() {
        if entry.script_path.as_ref().is_some_and(|p| p.ends_with(SCRIPT_FILE)) {
            entry.variables.set("score", ScriptValue::Int(score));
        }
    }
}

renzora::script!(update);
