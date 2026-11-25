//! A pickup counter. Coins are collected by proximity rather than events, and
//! every change pushes the new count to a HUD label. Reaching the goal fires
//! once. K adds a coin, so the counter is testable without world coins.
//!
//! The Lua original declared a pickup radius and a "coin_" prefix but never
//! collected anything; this port does: a coin within `PICKUP_RADIUS` is
//! despawned and counted.
//!
//! Setup: attach to the player. Coins are entities named "coin_<n>"; a UI text
//! named "CoinCount" shows the tally.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

const GOAL: u32 = 10;
const PICKUP_RADIUS: f32 = 1.2;
const COIN_PREFIX: &str = "coin_";
const LABEL: &str = "CoinCount";

#[derive(Component, Default)]
struct Inventory {
    coins: u32,
    won: bool,
}

fn update(ctx: &mut ScriptCtx) {
    let Some(won) = ctx.get::<Inventory>().map(|i| i.won) else { return };
    if won {
        return;
    }
    let debug_add = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(KeyCode::KeyK));
    let Some(here) = ctx.get::<Transform>().map(|t| t.translation) else { return };

    let world = ctx.world();
    let mut coins = world.query::<(Entity, &Name, &GlobalTransform)>();
    let picked: Vec<Entity> = coins
        .iter(world)
        .filter(|(_, n, t)| {
            n.as_str().starts_with(COIN_PREFIX) && t.translation().distance_squared(here) <= PICKUP_RADIUS * PICKUP_RADIUS
        })
        .map(|(e, _, _)| e)
        .collect();
    for coin in &picked {
        world.despawn(*coin);
    }

    let gained = picked.len() as u32 + u32::from(debug_add);
    if gained > 0 {
        add_coins(ctx, gained);
    }

    let Some(mut inventory) = ctx.get_mut::<Inventory>() else { return };
    if inventory.coins < GOAL {
        return;
    }
    inventory.won = true;
    drop(inventory);
    play_sound(ctx, "fanfare");
    set_label(ctx, "ALL COINS!".to_string());
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        ctx.insert(Inventory::default());
        refresh_label(ctx);
    }
}

fn add_coins(ctx: &mut ScriptCtx, n: u32) {
    if let Some(mut inventory) = ctx.get_mut::<Inventory>() {
        inventory.coins += n;
    }
    play_sound(ctx, "coin");
    refresh_label(ctx);
}

fn refresh_label(ctx: &mut ScriptCtx) {
    let coins = ctx.get::<Inventory>().map_or(0, |i| i.coins);
    set_label(ctx, format!("{coins} / {GOAL}"));
}

fn set_label(ctx: &mut ScriptCtx, text: String) {
    let me = ctx.entity();
    ScriptAction::new("ui_set_text", me).arg("name", LABEL).arg("text", text).send(ctx.world());
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

renzora::script!(update, hooks = hooks);
