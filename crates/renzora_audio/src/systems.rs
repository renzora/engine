//! Turning queued commands and world state into calls on the backend.
//!
//! Nothing here knows what a sound card is. Commands name asset paths and
//! entities; [`AudioLink`] takes handles and samples; this is the layer that
//! turns one into the other.

use bevy::prelude::*;

use renzora_audio_api::audio_backend::{EmitterState, PlayRequest, StopRequest, StopTarget, UpdateRequest};
use renzora_audio_api::settings;

use crate::commands::{AudioCommand, AudioCommandQueue};
use crate::components::{AudioPlayer, RolloffType};
use crate::link::{AudioLink, VoiceId};
use crate::mixer::MixerState;
use crate::preview::AudioPreviewState;
use crate::runtime::{ActiveVoices, SoundCache};

/// The "ears" in 3D space — an **override**, not a requirement.
///
/// Without one, sound is heard from the game camera (`renzora_audio::runtime`
/// picks the same camera play mode renders through). Most games never need this
/// component: in first person, and in anything 2D, the camera *is* the ears.
///
/// Add one when the two come apart:
/// - **Third person** — the camera trails the character by several metres, so
///   camera ears put the player's own footsteps in front of them and misjudge
///   the distance to everything nearby.
/// - **Strategy / orthographic** — a camera pulled fifty metres back would
///   attenuate the whole scene past `spatial_max_distance` into silence.
/// - **Split screen** — with several viewport cameras, which one hears is a
///   decision, and this is where it is written down.
///
/// Only affects *spatial* voices: a sound with `AudioPlayer.spatial` off (and
/// that is the default) is played at its authored volume and pan and never
/// consults the listener at all.
///
/// `active: false` disables one without deleting it. With several active, the
/// first found wins — which is arbitrary, so don't rely on it.
///
/// `Reflect` + `#[reflect(Component)]` are what make it survive a save: the
/// scene serializer walks `AppTypeRegistry`, so a component that is not
/// registered is simply not written — no warning, no error, it is just gone when
/// the scene comes back. This one had none of the derives and nothing registered
/// it, so attaching a listener never outlived the session.
#[derive(Component, Clone, Debug, Reflect, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
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
    mut project: Option<ResMut<renzora_project_api::CurrentProject>>,
    mut mixer: Option<ResMut<MixerState>>,
) {
    if queue.is_empty() {
        return;
    }
    // Parameter changes are batched and sent once at the end: they all ride the
    // same call, and a command issued this frame should be heard this frame.
    let mut batch = UpdateRequest::default();

    // Load, start, and record. Takes its resources as arguments rather than
    // capturing them, so the borrow checker can see that each arm below uses
    // them one at a time.
    //
    // No master multiplier here: master volume is the Master bus's gain, which
    // the backend applies to everything on the board, so a voice is started at
    // its own gain and nothing else.
    fn start(
        link: &mut AudioLink,
        cache: &mut SoundCache,
        voices: &mut ActiveVoices,
        project: Option<&renzora_project_api::CurrentProject>,
        path: &str,
        entity: Option<Entity>,
        mut r: PlayRequest,
    ) -> Option<VoiceId> {
        let sound = cache.get_or_load(link, project, path)?;
        let voice = link.next_voice();
        r.voice = voice.0;
        r.clip = sound.0;
        r.gain = r.gain.clamp(0.0, 2.0);
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
                    project.as_deref(),
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
                    project.as_deref(),
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
                    project.as_deref(),
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
                    project.as_deref(),
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
                    project.as_deref(),
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
                set_bus_volume(project.as_mut(), mixer.as_mut(), "Master", volume as f64);
            }

            AudioCommand::SetBusVolume { bus, volume } => {
                set_bus_volume(project.as_mut(), mixer.as_mut(), &bus, volume as f64);
            }

            AudioCommand::SetBusMuted { bus, muted } => {
                set_bus_muted(project.as_mut(), mixer.as_mut(), &bus, muted);
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
                    batch.gains.push((voice.0, volume));
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

/// Set a bus's volume in the settings store, which `sync_mixer_with_settings`
/// then applies to the board on this same frame.
///
/// Going through the store rather than straight at `MixerState` is the point:
/// the store is what the editor's fader, a settings menu and a Rust script all
/// read, so a volume set here is the one they show. Compared first, because
/// taking `DerefMut` on `CurrentProject` marks the whole project changed, and a
/// script that sets the volume every frame should not churn it.
///
/// With no project at all (a bare test app, or a tool built without one) the
/// board is the only state there is, so the strip is set directly.
fn set_bus_volume(
    project: Option<&mut ResMut<renzora_project_api::CurrentProject>>,
    mixer: Option<&mut ResMut<MixerState>>,
    bus: &str,
    volume: f64,
) {
    let volume = settings::clamp_volume(volume);
    let known = match project {
        Some(project) => {
            match settings::bus_volume(&project.config.audio, bus) {
                Some(current) if current == volume => true,
                Some(_) => settings::set_bus_volume(&mut project.config.audio, bus, volume),
                None => false,
            }
        }
        None => match mixer.and_then(|m| m.strip_by_key_mut(bus)) {
            Some(strip) => {
                strip.volume = volume;
                true
            }
            None => false,
        },
    };
    if !known {
        warn!("[audio] set_bus_volume: no bus with key `{bus}`");
    }
}

/// [`set_bus_volume`], for mute.
fn set_bus_muted(
    project: Option<&mut ResMut<renzora_project_api::CurrentProject>>,
    mixer: Option<&mut ResMut<MixerState>>,
    bus: &str,
    muted: bool,
) {
    let known = match project {
        Some(project) => match settings::bus_muted(&project.config.audio, bus) {
            Some(current) if current == muted => true,
            Some(_) => settings::set_bus_muted(&mut project.config.audio, bus, muted),
            None => false,
        },
        None => match mixer.and_then(|m| m.strip_by_key_mut(bus)) {
            Some(strip) => {
                strip.muted = muted;
                true
            }
            None => false,
        },
    };
    if !known {
        warn!("[audio] set_bus_muted: no bus with key `{bus}`");
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

/// Apply edits made to a live `AudioPlayer` to the voices it is already playing.
///
/// Without this the component is read exactly once, when `autoplay` fires
/// `PlayEntity`, and every slider moved afterwards writes to the component and is
/// never looked at again — the field changes in the inspector and nothing
/// happens, which is indistinguishable from the control being broken.
///
/// Volume, pitch, pan, bus and the spatial parameters are applied to the running
/// voice, so dragging a slider is continuous and re-routing does not restart the
/// sound. Only `clip` and the `spatial` *toggle* restart it: a new file is
/// obviously a new sound, and a voice started without an emitter has nowhere to
/// put one — turning spatial on has to build the voice again.
pub fn apply_audio_player_edits(
    mut link: ResMut<AudioLink>,
    mut voices: ResMut<ActiveVoices>,
    mut queue: ResMut<AudioCommandQueue>,
    changed: Query<(Entity, &AudioPlayer, Option<&GlobalTransform>), Changed<AudioPlayer>>,
    mut last: Local<std::collections::HashMap<Entity, (String, bool)>>,
) {
    if changed.is_empty() {
        return;
    }
    let mut batch = UpdateRequest::default();
    let mut restart: Vec<(Entity, AudioPlayer, Vec3)> = Vec::new();

    for (entity, player, transform) in &changed {
        // Only the two things a live voice cannot be talked into. `bus` is not
        // among them any more — see `UpdateRequest::buses`.
        let structural = (player.clip.clone(), player.spatial);
        let rebuilt = last
            .get(&entity)
            .is_some_and(|previous| previous != &structural);
        last.insert(entity, structural);

        let live = voices.of(entity);
        if live.is_empty() {
            continue;
        }
        if rebuilt {
            let position = transform.map(|t| t.translation()).unwrap_or(Vec3::ZERO);
            restart.push((entity, player.clone(), position));
            continue;
        }
        for voice in live {
            batch.gains.push((voice.0, player.volume));
            batch.pitches.push((voice.0, player.pitch.max(0.01) as f64));
            batch.buses.push((voice.0, player.bus.clone()));
            // A spatial voice takes its pan from listener geometry, so pushing
            // the authored pan at one would fight the position every frame.
            if !player.spatial {
                batch.pans.push((voice.0, player.panning));
            } else if let Some(transform) = transform {
                batch
                    .emitters
                    .push((voice.0, emitter_of(player, transform.translation())));
            }
        }
    }

    if !batch.gains.is_empty() {
        if let Err(e) = link.update(&batch) {
            warn!("[audio] {e}");
        }
    }

    // Restarts go through the command queue rather than being played here, so
    // they take exactly the path a fresh `PlayEntity` does — one place decides
    // what a play means.
    for (entity, player, position) in restart {
        for voice in voices.forget(entity) {
            link.stop(&StopRequest {
                target: StopTarget::Voice(voice.0),
                fade: 0.02,
            });
        }
        queue.push(AudioCommand::PlayEntity {
            entity,
            player,
            position,
        });
    }
}
