//! Audio command processing system
//!
//! Processes queued audio commands from scripts using Bevy's audio system.

use bevy::prelude::*;
use bevy::audio::{AudioSink, Volume};
use crate::core::PlayModeState;
use crate::scripting::resources::{
    AudioCommand, AudioCommandQueue, AudioState, AudioFade, MusicTrack, SoundEffect,
};

/// System to process queued audio commands
pub fn process_audio_commands(
    mut commands: Commands,
    mut queue: ResMut<AudioCommandQueue>,
    mut audio_state: ResMut<AudioState>,
    asset_server: Res<AssetServer>,
    music_query: Query<Entity, With<MusicTrack>>,
    sounds_query: Query<Entity, With<SoundEffect>>,
) {
    if queue.is_empty() {
        return;
    }

    for cmd in queue.drain() {
        match cmd {
            AudioCommand::PlaySound { path, volume, looping } => {
                // Load and play the sound
                let source: Handle<AudioSource> = asset_server.load(&path);
                let effective_volume = audio_state.effective_volume(volume);

                let settings = PlaybackSettings {
                    mode: if looping {
                        bevy::audio::PlaybackMode::Loop
                    } else {
                        bevy::audio::PlaybackMode::Despawn
                    },
                    volume: bevy::audio::Volume::Linear(effective_volume),
                    ..default()
                };

                let entity = commands.spawn((
                    AudioPlayer::new(source),
                    settings,
                    SoundEffect,
                )).id();

                audio_state.active_sounds.insert(entity, path);
            }

            AudioCommand::PlaySound3D { path, volume, position } => {
                // Load and play the sound (spatial audio - position tracked but basic playback)
                let source: Handle<AudioSource> = asset_server.load(&path);
                let effective_volume = audio_state.effective_volume(volume);

                let settings = PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Despawn,
                    volume: bevy::audio::Volume::Linear(effective_volume),
                    ..default()
                };

                // For now, just play the sound. Full 3D audio would require additional setup
                // with listener/emitter components from bevy_spatial_audio or similar
                let entity = commands.spawn((
                    AudioPlayer::new(source),
                    settings,
                    SoundEffect,
                    Transform::from_translation(position),
                )).id();

                debug!("Playing 3D sound '{}' at {:?}", path, position);
                audio_state.active_sounds.insert(entity, path);
            }

            AudioCommand::PlayMusic { path, volume, fade_in } => {
                // Stop current music first (with immediate despawn if fading in new music)
                if let Some(current) = audio_state.current_music.take() {
                    commands.entity(current).despawn();
                }
                for entity in music_query.iter() {
                    commands.entity(entity).despawn();
                }

                // Load and play new music
                let source: Handle<AudioSource> = asset_server.load(&path);
                let effective_volume = audio_state.effective_volume(volume);

                // Determine starting volume based on fade_in
                let (start_volume, needs_fade) = if fade_in > 0.0 {
                    (0.0, true)
                } else {
                    (effective_volume, false)
                };

                let settings = PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Loop,
                    volume: bevy::audio::Volume::Linear(start_volume),
                    ..default()
                };

                let mut entity_commands = commands.spawn((
                    AudioPlayer::new(source),
                    settings,
                    MusicTrack,
                ));

                // Add fade component if needed
                if needs_fade {
                    entity_commands.insert(AudioFade::fade_in(effective_volume, fade_in));
                    info!("Playing music: {} (fading in over {}s)", path, fade_in);
                } else {
                    info!("Playing music: {}", path);
                }

                audio_state.current_music = Some(entity_commands.id());
            }

            AudioCommand::StopMusic { fade_out } => {
                if fade_out > 0.0 {
                    // Add fade-out component to current music
                    if let Some(current) = audio_state.current_music {
                        // Add fade component - the update_audio_fades system will handle despawn
                        commands.entity(current).insert(AudioFade::fade_out(1.0, fade_out));
                        info!("Music fading out over {}s", fade_out);
                    }
                    // Clear the current music reference (it will be despawned by fade system)
                    audio_state.current_music = None;
                } else {
                    // Immediate stop
                    if let Some(current) = audio_state.current_music.take() {
                        commands.entity(current).despawn();
                    }
                    for entity in music_query.iter() {
                        commands.entity(entity).despawn();
                    }
                    info!("Music stopped");
                }
            }

            AudioCommand::StopAllSounds => {
                // Stop all sound effects
                for entity in sounds_query.iter() {
                    commands.entity(entity).despawn();
                }
                audio_state.active_sounds.clear();

                // Also stop music
                if let Some(current) = audio_state.current_music.take() {
                    commands.entity(current).despawn();
                }
                for entity in music_query.iter() {
                    commands.entity(entity).despawn();
                }
                info!("All sounds stopped");
            }

            AudioCommand::SetMasterVolume { volume } => {
                audio_state.master_volume = volume.clamp(0.0, 1.0);
                // Note: Changing volume of already playing sounds would require
                // querying and updating each AudioSink. For simplicity, this only
                // affects new sounds.
                debug!("Master volume set to {}", audio_state.master_volume);
            }
        }
    }
}

/// System to clean up audio when exiting play mode
pub fn cleanup_audio_on_stop(
    mut commands: Commands,
    play_mode: Res<PlayModeState>,
    mut audio_state: ResMut<AudioState>,
    music_query: Query<Entity, With<MusicTrack>>,
    sounds_query: Query<Entity, With<SoundEffect>>,
    mut last_playing: Local<bool>,
) {
    let currently_playing = play_mode.is_in_play_mode();

    // Detect transition from playing to editing
    if *last_playing && !currently_playing {
        // Stop all audio
        for entity in sounds_query.iter() {
            commands.entity(entity).despawn();
        }
        for entity in music_query.iter() {
            commands.entity(entity).despawn();
        }

        audio_state.current_music = None;
        audio_state.active_sounds.clear();
        // Reset master volume to default
        audio_state.master_volume = 1.0;

        info!("Audio cleaned up after exiting play mode");
    }

    *last_playing = currently_playing;
}

/// System to update audio fade effects
pub fn update_audio_fades(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut AudioFade, &mut AudioSink)>,
) {
    let delta = time.delta_secs();

    for (entity, mut fade, mut sink) in query.iter_mut() {
        // Advance the fade timer
        fade.tick(delta);

        // Calculate and apply the current volume
        let volume = fade.current_volume();
        sink.set_volume(Volume::Linear(volume));

        // Check if fade is complete
        if fade.is_complete() {
            if fade.despawn_on_complete {
                // Fade out complete - despawn the entity
                commands.entity(entity).despawn();
                debug!("Audio fade-out complete, despawning entity {:?}", entity);
            } else {
                // Fade in complete - remove the fade component
                commands.entity(entity).remove::<AudioFade>();
                debug!("Audio fade-in complete on entity {:?}", entity);
            }
        }
    }
}
