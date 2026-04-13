//! Kira audio systems
//!
//! Processes audio commands, syncs spatial audio, prunes finished sounds,
//! estimates VU meter levels, and manages preview playback.

use bevy::prelude::*;
use kira::{
    sound::static_sound::StaticSoundData,
    sound::streaming::StreamingSoundData,
    sound::PlaybackState,
    Tween,
};

use crate::commands::{AudioCommand, AudioCommandQueue};
use crate::manager::{KiraAudioManager, RolloffType, amplitude_to_db, vec3_to_mint, quat_to_mint};
use crate::mixer::MixerState;
use crate::preview::AudioPreviewState;

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

/// Process queued audio commands from the AudioCommandQueue using Kira.
pub fn process_audio_commands(
    mut queue: ResMut<AudioCommandQueue>,
    audio: Option<NonSendMut<KiraAudioManager>>,
    mixer: Option<Res<MixerState>>,
) {
    let Some(mut audio) = audio else { return };
    let Some(mixer) = mixer else { return };
    if queue.is_empty() { return; }

    for cmd in queue.drain() {
        match cmd {
            AudioCommand::PlaySound { path, volume, looping, bus, entity } => {
                let full_path = audio.resolve_path(&path);
                let effective_volume = (volume as f64 * audio.master_volume).clamp(0.0, 2.0);

                match StaticSoundData::from_file(&full_path) {
                    Ok(data) => {
                        let data = data.volume(amplitude_to_db(effective_volume));
                        let data = if looping {
                            data.loop_region(0.0..)
                        } else {
                            data
                        };

                        match audio.play_on_bus(data, &bus, &mixer) {
                            Ok(handle) => {
                                if let Some(ent) = entity {
                                    audio.track_sound(ent, handle);
                                }
                                debug!("[KiraAudio] Playing sound: {} on bus: {}", path, bus);
                            }
                            Err(e) => warn!("[KiraAudio] Failed to play {}: {}", path, e),
                        }
                    }
                    Err(e) => warn!("[KiraAudio] Failed to load {}: {}", path, e),
                }
            }

            AudioCommand::PlaySound3D { path, volume, position, bus, entity } => {
                let full_path = audio.resolve_path(&path);
                let effective_volume = (volume as f64 * audio.master_volume).clamp(0.0, 2.0);

                match StaticSoundData::from_file(&full_path) {
                    Ok(data) => {
                        let data = data.volume(amplitude_to_db(effective_volume));

                        if let Some(ent) = entity {
                            if let Some(spatial_track) = audio.get_or_create_spatial_track(
                                ent, position, &bus, 1.0, 50.0, &RolloffType::Logarithmic, &mixer,
                            ) {
                                match spatial_track.play(data) {
                                    Ok(handle) => {
                                        audio.track_sound(ent, handle);
                                        debug!("[KiraAudio] Playing 3D sound: {} at {:?}", path, position);
                                    }
                                    Err(e) => warn!("[KiraAudio] Failed to play 3D sound {}: {}", path, e),
                                }
                            }
                        } else {
                            // No entity - fallback to non-spatial playback
                            match audio.play_on_bus(data, &bus, &mixer) {
                                Ok(_handle) => {
                                    debug!("[KiraAudio] Playing 3D sound (no entity): {}", path);
                                }
                                Err(e) => warn!("[KiraAudio] Failed to play 3D sound {}: {}", path, e),
                            }
                        }
                    }
                    Err(e) => warn!("[KiraAudio] Failed to load {}: {}", path, e),
                }
            }

            AudioCommand::PlayMusic { path, volume, fade_in, bus } => {
                audio.stop_music(0.0);

                let full_path = audio.resolve_path(&path);
                let effective_volume = (volume as f64 * audio.master_volume).clamp(0.0, 2.0);

                match StreamingSoundData::from_file(&full_path) {
                    Ok(data) => {
                        let data = data
                            .volume(amplitude_to_db(effective_volume))
                            .loop_region(0.0..);

                        let data = if fade_in > 0.0 {
                            data.fade_in_tween(Tween {
                                duration: std::time::Duration::from_secs_f32(fade_in),
                                ..Default::default()
                            })
                        } else {
                            data
                        };

                        match audio.play_on_bus(data, &bus, &mixer) {
                            Ok(handle) => {
                                audio.music_handle = Some(handle);
                                info!("[KiraAudio] Playing music: {} on bus: {}", path, bus);
                            }
                            Err(e) => warn!("[KiraAudio] Failed to play music {}: {}", path, e),
                        }
                    }
                    Err(e) => warn!("[KiraAudio] Failed to load music {}: {}", path, e),
                }
            }

            AudioCommand::StopMusic { fade_out } => {
                audio.stop_music(fade_out);
                info!("[KiraAudio] Music stopped (fade={}s)", fade_out);
            }

            AudioCommand::StopAllSounds => {
                audio.stop_all_sounds();
                audio.stop_music(0.0);
                info!("[KiraAudio] All sounds stopped");
            }

            AudioCommand::SetMasterVolume { volume } => {
                audio.master_volume = (volume as f64).clamp(0.0, 1.0);
                debug!("[KiraAudio] Master volume set to {}", audio.master_volume);
            }

            AudioCommand::PauseSound { entity } => {
                if let Some(entity) = entity {
                    if let Some(handles) = audio.active_sounds.get_mut(&entity) {
                        for handle in handles.iter_mut() {
                            let _ = handle.pause(Tween::default());
                        }
                    }
                } else {
                    for handles in audio.active_sounds.values_mut() {
                        for handle in handles.iter_mut() {
                            let _ = handle.pause(Tween::default());
                        }
                    }
                    if let Some(ref mut h) = audio.music_handle {
                        let _ = h.pause(Tween::default());
                    }
                }
            }

            AudioCommand::ResumeSound { entity } => {
                if let Some(entity) = entity {
                    if let Some(handles) = audio.active_sounds.get_mut(&entity) {
                        for handle in handles.iter_mut() {
                            let _ = handle.resume(Tween::default());
                        }
                    }
                } else {
                    for handles in audio.active_sounds.values_mut() {
                        for handle in handles.iter_mut() {
                            let _ = handle.resume(Tween::default());
                        }
                    }
                    if let Some(ref mut h) = audio.music_handle {
                        let _ = h.resume(Tween::default());
                    }
                }
            }

            AudioCommand::SetSoundVolume { entity, volume, fade } => {
                if let Some(handles) = audio.active_sounds.get_mut(&entity) {
                    let tween = if fade > 0.0 {
                        Tween {
                            duration: std::time::Duration::from_secs_f32(fade),
                            ..Default::default()
                        }
                    } else {
                        Tween::default()
                    };
                    for handle in handles.iter_mut() {
                        let _ = handle.set_volume(amplitude_to_db(volume as f64), tween);
                    }
                }
            }

            AudioCommand::SetSoundPitch { entity, pitch, fade } => {
                if let Some(handles) = audio.active_sounds.get_mut(&entity) {
                    let tween = if fade > 0.0 {
                        Tween {
                            duration: std::time::Duration::from_secs_f32(fade),
                            ..Default::default()
                        }
                    } else {
                        Tween::default()
                    };
                    for handle in handles.iter_mut() {
                        let _ = handle.set_playback_rate(pitch as f64, tween);
                    }
                }
            }

            AudioCommand::CrossfadeMusic { path, volume, duration, bus } => {
                audio.stop_music(duration);

                let full_path = audio.resolve_path(&path);
                let effective_volume = (volume as f64 * audio.master_volume).clamp(0.0, 2.0);

                match StreamingSoundData::from_file(&full_path) {
                    Ok(data) => {
                        let data = data
                            .volume(amplitude_to_db(effective_volume))
                            .loop_region(0.0..)
                            .fade_in_tween(Tween {
                                duration: std::time::Duration::from_secs_f32(duration),
                                ..Default::default()
                            });

                        match audio.play_on_bus(data, &bus, &mixer) {
                            Ok(handle) => {
                                audio.music_handle = Some(handle);
                                info!("[KiraAudio] Crossfading to: {} on bus: {}", path, bus);
                            }
                            Err(e) => warn!("[KiraAudio] Crossfade failed {}: {}", path, e),
                        }
                    }
                    Err(e) => warn!("[KiraAudio] Failed to load music for crossfade {}: {}", path, e),
                }
            }
        }
    }
}

