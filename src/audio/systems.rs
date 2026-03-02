//! Kira audio systems
//!
//! Replaces Bevy's AudioPlayer-based systems with Kira handle-based systems.

use bevy::prelude::*;
use kira::{
    sound::static_sound::StaticSoundData,
    sound::streaming::StreamingSoundData,
    sound::{Region, EndPosition, PlaybackPosition},
    Panning, Tween,
};
use crate::component_system::components::audio_emitter::{AudioEmitterData, RolloffType};
use crate::component_system::components::audio_listener::AudioListenerData;

use crate::core::PlayModeState;
use crate::scripting::resources::{AudioCommand, AudioCommandQueue};
use crate::audio::mixer::MixerState;
use super::manager::{KiraAudioManager, amplitude_to_db, vec3_to_mint, quat_to_mint};
use super::preview::AudioPreviewState;

/// Apply AudioEmitterData properties (pitch, panning, fade_in, loop region) to sound data.
fn apply_emitter_settings(data: StaticSoundData, emitter: &AudioEmitterData) -> StaticSoundData {
    let data = if (emitter.pitch - 1.0).abs() > 0.001 {
        data.playback_rate(emitter.pitch as f64)
    } else {
        data
    };
    let data = if emitter.panning.abs() > 0.001 {
        data.panning(Panning(emitter.panning))
    } else {
        data
    };
    let data = if emitter.looping {
        if emitter.loop_end > 0.0 && emitter.loop_end > emitter.loop_start {
            data.loop_region(Region {
                start: PlaybackPosition::Seconds(emitter.loop_start),
                end: EndPosition::Custom(PlaybackPosition::Seconds(emitter.loop_end)),
            })
        } else if emitter.loop_start > 0.0 {
            data.loop_region(emitter.loop_start..)
        } else {
            data.loop_region(0.0..)
        }
    } else {
        data
    };
    let data = if emitter.fade_in > 0.0 {
        data.fade_in_tween(Tween {
            duration: std::time::Duration::from_secs_f32(emitter.fade_in),
            ..Default::default()
        })
    } else {
        data
    };
    data
}

