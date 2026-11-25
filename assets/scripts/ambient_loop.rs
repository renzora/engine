//! Starts a looping ambient bed once, when the script becomes ready. Starting
//! it from `update` instead would stack a new copy of the loop every frame.
//!
//! Setup: attach to any entity and point `AMBIENT` at a seamless loop (wind,
//! hum, rain).

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook};

const AMBIENT: &str = "audio/ambient_wind.ogg";
/// 0..1.
const VOLUME: f32 = 0.5;

fn update(_ctx: &mut ScriptCtx) {}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let me = ctx.entity();
        ScriptAction::new("play_sound", me)
            .arg("path", AMBIENT)
            .arg("volume", VOLUME)
            .arg("looping", true)
            .arg("bus", "Sfx")
            .send(ctx.world());
    }
}

renzora::script!(update, hooks = hooks);