/// Sync the Kira listener position/orientation and all spatial track positions each frame.
pub fn sync_spatial_audio(
    audio: Option<NonSendMut<KiraAudioManager>>,
    listener_query: Query<(&AudioListener, &GlobalTransform)>,
    spatial_entities: Query<&GlobalTransform>,
) {
    let Some(mut audio) = audio else { return };

    // Update listener from the first active AudioListener entity
    if let Some(ref mut listener) = audio.listener {
        for (data, transform) in &listener_query {
            if data.active {
                let pos = transform.translation();
                let rot = transform.to_isometry().rotation;
                listener.set_position(vec3_to_mint(pos), Tween::default());
                listener.set_orientation(quat_to_mint(rot), Tween::default());
                break;
            }
        }
    }

    // Update emitter positions and clean up despawned entities
    let despawned: Vec<Entity> = audio.spatial_tracks.keys()
        .filter(|e| spatial_entities.get(**e).is_err())
        .copied()
        .collect();
    for entity in despawned {
        audio.spatial_tracks.remove(&entity);
    }

    for (entity, track) in audio.spatial_tracks.iter_mut() {
        if let Ok(transform) = spatial_entities.get(*entity) {
            track.set_position(vec3_to_mint(transform.translation()), Tween::default());
        }
    }
}