/// Process queued audio commands from scripts using Kira
pub fn process_kira_commands(
    mut queue: ResMut<AudioCommandQueue>,
    audio: Option<NonSendMut<KiraAudioManager>>,
    mixer: Option<Res<MixerState>>,
    transforms: Query<&GlobalTransform>,
    emitter_query: Query<&AudioEmitterData>,
) {
    let Some(mut audio) = audio else { return };
    let Some(mixer) = mixer else { return };
    if queue.is_empty() {
        return;
    }

    for cmd in queue.drain() {
        match cmd {
            AudioCommand::PlaySound { path, volume, looping, bus, entity } => {
                let full_path = audio.resolve_path(&path);
                let effective_volume = (volume as f64 * audio.master_volume).clamp(0.0, 2.0);

                // Grab emitter send levels before borrowing audio mutably
                let (reverb_send, delay_send) = entity
                    .and_then(|e| emitter_query.get(e).ok())
                    .map(|em| (em.reverb_send, em.delay_send))
                    .unwrap_or((0.0, 0.0));

                match StaticSoundData::from_file(&full_path) {
                    Ok(data) => {
                        let data = data.volume(amplitude_to_db(effective_volume));
                        // Apply emitter settings if entity has AudioEmitterData, else just loop
                        let data = if let Some(emitter) = entity.and_then(|e| emitter_query.get(e).ok()) {
                            apply_emitter_settings(data, emitter)
                        } else if looping {
                            data.loop_region(0.0..)
                        } else {
                            data
                        };

                        // Route through emitter send track if reverb/delay > 0
                        let play_result = if let Some(ent) = entity {
                            if let Some(send_track) = audio.get_or_create_emitter_send_track(ent, &bus, reverb_send, delay_send, &mixer) {
                                send_track.play(data)
                            } else {
                                audio.play_on_bus(data, &bus, &mixer)
                            }
                        } else {
                            audio.play_on_bus(data, &bus, &mixer)
                        };

                        match play_result {
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

                // Get spatial params from emitter component if available, else use defaults
                let (min_dist, max_dist, rolloff) = entity
                    .and_then(|ent| emitter_query.get(ent).ok())
                    .map(|e| (e.spatial_min_distance, e.spatial_max_distance, e.spatial_rolloff.clone()))
                    .unwrap_or((1.0, 50.0, RolloffType::Logarithmic));

                // Use entity's GlobalTransform if available, else use the command's position
                let emitter_pos = entity
                    .and_then(|ent| transforms.get(ent).ok())
                    .map(|t| t.translation())
                    .unwrap_or(position);

                match StaticSoundData::from_file(&full_path) {
                    Ok(data) => {
                        let data = data.volume(amplitude_to_db(effective_volume));
                        // Apply emitter settings (pitch, panning, fade_in, loop region) if available
                        let data = if let Some(emitter) = entity.and_then(|e| emitter_query.get(e).ok()) {
                            apply_emitter_settings(data, emitter)
                        } else {
                            data
                        };

                        if let Some(ent) = entity {
                            if let Some(spatial_track) = audio.get_or_create_spatial_track(
                                ent, emitter_pos, &bus, min_dist, max_dist, &rolloff, &mixer,
                            ) {
                                match spatial_track.play(data) {
                                    Ok(handle) => {
                                        audio.track_sound(ent, handle);
                                        debug!("[KiraAudio] Playing 3D sound: {} at {:?}", path, emitter_pos);
                                    }
                                    Err(e) => warn!("[KiraAudio] Failed to play 3D sound {}: {}", path, e),
                                }
                            }
                        } else {
                            // No entity — fallback to non-spatial playback
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
    listener_query: Query<(&AudioListenerData, &GlobalTransform)>,
    spatial_entities: Query<&GlobalTransform>,
) {
    let Some(mut audio) = audio else { return };

    // Update listener from the first active AudioListenerData entity
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

/// Cleanup all Kira sounds when exiting play mode
pub fn cleanup_kira_on_stop(
    audio: Option<NonSendMut<KiraAudioManager>>,
    play_mode: Res<PlayModeState>,
    mut last_playing: Local<bool>,
) {
    let Some(mut audio) = audio else { return };
    let currently_playing = play_mode.is_in_play_mode();

    if *last_playing && !currently_playing {
        audio.stop_all_sounds();
        audio.stop_music(0.0);
        audio.master_volume = 1.0;
        info!("[KiraAudio] Cleaned up after exiting play mode");
    }

    *last_playing = currently_playing;
}

/// Stop sounds when an AudioEmitterData component is removed or its entity is despawned.
pub fn cleanup_removed_emitters(
    audio: Option<NonSendMut<KiraAudioManager>>,
    mut preview: Option<ResMut<crate::audio::AudioPreviewState>>,
    mut removed: RemovedComponents<AudioEmitterData>,
) {
    let Some(mut audio) = audio else { return };
    for entity in removed.read() {
        audio.stop_entity_sounds(entity);
        // Also stop preview if it was playing on this entity
        if let Some(ref mut preview) = preview {
            if preview.previewing_entity == Some(entity) {
                preview.stop();
            }
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
/// from the number of playing sounds per bus and their emitter volumes.
pub fn update_vu_meters(
    audio: Option<NonSendMut<KiraAudioManager>>,
    mut mixer: Option<ResMut<MixerState>>,
    emitters: Query<&AudioEmitterData>,
    preview: Option<Res<crate::audio::AudioPreviewState>>,
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

    // Estimate activity from playing sounds
    for (entity, handles) in &audio.active_sounds {
        let playing_count = handles.iter()
            .filter(|h| h.state() == kira::sound::PlaybackState::Playing)
            .count();
        if playing_count == 0 { continue; }

        let (bus_name, vol) = if let Ok(emitter) = emitters.get(*entity) {
            (emitter.bus.as_str(), emitter.volume)
        } else {
            ("Sfx", 1.0)
        };

        let level = (vol * 0.8).min(1.5);

        match bus_name {
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

    // Music handle — if music is actively playing, bump the Music bus meter
    if let Some(ref handle) = audio.music_handle {
        if handle.state() == kira::sound::PlaybackState::Playing {
            mixer.music.peak_level = mixer.music.peak_level.max(0.6);
        }
    }

    // Preview handle — bump the bus meter for the preview sound
    if let Some(ref preview) = preview {
        if let Some(ref handle) = preview.handle {
            if handle.state() == kira::sound::PlaybackState::Playing {
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

/// Autoplay audio emitters when entering play mode.
pub fn autoplay_audio_emitters(
    audio: Option<NonSendMut<KiraAudioManager>>,
    mixer: Option<Res<MixerState>>,
    play_mode: Res<PlayModeState>,
    mut last_playing: Local<bool>,
    emitters: Query<(Entity, &AudioEmitterData, &GlobalTransform)>,
) {
    let Some(mut audio) = audio else { return };
    let Some(mixer) = mixer else { return };
    let currently_playing = play_mode.is_in_play_mode();

    // Trigger on transition from editing → play mode
    if currently_playing && !*last_playing {
        for (entity, emitter, transform) in &emitters {
            if emitter.autoplay && !emitter.clip.is_empty() {
                let full_path = audio.resolve_path(&emitter.clip);
                let effective_volume = (emitter.volume as f64 * audio.master_volume).clamp(0.0, 2.0);

                match StaticSoundData::from_file(&full_path) {
                    Ok(data) => {
                        let data = data.volume(amplitude_to_db(effective_volume));
                        let data = apply_emitter_settings(data, emitter);

                        if emitter.spatial {
                            // Spatial: play on a spatial track
                            let pos = transform.translation();
                            if let Some(spatial_track) = audio.get_or_create_spatial_track(
                                entity, pos, &emitter.bus,
                                emitter.spatial_min_distance, emitter.spatial_max_distance,
                                &emitter.spatial_rolloff, &mixer,
                            ) {
                                match spatial_track.play(data) {
                                    Ok(handle) => {
                                        audio.track_sound(entity, handle);
                                        debug!("[KiraAudio] Autoplay spatial: {} on entity {:?}", emitter.clip, entity);
                                    }
                                    Err(e) => warn!("[KiraAudio] Autoplay spatial failed for {}: {}", emitter.clip, e),
                                }
                            }
                        } else {
                            // Non-spatial: route through send track if reverb/delay > 0
                            let play_result = if let Some(send_track) = audio.get_or_create_emitter_send_track(
                                entity, &emitter.bus, emitter.reverb_send, emitter.delay_send, &mixer,
                            ) {
                                send_track.play(data)
                            } else {
                                audio.play_on_bus(data, &emitter.bus, &mixer)
                            };
                            match play_result {
                                Ok(handle) => {
                                    audio.track_sound(entity, handle);
                                    debug!("[KiraAudio] Autoplay: {} on entity {:?}", emitter.clip, entity);
                                }
                                Err(e) => warn!("[KiraAudio] Autoplay failed for {}: {}", emitter.clip, e),
                            }
                        }
                    }
                    Err(e) => warn!("[KiraAudio] Autoplay load failed for {}: {}", emitter.clip, e),
                }
            }
        }
    }

    *last_playing = currently_playing;
}

/// Auto-stop preview when its sound handle finishes playing.
pub fn preview_audio_system(
    mut preview: Option<ResMut<AudioPreviewState>>,
    selection: Option<Res<crate::core::SelectionState>>,
) {
    let Some(ref mut preview) = preview else { return };

    // Stop preview when selection changes away from the previewing entity
    if let Some(ref selection) = selection {
        if selection.is_changed() {
            if let Some(previewing) = preview.previewing_entity {
                if selection.selected_entity != Some(previewing) {
                    preview.stop();
                    return;
                }
            }
        }
    }

    // Clean up handle when sound finishes naturally
    if let Some(ref handle) = preview.handle {
        if handle.state() == kira::sound::PlaybackState::Stopped {
            preview.handle = None;
            preview.previewing_entity = None;
            preview.previewing_path = None;
            preview.previewing_bus = None;
        }
    }
}
