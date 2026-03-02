//! Audio API functions for Rhai scripts

use rhai::{Engine, ImmutableString};
use super::super::rhai_commands::RhaiCommand;

/// Register audio functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Sound Effects
    // ===================

    engine.register_fn("play_sound", |path: ImmutableString| {
        super::push_command(RhaiCommand::PlaySound { path: path.to_string(), volume: 1.0, looping: false, bus: "Sfx".to_string() });
    });

    engine.register_fn("play_sound", |path: ImmutableString, bus: ImmutableString| {
        super::push_command(RhaiCommand::PlaySound { path: path.to_string(), volume: 1.0, looping: false, bus: bus.to_string() });
    });

    engine.register_fn("play_sound_at_volume", |path: ImmutableString, volume: f64| {
        super::push_command(RhaiCommand::PlaySound { path: path.to_string(), volume: volume as f32, looping: false, bus: "Sfx".to_string() });
    });

    engine.register_fn("play_sound_at_volume", |path: ImmutableString, volume: f64, bus: ImmutableString| {
        super::push_command(RhaiCommand::PlaySound { path: path.to_string(), volume: volume as f32, looping: false, bus: bus.to_string() });
    });

    engine.register_fn("play_sound_looping", |path: ImmutableString, volume: f64| {
        super::push_command(RhaiCommand::PlaySound { path: path.to_string(), volume: volume as f32, looping: true, bus: "Sfx".to_string() });
    });

    engine.register_fn("play_sound_looping", |path: ImmutableString, volume: f64, bus: ImmutableString| {
        super::push_command(RhaiCommand::PlaySound { path: path.to_string(), volume: volume as f32, looping: true, bus: bus.to_string() });
    });

    // ===================
    // 3D Spatial Audio
    // ===================

    engine.register_fn("play_sound_3d", |path: ImmutableString, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::PlaySound3D {
            path: path.to_string(),
            volume: 1.0,
            position: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32),
            bus: "Sfx".to_string(),
        });
    });

    engine.register_fn("play_sound_3d", |path: ImmutableString, x: f64, y: f64, z: f64, bus: ImmutableString| {
        super::push_command(RhaiCommand::PlaySound3D {
            path: path.to_string(),
            volume: 1.0,
            position: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32),
            bus: bus.to_string(),
        });
    });

    engine.register_fn("play_sound_3d_at_volume", |path: ImmutableString, volume: f64, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::PlaySound3D {
            path: path.to_string(),
            volume: volume as f32,
            position: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32),
            bus: "Sfx".to_string(),
        });
    });

    engine.register_fn("play_sound_3d_at_volume", |path: ImmutableString, volume: f64, x: f64, y: f64, z: f64, bus: ImmutableString| {
        super::push_command(RhaiCommand::PlaySound3D {
            path: path.to_string(),
            volume: volume as f32,
            position: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32),
            bus: bus.to_string(),
        });
    });

    // ===================
    // Music
    // ===================

    engine.register_fn("play_music", |path: ImmutableString| {
        super::push_command(RhaiCommand::PlayMusic { path: path.to_string(), volume: 1.0, fade_in: 0.0, bus: "Music".to_string() });
    });

    engine.register_fn("play_music", |path: ImmutableString, bus: ImmutableString| {
        super::push_command(RhaiCommand::PlayMusic { path: path.to_string(), volume: 1.0, fade_in: 0.0, bus: bus.to_string() });
    });

    engine.register_fn("play_music_with_fade", |path: ImmutableString, volume: f64, fade_in: f64| {
        super::push_command(RhaiCommand::PlayMusic { path: path.to_string(), volume: volume as f32, fade_in: fade_in as f32, bus: "Music".to_string() });
    });

    engine.register_fn("play_music_with_fade", |path: ImmutableString, volume: f64, fade_in: f64, bus: ImmutableString| {
        super::push_command(RhaiCommand::PlayMusic { path: path.to_string(), volume: volume as f32, fade_in: fade_in as f32, bus: bus.to_string() });
    });

    engine.register_fn("stop_music", || {
        super::push_command(RhaiCommand::StopMusic { fade_out: 0.0 });
    });

    engine.register_fn("stop_music_with_fade", |fade_out: f64| {
        super::push_command(RhaiCommand::StopMusic { fade_out: fade_out as f32 });
    });

    // crossfade_music(path, volume, duration)
    engine.register_fn("crossfade_music", |path: ImmutableString, volume: f64, duration: f64| {
        super::push_command(RhaiCommand::CrossfadeMusic {
            path: path.to_string(),
            volume: volume as f32,
            duration: duration as f32,
            bus: "Music".to_string(),
        });
    });

    engine.register_fn("crossfade_music", |path: ImmutableString, volume: f64, duration: f64, bus: ImmutableString| {
        super::push_command(RhaiCommand::CrossfadeMusic {
            path: path.to_string(),
            volume: volume as f32,
            duration: duration as f32,
            bus: bus.to_string(),
        });
    });

    // ===================
    // Volume / Playback Control
    // ===================

    engine.register_fn("set_master_volume", |volume: f64| {
        super::push_command(RhaiCommand::SetMasterVolume { volume: volume as f32 });
    });

    engine.register_fn("stop_all_sounds", || {
        super::push_command(RhaiCommand::StopAllSounds);
    });

    engine.register_fn("pause_sound", || {
        super::push_command(RhaiCommand::PauseSound);
    });

    engine.register_fn("pause_sound", |entity_id: i64| {
        super::push_command(RhaiCommand::PauseSoundEntity { entity_id: entity_id as u64 });
    });

    engine.register_fn("resume_sound", || {
        super::push_command(RhaiCommand::ResumeSound);
    });

    engine.register_fn("resume_sound", |entity_id: i64| {
        super::push_command(RhaiCommand::ResumeSoundEntity { entity_id: entity_id as u64 });
    });

    // set_sound_volume(volume, fade_seconds)
    engine.register_fn("set_sound_volume", |volume: f64, fade: f64| {
        super::push_command(RhaiCommand::SetSoundVolume { volume: volume as f32, fade: fade as f32 });
    });

    // set_sound_volume(entity_id, volume, fade_seconds)
    engine.register_fn("set_sound_volume", |entity_id: i64, volume: f64, fade: f64| {
        super::push_command(RhaiCommand::SetSoundVolumeEntity { entity_id: entity_id as u64, volume: volume as f32, fade: fade as f32 });
    });

    // set_sound_pitch(pitch, fade_seconds)
    engine.register_fn("set_sound_pitch", |pitch: f64, fade: f64| {
        super::push_command(RhaiCommand::SetSoundPitch { pitch: pitch as f32, fade: fade as f32 });
    });

    // set_sound_pitch(entity_id, pitch, fade_seconds)
    engine.register_fn("set_sound_pitch", |entity_id: i64, pitch: f64, fade: f64| {
        super::push_command(RhaiCommand::SetSoundPitchEntity { entity_id: entity_id as u64, pitch: pitch as f32, fade: fade as f32 });
    });

    // ===================
    // Sound State Queries
    // ===================

    // is_sound_playing(entity_id) — check if an entity has active sounds
    // Use: is_sound_playing(self_entity_id) for self, or is_sound_playing(other_id) for others
    engine.register_fn("is_sound_playing", |entity_id: i64| -> bool {
        super::is_entity_sound_playing(entity_id as u64)
    });
}
