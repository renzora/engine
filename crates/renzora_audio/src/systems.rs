//! Turning queued commands and world state into calls on the backend.
//!
//! Nothing here knows what a sound card is. Commands name asset paths and
//! entities; [`AudioLink`] takes handles and samples; this is the layer that
//! turns one into the other.

use bevy::prelude::*;

use renzora_plugin::audio::{EmitterState, PlayRequest, StopRequest, StopTarget, UpdateRequest};

use crate::commands::{AudioCommand, AudioCommandQueue};
use crate::components::{AudioPlayer, RolloffType};
use crate::link::{AudioLink, VoiceId};
use crate::preview::AudioPreviewState;
use crate::runtime::{ActiveVoices, SoundCache};

/// Marker component for the audio listener entity (the "ears" in 3D space).
#[derive(Component, Clone, Debug)]
pub struct AudioListener {
    pub active: bool,
}

impl Default for AudioListener {
    fn default() -> Self {
        Self { active: true }
    }
}

/// System set for ordering audio systems.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AudioSet {
    Commands,
    Sync,
    Cleanup,
}

/// The music voice, if one is playing.
///
/// One voice rather than an entity's, because `play_music` has always meant
/// "there is one soundtrack" — a second call replaces the first.
#[derive(Resource, Default)]
pub struct MusicVoice(pub Option<VoiceId>);

/// A runtime volume multiplier applied on top of the mixer's master strip.
///
/// Separate from the strip because they mean different things: the strip is the
/// project's mix, authored in the panel and saved to `project.toml`, while this
/// is what a game's own volume slider drives. Folding them together would let a
/// player's setting rewrite the developer's mix.
#[derive(Resource)]
pub struct MasterVolume(pub f32);

impl Default for MasterVolume {
    fn default() -> Self {
        Self(1.0)
    }
}

/// A play request with everything at its neutral value.
///
/// An empty bus key becomes `Sfx` — that is what an `AudioPlayer` left untouched
/// carries, and the backend would otherwise route it to master. An unknown
/// *non-empty* key is passed through on purpose: a scene authored against a
/// since-deleted bus should be audible and wrong rather than silent, which is a
/// bug nobody can find.
fn request(bus: &str) -> PlayRequest {
    PlayRequest {
        voice: 0,
        clip: 0,
        bus: if bus.is_empty() { "Sfx".into() } else { bus.into() },
        gain: 1.0,
        pan: 0.0,
        pitch: 1.0,
        looping: None,
        fade_in: 0.0,
        start: 0.0,
        emitter: None,
        reverb_send: 0.0,
        delay_send: 0.0,
    }
}

/// Emitter parameters from an `AudioPlayer`'s spatial fields.
fn emitter_of(player: &AudioPlayer, position: Vec3) -> EmitterState {
    EmitterState {
        position: position.to_array(),
        min_distance: player.spatial_min_distance,
        max_distance: player.spatial_max_distance,
        rolloff: match player.spatial_rolloff {
            RolloffType::Linear => 1,
            RolloffType::Logarithmic => 0,
        },
    }
}

