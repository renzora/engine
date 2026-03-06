//! Audio components — data attached to entities for audio playback.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::manager::RolloffType;

/// Audio emitter component with full playback parameters.
///
/// Attach to an entity to make it a sound source. The audio system reads these
/// fields to configure Kira playback (volume, pitch, spatial positioning, etc.).
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct AudioPlayerData {
    /// Relative path to the audio clip asset.
    pub clip: String,
    /// Volume multiplier (0.0–2.0, 1.0 = unity).
    pub volume: f32,
    /// Playback speed multiplier (1.0 = normal).
    pub pitch: f32,
    /// Stereo panning (-1.0 left, 0.0 center, 1.0 right).
    pub panning: f32,
    /// Whether the clip loops.
    pub looping: bool,
    /// Loop region start in seconds (0.0 = from beginning).
    pub loop_start: f64,
    /// Loop region end in seconds (0.0 = until end).
    pub loop_end: f64,
    /// Automatically play when entering play mode.
    pub autoplay: bool,
    /// Fade-in duration in seconds.
    pub fade_in: f32,
    /// Which mixer bus to route to ("Sfx", "Music", "Ambient", or custom).
    pub bus: String,
    /// Enable 3D spatial audio.
    pub spatial: bool,
    /// Minimum distance for spatial attenuation.
    pub spatial_min_distance: f32,
    /// Maximum distance for spatial attenuation.
    pub spatial_max_distance: f32,
    /// Distance rolloff curve.
    pub spatial_rolloff: RolloffType,
    /// Send level to the global reverb bus (0.0–1.0).
    pub reverb_send: f32,
    /// Send level to the global delay bus (0.0–1.0).
    pub delay_send: f32,
}

impl Default for AudioPlayerData {
    fn default() -> Self {
        Self {
            clip: String::new(),
            volume: 1.0,
            pitch: 1.0,
            panning: 0.0,
            looping: false,
            loop_start: 0.0,
            loop_end: 0.0,
            autoplay: false,
            fade_in: 0.0,
            bus: "Sfx".to_string(),
            spatial: false,
            spatial_min_distance: 1.0,
            spatial_max_distance: 50.0,
            spatial_rolloff: RolloffType::Logarithmic,
            reverb_send: 0.0,
            delay_send: 0.0,
        }
    }
}
