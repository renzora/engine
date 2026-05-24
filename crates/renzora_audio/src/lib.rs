//! Kira-based audio system for Bevy 0.18
//!
//! On native platforms: full Kira audio with spatial audio, mixer, streaming.
//! On WASM: no-op stub (Kira's cpal backend doesn't compile for wasm).

// `fx_bridge` is the only module here that compiles on WASM too — it's
// pure data types with no audio backend. Other UI crates depend on these
// types regardless of platform, so keep them outside the cfg_if gate.
pub mod fx_bridge;

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        pub mod asset_loader;
        pub mod autoplay;
        pub mod commands;
        pub mod components;
        pub mod manager;
        pub mod microphone;
        pub mod mixer;
        pub mod preview;
        pub mod script_actions;
        pub mod systems;
        pub mod timeline;
        pub mod timeline_scheduler;

        pub use commands::{AudioCommand, AudioCommandQueue};
        pub use components::AudioPlayer;
        pub use manager::{amplitude_to_db, quat_to_mint, vec3_to_mint, ActiveInputStream, KiraAudioManager, RolloffType};
        pub use microphone::{
            list_input_devices, list_output_devices, open_microphone, MicError,
            MicrophoneSoundData, MicrophoneSoundHandle, OpenedMicrophone,
        };
        pub use mixer::{ChannelStrip, MixerState};
        pub use preview::AudioPreviewState;
        pub use systems::{AudioListener, AudioSet};
        pub use timeline::{
            ClipId, TimelineClip, TimelineState, TimelineTrack, TrackId, Transport, TransportState,
        };
        pub use timeline_scheduler::ActiveClips;
    }
}

use bevy::prelude::*;

pub use fx_bridge::{
    BusInsertsSummary, FxSlotSummary, MixerFxCommand, MixerFxOp, PluginCatalog, PluginCatalogEntry,
};

/// Bevy plugin that initializes the Kira audio system.
#[derive(Default)]
pub struct KiraPlugin;

impl Plugin for KiraPlugin {
    fn build(&self, _app: &mut App) {
        info!("[runtime] KiraPlugin");

        // Bridge types — registered on every platform so panels can read
        // them safely whether or not the native audio stack is up.
        _app.init_resource::<BusInsertsSummary>()
            .init_resource::<PluginCatalog>()
            .add_message::<MixerFxCommand>();

        #[cfg(not(target_arch = "wasm32"))]
        {
            use self::{
                commands::AudioCommandQueue, manager::KiraAudioManager, microphone, mixer,
                preview::AudioPreviewState, systems, systems::AudioSet, timeline::TimelineState,
                timeline_scheduler,
            };

            _app.insert_non_send_resource(KiraAudioManager::new())
                .insert_non_send_resource(timeline_scheduler::ActiveClips::default())
                .insert_resource(AudioPreviewState::default())
                .insert_resource(mixer::MixerState::default())
                .insert_resource(AudioCommandQueue::default())
                .insert_resource(TimelineState::default())
                .configure_sets(
                    Update,
                    (AudioSet::Commands, AudioSet::Sync, AudioSet::Cleanup).chain(),
                )
                .add_systems(Update, sync_audio_project_path.before(AudioSet::Commands))
                .add_systems(
                    Update,
                    autoplay::audio_player_autoplay.before(AudioSet::Commands),
                )
                .add_systems(
                    Update,
                    systems::process_audio_commands.in_set(AudioSet::Commands),
                )
                .add_systems(Update, systems::sync_spatial_audio.in_set(AudioSet::Sync))
                .add_systems(Update, mixer::sync_mixer_to_kira)
                .add_systems(Update, microphone::sync_microphone_inputs)
                .add_systems(Update, systems::preview_audio_system)
                .add_systems(
                    Update,
                    systems::prune_finished_sounds.in_set(AudioSet::Cleanup),
                )
                .add_systems(Update, systems::update_vu_meters)
                .add_systems(Update, timeline_scheduler::tick_transport)
                .add_systems(
                    Update,
                    timeline_scheduler::drive_clip_playback
                        .after(timeline_scheduler::tick_transport),
                )
                .add_systems(Update, timeline_scheduler::cache_clip_durations);

            // Consume audio ScriptActions (play_sound/play_music/etc.) emitted by
            // scripts and blueprints, forwarding them to the command queue.
            _app.add_observer(crate::script_actions::handle_audio_script_actions);
        }

        #[cfg(target_arch = "wasm32")]
        info!("[runtime] Audio disabled on WASM");
    }
}

/// Keep the audio manager's project root in sync with the current project, so
/// `resolve_path` turns relative clip paths (e.g. `"audio/music.ogg"`) into
/// absolute paths the direct `from_file` callers (music, preview, timeline) can
/// open. Without this the manager's `project_path` stays `None` and paths
/// resolve against the process working directory.
#[cfg(not(target_arch = "wasm32"))]
fn sync_audio_project_path(
    project: Option<Res<renzora::core::CurrentProject>>,
    audio: Option<NonSendMut<manager::KiraAudioManager>>,
) {
    let (Some(project), Some(mut audio)) = (project, audio) else {
        return;
    };
    if project.is_changed() || audio.project_path.is_none() {
        audio.project_path = Some(project.path.clone());
    }
}

renzora::add!(KiraPlugin);
