//! A health bar: passive regen, a test hotkey for damage, a child bar whose X
//! scale is the health fraction, and a material that shifts from green to red
//! as health drops, flashing red briefly on a hit.
//!
//! The Lua version read and wrote `Health.current` by reflection, but the
//! engine has no `Health` component, so it never did anything. Health lives in
//! this script's own `Health` component instead; other Rust scripts can read
//! or damage it there and the bar follows.
//!
//! Setup: attach to the character; a child named "fill" is the bar. Press H to
//! take damage.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptInput};

const MAX_HEALTH: f32 = 100.0;
const REGEN_PER_SEC: f32 = 3.0;
/// Damage the test hotkey deals.
const HIT_DAMAGE: f32 = 15.0;
/// The bar's X scale at full health.
const BAR_FULL_WIDTH: f32 = 1.0;
/// Seconds the bar flashes red after a hit.
const FLASH_TIME: f32 = 0.3;

#[derive(Component)]
struct Health {
    current: f32,
    max: f32,
    /// Seconds of hit flash left.
    flash: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Health>() {
        ctx.insert(Health { current: MAX_HEALTH, max: MAX_HEALTH, flash: 0.0 });
    }
    let hit = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(KeyCode::KeyH));

    let Some(mut health) = ctx.get_mut::<Health>() else { return };
    if health.max <= 0.0 {
        return;
    }
    let max = health.max;
    if health.current < max {
        health.current = (health.current + REGEN_PER_SEC * dt).clamp(0.0, max);
    }
    if hit {
        health.current = (health.current - HIT_DAMAGE).clamp(0.0, max);
        health.flash = FLASH_TIME;
    }
    let frac = health.current / max;
    let flashing = health.flash > 0.0;
    if flashing {
        health.flash -= dt;
    }
    drop(health);

    let fill = ctx
        .children()
        .into_iter()
        .find(|&child| ctx.get_on::<Name>(child).is_some_and(|n| n.as_str() == "fill"));
    if let Some(fill) = fill {
        if let Some(mut transform) = ctx.get_mut_on::<Transform>(fill) {
            transform.scale.x = frac * BAR_FULL_WIDTH;
        }
    }

    let (r, g, b) = if flashing { (1.0, 0.1, 0.1) } else { (1.0 - frac, frac, 0.1) };
    let me = ctx.entity();
    ScriptAction::new("set_material_color", me)
        .arg("r", r)
        .arg("g", g)
        .arg("b", b)
        .arg("a", 1.0)
        .send(ctx.world());
}

renzora::script!(update);