/// Process queued audio commands.
#[allow(clippy::too_many_arguments)]
pub fn process_audio_commands(
    mut queue: ResMut<AudioCommandQueue>,
    mut link: ResMut<AudioLink>,
    mut cache: ResMut<SoundCache>,
    mut voices: ResMut<ActiveVoices>,
    mut music: ResMut<MusicVoice>,
    mut master: ResMut<MasterVolume>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    if queue.is_empty() {
        return;
    }
    let project = project.as_deref();
    // Parameter changes are batched and sent once at the end: they all ride the
    // same call, and a command issued this frame should be heard this frame.
    let mut batch = UpdateRequest::default();
    let master_volume = master.0;

    // Load, start, and record. Takes its resources as arguments rather than
    // capturing them, so the borrow checker can see that each arm below uses
    // them one at a time.
    fn start(
        link: &mut AudioLink,
        cache: &mut SoundCache,
        voices: &mut ActiveVoices,
        project: Option<&renzora::core::CurrentProject>,
        master: f32,
        path: &str,
        entity: Option<Entity>,
        mut r: PlayRequest,
    ) -> Option<VoiceId> {
        let sound = cache.get_or_load(link, project, path)?;
        let voice = link.next_voice();
        r.voice = voice.0;
        r.clip = sound.0;
        r.gain = (r.gain * master).clamp(0.0, 2.0);
        if let Err(e) = link.play(&r) {
            warn!("[audio] could not play `{path}`: {e}");
            return None;
        }
        if let Some(entity) = entity {
            voices.insert(entity, voice);
        }
        Some(voice)
    }

    for cmd in queue.drain() {
        match cmd {
            AudioCommand::PlaySound {
                path,
                volume,
                looping,
                bus,
                entity,
            } => {
                let mut r = request(&bus);
                r.gain = volume;
                // `(0, 0)` is the idiom for "loop the whole clip": the backend
                // clamps a degenerate region to the full length.
                r.looping = looping.then_some((0.0, 0.0));
                start(
                    &mut link,
                    &mut cache,
                    &mut voices,
                    project,
                    master_volume,
                    &path,
                    entity,
                    r,
                );
            }

            AudioCommand::PlayEntity {
                entity,
                player,
                position,
            } => {
                if player.clip.is_empty() {
                    continue;
                }
                let mut r = request(&player.bus);
                r.gain = player.volume;
                r.pitch = player.pitch.max(0.01) as f64;
                r.fade_in = player.fade_in;
                r.reverb_send = player.reverb_send;
                r.delay_send = player.delay_send;
                if player.looping {
                    r.looping = Some((player.loop_start, player.loop_end));
                }
                if player.spatial {
                    // Pan comes from listener geometry for a positioned sound, so
                    // the authored pan is left centred rather than fighting it —
                    // which is what the spatial path always did.
                    r.emitter = Some(emitter_of(&player, position));
                } else {
                    r.pan = player.panning;
                }
                start(
                    &mut link,
                    &mut cache,
                    &mut voices,
                    project,
                    master_volume,
                    &player.clip,
                    Some(entity),
                    r,
                );
            }

            AudioCommand::PlaySound3D {
                path,
                volume,
                position,
                bus,
                entity,
            } => {
                let mut r = request(&bus);
                r.gain = volume;
                r.emitter = Some(EmitterState {
                    position: position.to_array(),
                    min_distance: 1.0,
                    max_distance: 50.0,
                    rolloff: 0,
                });
                start(
                    &mut link,
                    &mut cache,
                    &mut voices,
                    project,
                    master_volume,
                    &path,
                    entity,
                    r,
                );
            }

            AudioCommand::PlayMusic {
                path,
                volume,
                fade_in,
                bus,
            } => {
                if let Some(previous) = music.0.take() {
                    link.stop(&StopRequest {
                        target: StopTarget::Voice(previous.0),
                        fade: 0.0,
                    });
                }
                let mut r = request(&bus);
                r.gain = volume;
                r.fade_in = fade_in;
                r.looping = Some((0.0, 0.0));
                // No entity: music outlives whatever asked for it, and there is
                // nothing for it to be cleaned up alongside.
                music.0 = start(
                    &mut link,
                    &mut cache,
                    &mut voices,
                    project,
                    master_volume,
                    &path,
                    None,
                    r,
                );
            }

            AudioCommand::StopMusic { fade_out } => {
                if let Some(voice) = music.0.take() {
                    link.stop(&StopRequest {
                        target: StopTarget::Voice(voice.0),
                        fade: fade_out,
                    });
                }
            }

            AudioCommand::CrossfadeMusic {
                path,
                volume,
                duration,
                bus,
            } => {
                // The old track fades out over the same span the new one fades
                // in, which is what makes this a crossfade rather than a gap.
                if let Some(previous) = music.0.take() {
                    link.stop(&StopRequest {
                        target: StopTarget::Voice(previous.0),
                        fade: duration,
                    });
                }
                let mut r = request(&bus);
                r.gain = volume;
                r.fade_in = duration;
                r.looping = Some((0.0, 0.0));
                music.0 = start(
                    &mut link,
                    &mut cache,
                    &mut voices,
                    project,
                    master_volume,
                    &path,
                    None,
                    r,
                );
            }

            AudioCommand::StopAllSounds => {
                link.stop(&StopRequest {
                    target: StopTarget::All,
                    fade: 0.0,
                });
                music.0 = None;
                *voices = ActiveVoices::default();
            }

            AudioCommand::SetMasterVolume { volume } => {
                master.0 = volume.clamp(0.0, 1.0);
            }

            AudioCommand::PauseSound { entity } => {
                for voice in targets(&voices, &music, entity) {
                    batch.paused.push((voice.0, true));
                }
            }

            AudioCommand::ResumeSound { entity } => {
                for voice in targets(&voices, &music, entity) {
                    batch.paused.push((voice.0, false));
                }
            }

            AudioCommand::SetSoundVolume { entity, volume, .. } => {
                // `fade` is accepted and ignored. The backend ramps a gain change
                // over a block regardless, and a per-parameter tween would be a
                // whole automation system for a value nothing in the editor
                // animates. Taking the argument and not acting on the tween beats
                // removing it and breaking every caller.
                for voice in voices.of(entity) {
                    batch.gains.push((voice.0, volume * master_volume));
                }
            }

            AudioCommand::SetSoundPitch { entity, pitch, .. } => {
                for voice in voices.of(entity) {
                    batch.pitches.push((voice.0, pitch as f64));
                }
            }
        }
    }

    if !batch.gains.is_empty() || !batch.pitches.is_empty() || !batch.paused.is_empty() {
        if let Err(e) = link.update(&batch) {
            warn!("[audio] {e}");
        }
    }
}

