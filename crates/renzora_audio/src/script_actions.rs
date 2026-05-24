//! Bridge script/blueprint audio actions to the audio command queue.
//!
//! Lua's `play_sound` / `play_music` / `stop_music` / `stop_all_sounds` (and the
//! blueprint equivalents) are emitted by `renzora_scripting` as generic
//! `ScriptAction` events, on the expectation that the audio crate observes them
//! (see the routing comment in `renzora_scripting::systems::commands`). This
//! observer is that missing consumer — without it, script-driven audio is
//! silently dropped while component-driven audio (AudioPlayer) still works.

use bevy::prelude::*;
use renzora::{ScriptAction, ScriptActionValue};

use crate::commands::{AudioCommand, AudioCommandQueue};

/// Observe `ScriptAction`s and forward audio ones to the `AudioCommandQueue`.
pub fn handle_audio_script_actions(trigger: On<ScriptAction>, mut queue: ResMut<AudioCommandQueue>) {
    let action = trigger.event();

    let arg_str = |k: &str| match action.args.get(k) {
        Some(ScriptActionValue::String(v)) => Some(v.clone()),
        _ => None,
    };
    let arg_f32 = |k: &str| match action.args.get(k) {
        Some(ScriptActionValue::Float(v)) => Some(*v),
        _ => None,
    };
    let arg_bool = |k: &str| match action.args.get(k) {
        Some(ScriptActionValue::Bool(v)) => Some(*v),
        _ => None,
    };

    match action.name.as_str() {
        "play_sound" => {
            let Some(path) = arg_str("path") else {
                return;
            };
            queue.push(AudioCommand::PlaySound {
                path,
                volume: arg_f32("volume").unwrap_or(1.0),
                looping: arg_bool("looping").unwrap_or(false),
                bus: arg_str("bus").unwrap_or_else(|| "Sfx".to_string()),
                entity: None,
            });
        }
        "play_music" => {
            let Some(path) = arg_str("path") else {
                return;
            };
            queue.push(AudioCommand::PlayMusic {
                path,
                volume: arg_f32("volume").unwrap_or(1.0),
                fade_in: arg_f32("fade_in").unwrap_or(0.0),
                bus: arg_str("bus").unwrap_or_else(|| "Music".to_string()),
            });
        }
        "stop_music" => {
            queue.push(AudioCommand::StopMusic {
                fade_out: arg_f32("fade_out").unwrap_or(0.0),
            });
        }
        "stop_all_sounds" => {
            queue.push(AudioCommand::StopAllSounds);
        }
        _ => {}
    }
}
