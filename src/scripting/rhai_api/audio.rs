//! Audio API functions for Rhai scripts

use rhai::{Dynamic, Engine, Map, ImmutableString};

/// Register audio functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Sound Effects
    // ===================

    // play_sound(path) - Play a one-shot sound effect
    engine.register_fn("play_sound", |path: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("play_sound"));
        m.insert("path".into(), Dynamic::from(path));
        m.insert("volume".into(), Dynamic::from(1.0));
        m.insert("looping".into(), Dynamic::from(false));
        m
    });

    // play_sound_at_volume(path, volume) - Play sound with volume (0.0 to 1.0)
    engine.register_fn("play_sound_at_volume", |path: ImmutableString, volume: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("play_sound"));
        m.insert("path".into(), Dynamic::from(path));
        m.insert("volume".into(), Dynamic::from(volume));
        m.insert("looping".into(), Dynamic::from(false));
        m
    });

    // play_sound_looping(path, volume) - Play looping sound
    engine.register_fn("play_sound_looping", |path: ImmutableString, volume: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("play_sound"));
        m.insert("path".into(), Dynamic::from(path));
        m.insert("volume".into(), Dynamic::from(volume));
        m.insert("looping".into(), Dynamic::from(true));
        m
    });

    // ===================
    // 3D Spatial Audio
    // ===================

    // play_sound_3d(path, x, y, z) - Play sound at position
    engine.register_fn("play_sound_3d", |path: ImmutableString, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("play_sound_3d"));
        m.insert("path".into(), Dynamic::from(path));
        m.insert("volume".into(), Dynamic::from(1.0));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // play_sound_3d_at_volume(path, volume, x, y, z)
    engine.register_fn("play_sound_3d_at_volume", |path: ImmutableString, volume: f64, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("play_sound_3d"));
        m.insert("path".into(), Dynamic::from(path));
        m.insert("volume".into(), Dynamic::from(volume));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // ===================
    // Music
    // ===================

    // play_music(path) - Play background music
    engine.register_fn("play_music", |path: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("play_music"));
        m.insert("path".into(), Dynamic::from(path));
        m.insert("volume".into(), Dynamic::from(1.0));
        m.insert("fade_in".into(), Dynamic::from(0.0));
        m
    });

    // play_music_with_fade(path, volume, fade_in_seconds)
    engine.register_fn("play_music_with_fade", |path: ImmutableString, volume: f64, fade_in: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("play_music"));
        m.insert("path".into(), Dynamic::from(path));
        m.insert("volume".into(), Dynamic::from(volume));
        m.insert("fade_in".into(), Dynamic::from(fade_in));
        m
    });

    // stop_music() - Stop music immediately
    engine.register_fn("stop_music", || -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("stop_music"));
        m.insert("fade_out".into(), Dynamic::from(0.0));
        m
    });

    // stop_music_with_fade(fade_out_seconds)
    engine.register_fn("stop_music_with_fade", |fade_out: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("stop_music"));
        m.insert("fade_out".into(), Dynamic::from(fade_out));
        m
    });

    // ===================
    // Volume Control
    // ===================

    // set_master_volume(volume) - Set global volume (0.0 to 1.0)
    engine.register_fn("set_master_volume", |volume: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_master_volume"));
        m.insert("volume".into(), Dynamic::from(volume));
        m
    });

    // stop_all_sounds()
    engine.register_fn("stop_all_sounds", || -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("stop_all_sounds"));
        m
    });
}
