//! Kira-based audio system for Bevy 0.18
//!
//! Provides full control over audio playback via the kira crate:
//! - Mixer tracks (sfx, music, ambient, custom)
//! - Per-instance handle control (pause, resume, volume, pitch tweens)
//! - Audio preview (plays outside of play mode)
//! - Command queue for decoupled audio control
//! - Spatial audio with 3D listener and emitter positioning

pub mod commands;
pub mod components;
pub mod manager;
pub mod mixer;
pub mod preview;
pub mod systems;

pub use commands::{AudioCommand, AudioCommandQueue};
pub use components::AudioPlayer;
pub use manager::{amplitude_to_db, quat_to_mint, vec3_to_mint, KiraAudioManager, RolloffType};
pub use mixer::{ChannelStrip, MixerState};
pub use preview::AudioPreviewState;
pub use systems::{AudioListener, AudioSet};

use bevy::prelude::*;

/// Bevy plugin that initializes the Kira audio system.
pub struct KiraPlugin;

impl Plugin for KiraPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] KiraPlugin");
        app.insert_non_send_resource(KiraAudioManager::new())
            .insert_resource(AudioPreviewState::default())
            .insert_resource(MixerState::default())
            .insert_resource(AudioCommandQueue::default())
            .configure_sets(
                Update,
                (AudioSet::Commands, AudioSet::Sync, AudioSet::Cleanup).chain(),
            )
            // Command processing
            .add_systems(
                Update,
                systems::process_audio_commands.in_set(AudioSet::Commands),
            )
            // Spatial audio sync
            .add_systems(
                Update,
                systems::sync_spatial_audio.in_set(AudioSet::Sync),
            )
            // Sync mixer UI state to Kira track handles
            .add_systems(Update, mixer::sync_mixer_to_kira)
            // Preview runs unconditionally (outside play mode too)
            .add_systems(Update, systems::preview_audio_system)
            // Prune finished sound handles
            .add_systems(
                Update,
                systems::prune_finished_sounds.in_set(AudioSet::Cleanup),
            )
            // VU meter peak level estimation
            .add_systems(Update, systems::update_vu_meters);
    }
}
