//! Kira-based audio system for Bevy 0.18
//!
//! On native platforms: full Kira audio with spatial audio, mixer, streaming.
//! On WASM: no-op stub (Kira's cpal backend doesn't compile for wasm).

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
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
    }
}

use bevy::prelude::*;

/// Bevy plugin that initializes the Kira audio system.
pub struct KiraPlugin;

impl Plugin for KiraPlugin {
    fn build(&self, _app: &mut App) {
        info!("[runtime] KiraPlugin");

        #[cfg(not(target_arch = "wasm32"))]
        {
            use self::{manager::KiraAudioManager, preview::AudioPreviewState,
                       commands::AudioCommandQueue, mixer, systems, systems::AudioSet};

            _app.insert_non_send_resource(KiraAudioManager::new())
                .insert_resource(AudioPreviewState::default())
                .insert_resource(mixer::MixerState::default())
                .insert_resource(AudioCommandQueue::default())
                .configure_sets(
                    Update,
                    (AudioSet::Commands, AudioSet::Sync, AudioSet::Cleanup).chain(),
                )
                .add_systems(
                    Update,
                    systems::process_audio_commands.in_set(AudioSet::Commands),
                )
                .add_systems(
                    Update,
                    systems::sync_spatial_audio.in_set(AudioSet::Sync),
                )
                .add_systems(Update, mixer::sync_mixer_to_kira)
                .add_systems(Update, systems::preview_audio_system)
                .add_systems(
                    Update,
                    systems::prune_finished_sounds.in_set(AudioSet::Cleanup),
                )
                .add_systems(Update, systems::update_vu_meters);
        }

        #[cfg(target_arch = "wasm32")]
        info!("[runtime] Audio disabled on WASM");
    }
}
