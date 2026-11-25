//! Fires an ability on a key press, gated by a cooldown. While the cooldown
//! runs the key does nothing, so the ability can never double-fire.
//!
//! Setup: attach to the caster; press Q to use the ability.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptInput};

const ABILITY_KEY: KeyCode = KeyCode::KeyQ;
/// Seconds between uses.
const COOLDOWN: f32 = 2.0;
const CAST_SOUND: &str = "audio/cast.ogg";

#[derive(Component)]
struct Ability {
    /// Seconds until the ability is ready again; zero or less means ready.
    cooldown: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Ability>() {
        ctx.insert(Ability { cooldown: 0.0 });
    }
    let pressed = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(ABILITY_KEY));
    let Some(mut ability) = ctx.get_mut::<Ability>() else { return };
    if ability.cooldown > 0.0 {
        ability.cooldown -= dt;
        return;
    }
    if !pressed {
        return;
    }
    ability.cooldown = COOLDOWN;
    drop(ability);

    // Stand-ins for the actual ability effect.
    let me = ctx.entity();
    ScriptAction::new("screen_shake", me)
        .arg("intensity", 0.3)
        .arg("duration", 0.15)
        .send(ctx.world());
    ScriptAction::new("play_sound", me)
        .arg("path", CAST_SOUND)
        .arg("volume", 0.7)
        .arg("looping", false)
        .arg("bus", "Sfx")
        .send(ctx.world());
}

renzora::script!(update);
