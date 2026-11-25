//! A three-button ability bar with per-ability cooldowns plus a combo window.
//! Casting Q, W, E in that order inside the window triggers a finisher. Each
//! cooldown is its own float, so a HUD can read them independently.
//!
//! Setup: attach to the player; abilities are on Q, W and E. The animator
//! needs triggers "CastQ", "CastW", "CastE" and "Finisher".

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptInput};

/// The abilities: key, its letter in the combo, cooldown seconds, anim trigger.
const ABILITIES: [(KeyCode, char, f32, &str); 3] = [
    (KeyCode::KeyQ, 'Q', 1.5, "CastQ"),
    (KeyCode::KeyW, 'W', 2.5, "CastW"),
    (KeyCode::KeyE, 'E', 4.0, "CastE"),
];
/// Seconds after a cast in which the next cast still extends the combo.
const COMBO_WINDOW: f32 = 1.0;
const FINISHER: &str = "QWE";

#[derive(Component, Default)]
struct AbilityBar {
    /// Seconds left on each ability, in `ABILITIES` order.
    cooldowns: [f32; 3],
    combo: String,
    combo_timer: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<AbilityBar>() {
        ctx.insert(AbilityBar::default());
    }
    let pressed: [bool; 3] = match ctx.get_resource::<ScriptInput>() {
        Some(input) => ABILITIES.map(|(key, ..)| input.is_key_just_pressed(key)),
        None => [false; 3],
    };

    let Some(mut bar) = ctx.get_mut::<AbilityBar>() else { return };
    for cd in bar.cooldowns.iter_mut() {
        *cd = (*cd - dt).max(0.0);
    }
    // The combo buffer lapses when the window runs out.
    if bar.combo_timer > 0.0 {
        bar.combo_timer -= dt;
        if bar.combo_timer <= 0.0 {
            bar.combo.clear();
        }
    }

    let mut casts = Vec::new();
    let mut finisher = false;
    for (i, (_, letter, cost, trigger)) in ABILITIES.iter().enumerate() {
        if !pressed[i] || bar.cooldowns[i] > 0.0 {
            continue;
        }
        bar.cooldowns[i] = *cost;
        casts.push(*trigger);
        bar.combo.push(*letter);
        bar.combo_timer = COMBO_WINDOW;
        let excess = bar.combo.len().saturating_sub(FINISHER.len());
        bar.combo.drain(..excess);
        if bar.combo == FINISHER {
            finisher = true;
            bar.combo.clear();
        }
    }
    drop(bar);

    let me = ctx.entity();
    for trigger in casts {
        trigger_anim(ctx, trigger);
        play_sound(ctx, "cast");
    }
    if finisher {
        trigger_anim(ctx, "Finisher");
        play_sound(ctx, "finisher");
        ScriptAction::new("screen_shake", me)
            .arg("intensity", 0.5)
            .arg("duration", 0.4)
            .send(ctx.world());
    }
}

fn trigger_anim(ctx: &mut ScriptCtx, name: &str) {
    let me = ctx.entity();
    ScriptAction::new("trigger_anim", me).arg("name", name).send(ctx.world());
}

fn play_sound(ctx: &mut ScriptCtx, path: &str) {
    let me = ctx.entity();
    ScriptAction::new("play_sound", me)
        .arg("path", path)
        .arg("volume", 1.0)
        .arg("looping", false)
        .arg("bus", "Sfx")
        .send(ctx.world());
}

renzora::script!(update);
