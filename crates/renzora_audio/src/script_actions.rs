//! Bridge script/blueprint audio actions to the audio command queue.
//!
//! Lua's `play_sound` / `play_music` / `stop_music` / `stop_all_sounds` (and the
//! blueprint equivalents) are emitted by `renzora_scripting` as generic
//! `ScriptAction` events, on the expectation that the audio crate observes them
//! (see the routing comment in `renzora_scripting::systems::commands`). This
//! observer is that consumer — without it, script-driven audio is silently
//! dropped while component-driven audio (AudioPlayer) still works.
//!
//! It also handles `play_audio_player`, which fires a one-shot from a target
//! entity's `AudioPlayer` component (picking a random clip from its pool with
//! per-shot pitch/volume jitter) — the "component owns the data, script fires
//! the verb" model.

use std::collections::HashMap;

use bevy::prelude::*;
use renzora::{ScriptAction, ScriptActionValue};

use crate::commands::{AudioCommand, AudioCommandQueue};
use crate::components::AudioPlayer;

/// Per-entity runtime state for `AudioPlayer` one-shots: the last clip index
/// played (so we can avoid repeats) and a tiny PRNG for selection + jitter.
#[derive(Resource)]
pub struct AudioPlayerRuntime {
    last_clip: HashMap<Entity, usize>,
    rng: u64,
}

impl Default for AudioPlayerRuntime {
    fn default() -> Self {
        Self {
            last_clip: HashMap::new(),
            rng: 0x9E37_79B9_7F4A_7C15,
        }
    }
}

impl AudioPlayerRuntime {
    /// xorshift64 — dependency-free PRNG, good enough for audio variation.
    fn next_u64(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    /// Uniform float in [0, 1).
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Pick an index in `0..len`, avoiding `avoid` when there's a choice.
    fn pick(&mut self, len: usize, avoid: usize) -> usize {
        if len <= 1 {
            return 0;
        }
        let mut idx = (self.next_u64() % len as u64) as usize;
        if idx == avoid {
            idx = (idx + 1) % len;
        }
        idx
    }

    /// Apply a symmetric +/- jitter to a base value, clamped to >= `min`.
    fn jitter(&mut self, base: f32, amount: f32, min: f32) -> f32 {
        if amount <= 0.0 {
            return base;
        }
        (base + (self.next_f32() * 2.0 - 1.0) * amount).max(min)
    }
}

/// Observe `ScriptAction`s and forward audio ones to the `AudioCommandQueue`.
pub fn handle_audio_script_actions(
    trigger: On<ScriptAction>,
    mut queue: ResMut<AudioCommandQueue>,
    mut runtime: ResMut<AudioPlayerRuntime>,
    players: Query<(&AudioPlayer, Option<&GlobalTransform>)>,
    names: Query<(Entity, &Name)>,
) {
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
        // Fire a one-shot from a target entity's AudioPlayer component.
        "play_audio_player" => {
            // Target: explicit name, else the script's own entity.
            let target = match &action.target_entity {
                Some(name) => names
                    .iter()
                    .find(|(_, n)| n.as_str() == name)
                    .map(|(e, _)| e),
                None => Some(action.entity),
            };
            let Some(target) = target else {
                return;
            };
            let Ok((player, xform)) = players.get(target) else {
                return;
            };

            // Build the clip pool: `clips` if any, else fall back to `clip`.
            let pool: Vec<String> = if !player.clips.is_empty() {
                player
                    .clips
                    .iter()
                    .filter(|c| !c.is_empty())
                    .cloned()
                    .collect()
            } else if !player.clip.is_empty() {
                vec![player.clip.clone()]
            } else {
                Vec::new()
            };
            if pool.is_empty() {
                return;
            }

            let last = runtime.last_clip.get(&target).copied().unwrap_or(usize::MAX);
            let idx = runtime.pick(pool.len(), last);
            runtime.last_clip.insert(target, idx);

            // Apply per-shot jitter to a clone, then reuse the PlayEntity path.
            let mut shot = player.clone();
            shot.clip = pool[idx].clone();
            shot.looping = false; // one-shots never loop
            shot.volume = runtime.jitter(player.volume, player.volume_jitter, 0.0);
            shot.pitch = runtime.jitter(player.pitch, player.pitch_jitter, 0.01);

            let position = xform.map(|t| t.translation()).unwrap_or(Vec3::ZERO);
            queue.push(AudioCommand::PlayEntity {
                entity: target,
                player: shot,
                position,
            });
        }
        _ => {}
    }
}
