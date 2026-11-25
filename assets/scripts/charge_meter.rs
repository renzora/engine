//! Hold a key to charge, release to fire with power scaled by the charge. The
//! material shifts from green to red as the meter fills, which shows the level
//! without any UI.
//!
//! Setup: attach to a dynamic rigid body with a material; hold Space to charge
//! and release to launch it along -Z.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptInput};

const CHARGE_KEY: KeyCode = KeyCode::Space;
/// Charge gained per second, from 0 to 1.
const RATE: f32 = 0.8;
/// Impulse at full charge.
const MAX_IMPULSE: f32 = 12.0;

#[derive(Component)]
struct Charge {
    charging: bool,
    /// 0..1.
    level: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Charge>() {
        ctx.insert(Charge { charging: false, level: 0.0 });
    }
    let held = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_pressed(CHARGE_KEY));
    let me = ctx.entity();
    let Some(mut charge) = ctx.get_mut::<Charge>() else { return };

    if held {
        charge.charging = true;
        charge.level = (charge.level + RATE * dt).clamp(0.0, 1.0);
        let level = charge.level;
        drop(charge);
        set_color(ctx, level, 1.0 - level, 0.0);
        return;
    }
    if !charge.charging {
        return;
    }
    let level = charge.level;
    charge.level = 0.0;
    charge.charging = false;
    drop(charge);

    let world = ctx.world();
    ScriptAction::new("apply_impulse", me)
        .arg("x", 0.0)
        .arg("y", 0.0)
        .arg("z", -level * MAX_IMPULSE)
        .send(world);
    ScriptAction::new("screen_shake", me)
        .arg("intensity", level * 0.5)
        .arg("duration", 0.2)
        .send(world);
    set_color(ctx, 0.0, 1.0, 0.0);
}

fn set_color(ctx: &mut ScriptCtx, r: f32, g: f32, b: f32) {
    let me = ctx.entity();
    ScriptAction::new("set_material_color", me)
        .arg("r", r)
        .arg("g", g)
        .arg("b", b)
        .arg("a", 1.0)
        .send(ctx.world());
}

renzora::script!(update);
