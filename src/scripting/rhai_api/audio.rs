//! Audio API functions for Rhai scripts

use rhai::{Engine, ImmutableString};
use super::super::rhai_commands::RhaiCommand;

/// Register audio functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Sound Effects
    // ===================

    // play_sound(path) - Play a one-shot sound effect
    engine.register_fn("play_sound", |path: ImmutableString| {
        super::push_command(RhaiCommand::PlaySound { path: path.to_string(), volume: 1.0, looping: false });
    });

    // play_sound_at_volume(path, volume) - Play sound with volume (0.0 to 1.0)
    engine.register_fn("play_sound_at_volume", |path: ImmutableString, volume: f64| {
        super::push_command(RhaiCommand::PlaySound { path: path.to_string(), volume: volume as f32, looping: false });
    });

    // play_sound_looping(path, volume) - Play looping sound
    engine.register_fn("play_sound_looping", |path: ImmutableString, volume: f64| {
        super::push_command(RhaiCommand::PlaySound { path: path.to_string(), volume: volume as f32, looping: true });
    });

    // ===================
    // 3D Spatial Audio
    // ===================

    // play_sound_3d(path, x, y, z) - Play sound at position
    engine.register_fn("play_sound_3d", |path: ImmutableString, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::PlaySound3D { path: path.to_string(), volume: 1.0, position: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });

    // play_sound_3d_at_volume(path, volume, x, y, z)
    engine.register_fn("play_sound_3d_at_volume", |path: ImmutableString, volume: f64, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::PlaySound3D { path: path.to_string(), volume: volume as f32, position: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });

    // ===================
    // Music
    // ===================

    // play_music(path) - Play background music
    engine.register_fn("play_music", |path: ImmutableString| {
        super::push_command(RhaiCommand::PlayMusic { path: path.to_string(), volume: 1.0, fade_in: 0.0 });
    });

    // play_music_with_fade(path, volume, fade_in_seconds)
    engine.register_fn("play_music_with_fade", |path: ImmutableString, volume: f64, fade_in: f64| {
        super::push_command(RhaiCommand::PlayMusic { path: path.to_string(), volume: volume as f32, fade_in: fade_in as f32 });
    });

    // stop_music() - Stop music immediately
    engine.register_fn("stop_music", || {
        super::push_command(RhaiCommand::StopMusic { fade_out: 0.0 });
    });

    // stop_music_with_fade(fade_out_seconds)
    engine.register_fn("stop_music_with_fade", |fade_out: f64| {
        super::push_command(RhaiCommand::StopMusic { fade_out: fade_out as f32 });
    });

    // ===================
    // Volume Control
    // ===================

    // set_master_volume(volume) - Set global volume (0.0 to 1.0)
    engine.register_fn("set_master_volume", |volume: f64| {
        super::push_command(RhaiCommand::SetMasterVolume { volume: volume as f32 });
    });

    // stop_all_sounds()
    engine.register_fn("stop_all_sounds", || {
        super::push_command(RhaiCommand::StopAllSounds);
    });
}