/// Prune finished sound handles every frame to avoid stale accumulation.
pub fn prune_finished_sounds(audio: Option<NonSendMut<KiraAudioManager>>) {
    let Some(mut audio) = audio else { return };
    audio.prune_finished();
}

/// Update VU meter peak levels from active sound handles.
/// Since Kira 0.12 doesn't expose per-track metering, we estimate activity
/// from the number of playing sounds per bus.
pub fn update_vu_meters(
    audio: Option<NonSendMut<KiraAudioManager>>,
    mut mixer: Option<ResMut<MixerState>>,
    preview: Option<Res<AudioPreviewState>>,
) {
    let Some(audio) = audio else { return };
    let Some(ref mut mixer) = mixer else { return };

    // Decay all peak levels toward zero
    const DECAY_RATE: f32 = 3.0; // per second approx (at 60fps: ~0.05 per frame)
    let decay = DECAY_RATE / 60.0;

    mixer.master.peak_level = (mixer.master.peak_level - decay).max(0.0);
    mixer.sfx.peak_level = (mixer.sfx.peak_level - decay).max(0.0);
    mixer.music.peak_level = (mixer.music.peak_level - decay).max(0.0);
    mixer.ambient.peak_level = (mixer.ambient.peak_level - decay).max(0.0);
    for (_, strip) in mixer.custom_buses.iter_mut() {
        strip.peak_level = (strip.peak_level - decay).max(0.0);
    }

    // Estimate activity from playing sounds (without emitter query, assume SFX bus)
    for (_entity, handles) in &audio.active_sounds {
        let playing_count = handles.iter()
            .filter(|h| h.state() == PlaybackState::Playing)
            .count();
        if playing_count == 0 { continue; }

        let level = 0.8_f32.min(1.5);
        mixer.sfx.peak_level = mixer.sfx.peak_level.max(level);
    }

    // Music handle - if music is actively playing, bump the Music bus meter
    if let Some(ref handle) = audio.music_handle {
        if handle.state() == PlaybackState::Playing {
            mixer.music.peak_level = mixer.music.peak_level.max(0.6);
        }
    }

    // Preview handle - bump the bus meter for the preview sound
    if let Some(ref preview) = preview {
        if let Some(ref handle) = preview.handle {
            if handle.state() == PlaybackState::Playing {
                let level = 0.7_f32;
                match preview.previewing_bus.as_deref().unwrap_or("Sfx") {
                    "Music" => mixer.music.peak_level = mixer.music.peak_level.max(level),
                    "Ambient" => mixer.ambient.peak_level = mixer.ambient.peak_level.max(level),
                    "Master" => mixer.master.peak_level = mixer.master.peak_level.max(level),
                    name => {
                        if let Some(idx) = mixer.custom_buses.iter().position(|(n, _)| n == name) {
                            mixer.custom_buses[idx].1.peak_level = mixer.custom_buses[idx].1.peak_level.max(level);
                        } else {
                            mixer.sfx.peak_level = mixer.sfx.peak_level.max(level);
                        }
                    }
                }
            }
        }
    }

    // Master reflects all activity
    let max_sub = mixer.sfx.peak_level
        .max(mixer.music.peak_level)
        .max(mixer.ambient.peak_level);
    mixer.master.peak_level = mixer.master.peak_level.max(max_sub);
}

/// Auto-stop preview when its sound handle finishes playing.
pub fn preview_audio_system(
    mut preview: Option<ResMut<AudioPreviewState>>,
) {
    let Some(ref mut preview) = preview else { return };

    // Clean up handle when sound finishes naturally
    if let Some(ref handle) = preview.handle {
        if handle.state() == PlaybackState::Stopped {
            preview.handle = None;
            preview.previewing_entity = None;
            preview.previewing_path = None;
            preview.previewing_bus = None;
        }
    }
}