/// Which voices a pause or resume applies to. `None` means everything, music
/// included — that is what a global pause has always meant.
fn targets(voices: &ActiveVoices, music: &MusicVoice, entity: Option<Entity>) -> Vec<VoiceId> {
    match entity {
        Some(entity) => voices.of(entity).to_vec(),
        None => {
            let mut all = voices.all();
            all.extend(music.0);
            all
        }
    }
}

/// Push moved emitters to the backend each frame.
pub fn sync_spatial_audio(
    mut link: ResMut<AudioLink>,
    voices: Res<ActiveVoices>,
    transforms: Query<&GlobalTransform>,
) {
    if !link.is_active() || voices.is_empty() {
        return;
    }
    let mut moved = Vec::new();
    for (entity, ids) in voices.iter() {
        let Ok(transform) = transforms.get(entity) else {
            continue;
        };
        let position = transform.translation().to_array();
        moved.extend(ids.iter().map(|id| (id.0, position)));
    }
    if moved.is_empty() {
        return;
    }
    let request = UpdateRequest {
        moved,
        ..Default::default()
    };
    if let Err(e) = link.update(&request) {
        warn!("[audio] {e}");
    }
}

/// Stop and forget voices whose entity has gone away.
///
/// Without this a despawned emitter plays to its natural end from wherever it
/// died, and its bookkeeping never clears. A short fade rather than an abrupt
/// stop, because a cut mid-waveform is a click.
pub fn drop_despawned_voices(
    mut link: ResMut<AudioLink>,
    mut voices: ResMut<ActiveVoices>,
    alive: Query<Entity>,
) {
    if voices.is_empty() {
        return;
    }
    let gone: Vec<Entity> = voices.entities().filter(|e| alive.get(*e).is_err()).collect();
    for entity in gone {
        for voice in voices.forget(entity) {
            link.stop(&StopRequest {
                target: StopTarget::Voice(voice.0),
                fade: 0.02,
            });
        }
    }
}

/// Clear the preview once its voice has finished.
pub fn preview_audio_system(
    mut preview: Option<ResMut<AudioPreviewState>>,
    voices: Res<ActiveVoices>,
) {
    let Some(preview) = preview.as_mut() else {
        return;
    };
    let Some(voice) = preview.voice else { return };
    // The backend reports finishes by dropping them from `ActiveVoices`, so
    // "still tracked" and "still playing" are the same question.
    if !voices.contains(voice) {
        preview.clear();
    }
}
