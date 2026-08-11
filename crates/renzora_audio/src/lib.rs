//! The engine's audio **API**. There is no audio in it.
//!
//! This crate owns the bus graph, the components scenes serialize, the command
//! queue, the timeline and the emitter bookkeeping. What makes sound is a
//! separate C-ABI plugin implementing [`renzora_plugin::audio::Backend`] — drop
//! `audio.dll` beside the binary and the game plays; leave it out and the same
//! binary runs silent, carrying none of the cost.
//!
//! That split is why nothing here links a device, a decoder or any DSP.
//! [`link`] is the one module that knows a backend is a plugin at all, and it
//! answers harmlessly when none is loaded — which is what makes the plugin
//! genuinely removable rather than nominally optional.
//!
//! On wasm the same API is linked against a WebAudio backend instead, because
//! cpal cannot capture in a browser. Neither backend knows the other exists.

// `fx_bridge` and `link` compile on every platform — they are data types and a
// function pointer, with no audio in them. Other UI crates depend on these
// regardless of platform, so they stay outside the cfg gate.
pub mod fx_bridge;

/// The engine side of the audio boundary — the one place that knows the backend
/// is a plugin.
pub mod link;

/// A PCM decoder for waveform drawing. Behind the `decode` feature — see the
/// module doc for why the runtime deliberately does not have one.
#[cfg(feature = "decode")]
pub mod decode;

pub use link::{AudioLink, CaptureId, SoundId, VoiceId};

/// The request vocabulary, re-exported.
///
/// A caller of this API should not have to name `renzora_plugin` to ask for a
/// sound — that crate is the *boundary*, and which crate the types happen to be
/// declared in is an implementation detail of how the backend is loaded.
pub use renzora_plugin::audio::{
    BackendInfo, Caps, EmitterState, ListenerState, PlayRequest, StopRequest, StopTarget,
};

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        pub mod autoplay;
        pub mod commands;
        pub mod components;
        pub mod mixer;
        pub mod preview;
        pub mod runtime;
        pub mod script_actions;
        pub mod systems;
        pub mod timeline;
        pub mod timeline_scheduler;

        pub use commands::{AudioCommand, AudioCommandQueue};
        pub use components::{AudioPlayer, RolloffType};
        pub use mixer::{
            rename_custom_bus, Bus, ChannelStrip, MixerState, BUILTIN_BUSES, BUS_COLORS,
        };
        pub use preview::AudioPreviewState;
        pub use runtime::{ActiveVoices, SoundCache};
        pub use systems::{AudioListener, AudioSet, MasterVolume, MusicVoice};
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

