//! Steps the material through a palette, one colour per key press. The index
//! advances only on the just-pressed edge so one tap is one step, and the
//! palette is plain data so it is easy to extend.
//!
//! Setup: attach to an entity with a material; press C to advance.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptInput};

const CYCLE_KEY: KeyCode = KeyCode::KeyC;
/// RGB colours, in cycle order.
const PALETTE: [[f32; 3]; 3] = [[0.9, 0.2, 0.2], [0.2, 0.8, 0.3], [0.2, 0.4, 0.9]];

#[derive(Component)]
struct ColorCycle {
    index: usize,
}

fn update(ctx: &mut ScriptCtx) {
    if !ctx.has::<ColorCycle>() {
        ctx.insert(ColorCycle { index: 0 });
        apply(ctx, 0);
        return;
    }
    let pressed = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(CYCLE_KEY));
    if !pressed {
        return;
    }
    let Some(mut cycle) = ctx.get_mut::<ColorCycle>() else { return };
    cycle.index = (cycle.index + 1) % PALETTE.len();
    let index = cycle.index;
    drop(cycle);
    apply(ctx, index);
}

fn apply(ctx: &mut ScriptCtx, index: usize) {
    let [r, g, b] = PALETTE[index];
    let me = ctx.entity();
    ScriptAction::new("set_material_color", me)
        .arg("r", r)
        .arg("g", g)
        .arg("b", b)
        .arg("a", 1.0)
        .send(ctx.world());
}

renzora::script!(update);
