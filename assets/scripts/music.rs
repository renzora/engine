//! Background music: starts a looping track when the game starts. Playback is
//! driven from the script, so the entity needs no AudioPlayer component.
//!
//! Music streams from disk, loops on its own and routes to the "Music" mixer
//! bus, so the mixer's Music fader controls it. Formats: .ogg (recommended
//! for music), .wav, .flac, .mp3.
//!
//! Setup: attach to any entity. `CLIP` is relative to the project folder: a
//! file at `<project>/audio/theme.ogg` is `"audio/theme.ogg"`.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook};

const CLIP: &str = "audio/background_music.mp3";
/// 0..2, where 1 is unity gain.
const VOLUME: f32 = 0.7;
/// Fade-in seconds; 0 starts at full volume.
const FADE_IN: f32 = 1.5;
const PLAY_ON_START: bool = true;

fn update(_ctx: &mut ScriptCtx) {}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if !matches!(hook, ScriptHook::Ready) || !PLAY_ON_START || CLIP.is_empty() {
        return;
    }
    let me = ctx.entity();
    ScriptAction::new("play_music", me)
        .arg("path", CLIP)
        .arg("volume", VOLUME)
        .arg("fade_in", FADE_IN)
        .arg("bus", "Music")
        .send(ctx.world());
}

renzora::script!(update, hooks = hooks);