/// Device names the loaded backend reports.
///
/// Mirrored into a resource rather than queried on demand because the mixer
/// panel's device menus are built inside a panel-content closure that has no
/// `World` to call the backend from — and enumerating devices is a system call
/// per open, which a menu should not pay for while it is being drawn.
#[derive(Resource, Default, Clone)]
pub struct AudioDevices {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

/// The engine's audio API.
#[derive(Default)]
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, _app: &mut App) {
        // Registered on every platform so panels can read them safely whether or
        // not a backend is present.
        _app.init_resource::<BusInsertsSummary>()
            .init_resource::<PluginCatalog>()
            .init_resource::<AudioLink>()
            .init_resource::<AudioDevices>()
            .add_message::<MixerFxCommand>();

        #[cfg(not(target_arch = "wasm32"))]
        {
            use self::{
                commands::AudioCommandQueue, mixer, preview::AudioPreviewState, runtime, systems,
                systems::AudioSet, timeline::TimelineState, timeline_scheduler,
            };

            // Reflection registration is what makes a component serializable:
            // `save_scene` walks `AppTypeRegistry` and silently skips anything
            // absent from it. Both of these were missing, so neither an emitter
            // nor a listener survived a save — `RolloffType` too, because a
            // nested type has to be registered for the field around it to
            // round-trip.
            _app.register_type::<components::AudioPlayer>()
                .register_type::<components::RolloffType>()
                .register_type::<systems::AudioListener>();

            _app.init_resource::<runtime::SoundCache>()
                .init_resource::<runtime::ActiveVoices>()
                .init_resource::<timeline_scheduler::ActiveClips>()
                .init_resource::<systems::MusicVoice>()
                .init_resource::<systems::MasterVolume>()
                .insert_resource(AudioPreviewState::default())
                .insert_resource(mixer::MixerState::default())
                .insert_resource(AudioCommandQueue::default())
                .insert_resource(script_actions::AudioPlayerRuntime::default())
                .insert_resource(TimelineState::default())
                .configure_sets(
                    Update,
                    (AudioSet::Commands, AudioSet::Sync, AudioSet::Cleanup).chain(),
                )
                // Adopting the backend runs before anything that would talk to
                // it, so a plugin that loaded this frame is usable this frame.
                .add_systems(
                    Update,
                    runtime::adopt_backend.before(AudioSet::Commands),
                )
                // After adoption, so the first frame of a shipped runtime — where
                // the scene is already spawned and the backend arrives the same
                // frame — sees an active link rather than waiting for the next one.
                .add_systems(
                    Update,
                    autoplay::audio_player_autoplay
                        .after(runtime::adopt_backend)
                        .before(AudioSet::Commands),
                )
                .add_systems(
                    Update,
                    systems::process_audio_commands.in_set(AudioSet::Commands),
                )
                .add_systems(Update, systems::sync_spatial_audio.in_set(AudioSet::Sync))
                // After the command pass, so a restart it queues is picked up on
                // the next frame rather than racing this one's playback.
                .add_systems(
                    Update,
                    systems::apply_audio_player_edits.in_set(AudioSet::Sync),
                )
                // Load the board before syncing it, so a freshly-opened project
                // reaches the backend on the same frame; save after, so a change
                // made this frame is what gets written.
                .add_systems(
                    Update,
                    (
                        mixer::load_mixer_config,
                        runtime::sync_mixer_to_backend,
                        mixer::save_mixer_config,
                    )
                        .chain()
                        .in_set(AudioSet::Sync),
                )
                // The per-frame conversation last: it collects the meters and the
                // finished voices, which every system above may have added to.
                .add_systems(Update, runtime::audio_update.in_set(AudioSet::Cleanup))
                // After the update that drops finished voices, so the marker
                // clears on the same frame a sound ends rather than a frame late.
                .add_systems(
                    Update,
                    runtime::mark_emitting_entities.after(runtime::audio_update),
                )
                .add_systems(Update, systems::preview_audio_system)
                .add_systems(
                    Update,
                    systems::drop_despawned_voices.in_set(AudioSet::Cleanup),
                )
                .add_systems(Update, sync_audio_devices)
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
    }
}

/// Refresh the device list when a backend appears.
///
/// Not per-frame: enumerating devices is a system call, and the list only
/// changes when hardware does. The mixer's menus re-enumerate on open for the
/// mic-plugged-in-after-launch case; this is the baseline they start from.
#[cfg(not(target_arch = "wasm32"))]
fn sync_audio_devices(
    mut link: ResMut<AudioLink>,
    mut devices: ResMut<AudioDevices>,
    mut known: Local<bool>,
) {
    if !link.is_active() {
        *known = false;
        return;
    }
    if *known {
        return;
    }
    *known = true;
    let list = link.list_devices();
    devices.inputs = list.inputs;
    devices.outputs = list.outputs;
}

/// Enumerate input devices. Empty when no backend is loaded, or when the one
/// that is cannot enumerate — a browser cannot before permission is granted.
pub fn list_input_devices(devices: &AudioDevices) -> Vec<String> {
    devices.inputs.clone()
}

/// Enumerate output devices. See [`list_input_devices`].
pub fn list_output_devices(devices: &AudioDevices) -> Vec<String> {
    devices.outputs.clone()
}

renzora::add!(AudioPlugin);
