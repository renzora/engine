//! The Renzora audio engine, as a crate the binary links rather than a plugin it
//! loads.
//!
//! `renzora_audio` ships the audio *API*: the bus graph, the components, the
//! command queue, the timeline, and no audio. This crate is the audio. It
//! decodes, mixes, spatialises, and hands blocks to the sound card.
//!
//! # Why this is not a distribution plugin
//!
//! It was one, and the argument was the ordinary one for plugins: cpal,
//! symphonia and the decoders are real weight, and a 2D puzzle game that never
//! makes a sound should not compile them. That part still holds, but a *feature*
//! strips a crate just as completely as a missing file does, and the `audio`
//! feature on `renzora_runtime` is where that decision now lives.
//!
//! What the plugin form could not do is be reliably present. Every one of the
//! API's moving parts is inert without a backend: an `AudioPlayer` resolves no
//! sound, the mixer's faders move nothing, the timeline schedules silence. A
//! game exported without the library beside it, or a player who deleted one file
//! out of `plugins/`, got a binary that ran perfectly and made no noise, with no
//! error to explain it. Linking it in makes "the engine has audio" a property of
//! the build rather than of the folder it was copied into.
//!
//! # What has not changed
//!
//! The contract. This registers through `renzora_plugin::audio::Backend` and is
//! installed with [`load_static`](renzora_plugin::host::loader::load_static),
//! the same call the lean exporter uses for a plugin it compiled in, so it takes
//! an ordinary registration slot and is reported like any other backend. Nothing
//! in the host knows this one arrived by a different route, which is what still
//! lets a second backend (WebAudio on wasm, where the browser supplies the graph
//! and the decoders) implement the same contract without sharing a line of code
//! with this one.
//!
//! Below the plugin at the bottom of this file, the mixer links neither Bevy nor
//! the engine. It speaks in `f32` samples, bus keys and positions, which is all
//! a mixer needs to know.
//!
//! ## Layout
//!
//! | module | what it is |
//! |---|---|
//! | [`decode`] | encoded bytes → PCM, via symphonia |
//! | [`effects`] | the reverb and delay sends |
//! | [`device`] | the cpal output stream and the queue into it |
//! | [`capture`] | microphone / line input |
//! | [`backend`] | the engine's audio-backend contract, implemented |
//! | [`pcm`] | decoded audio and fractional-rate playback |
//! | [`spatial`] | distance attenuation and stereo positioning |
//! | [`graph`] | voices, buses, and the block render loop |
//!
//! ## What is not here, deliberately
//!
//! **File I/O.** The engine resolves paths and hands over bytes, exactly as it
//! does for script source. That is not tidiness: exported and Android builds
//! read assets out of an rpak archive through a closure the engine owns, so a
//! backend doing its own `std::fs` would work in the editor and fail in every
//! shipped game.

extern crate alloc;

/// The engine's audio backend, over the mixer below. Native only: a browser
/// build implements the same contract against WebAudio instead.
#[cfg(not(target_arch = "wasm32"))]
pub mod backend;
/// Microphone and line input. Native only, for the same reason as [`device`],
/// and more sharply: cpal's wasm hosts return an error from
/// `build_input_stream_raw`, so capture on the web needs `getUserMedia` and a
/// backend written against it.
#[cfg(not(target_arch = "wasm32"))]
pub mod capture;
pub mod decode;
/// Reverb and delay, as shared send effects.
pub mod effects;
/// The sound card. Native only: a browser build gets a WebAudio backend
/// instead, because cpal's wasm hosts cannot capture (see `Cargo.toml`).
#[cfg(not(target_arch = "wasm32"))]
pub mod device;
pub mod graph;
pub mod pcm;
pub mod spatial;

pub use decode::{decode, DecodeError};
pub use effects::{Delay, Reverb, ReverbSettings};
#[cfg(not(target_arch = "wasm32"))]
pub use backend::RenzoraAudio;
#[cfg(not(target_arch = "wasm32"))]
pub use capture::{input_devices, output_devices, resample_stereo, Capture};
#[cfg(not(target_arch = "wasm32"))]
pub use device::{AudioDevice, Command, DeviceError};
pub use graph::{Bus, Engine, PlayParams, VoiceId};
pub use pcm::{Pcm, PcmRef};
pub use spatial::{Emitter, Listener, Rolloff};

// ── The plugin ───────────────────────────────────────────────────────────

/// Registers the audio backend. An ordinary Bevy plugin, so it is declared and
/// installed exactly like every other crate in this workspace.
#[derive(Default)]
pub struct RenzoraAudioPlugin;

impl bevy::app::Plugin for RenzoraAudioPlugin {
    // On wasm there is no cpal backend to register, so this installs nothing and
    // the host is left without one, which the contract already models as "a game
    // that carries no audio" and reports the same way. cpal's wasm hosts return
    // an error from `build_input_stream_raw`, so capture there needs
    // `getUserMedia`; when the WebAudio backend lands it registers here, by the
    // same `load_static` call.
    #[cfg(target_arch = "wasm32")]
    fn build(&self, _app: &mut bevy::app::App) {}

    #[cfg(not(target_arch = "wasm32"))]
    fn build(&self, app: &mut bevy::app::App) {
        // Installed through `load_static`, the path the lean exporter uses for a
        // plugin it compiled in. Deliberate rather than incidental: the mixer
        // implements `renzora_plugin::audio::Backend`, and the host adopts a
        // backend by descriptor. Going through the ordinary contract means the
        // built-in mixer and one loaded from `plugins/` arrive by the same
        // route, take the same registration slot and are reported the same way,
        // rather than the built-in one being a special case every other part of
        // the system has to know about.
        //
        // Three arguments, none of them obvious:
        //
        // * `init` is named directly rather than looked up by string, because
        //   `static_link` makes the entry point an ordinary function instead of
        //   an exported symbol.
        // * `id: "audio"` is what the plugin scanner matches a loose
        //   `plugins/audio` against. An old copy left in an install directory is
        //   then skipped rather than registering a second backend. See
        //   `LinkedPluginIds`.
        // * `true` for `is_editor` gates Editor-scope plugins; it describes the
        //   host, not this plugin, which is Runtime and belongs in both.
        let outcome = renzora_plugin::host::loader::load_static(
            app.world_mut(),
            &renzora_plugin::static_link::StaticPlugin {
                id: "audio",
                scope: renzora_plugin::sys::PluginScope::Runtime,
                init: plugin::renzora_plugin_init,
            },
            true,
        );
        if !matches!(outcome, renzora_plugin::host::loader::LoadOutcome::Loaded) {
            bevy::log::error!("[audio] the built-in audio backend did not install: {outcome:?}");
        }
    }
}

// Declared, not hand-wired. `cargo renzora sync` reads this and writes the entry
// into the runtime's generated plugin list, so adding the crate is the whole
// job. `Runtime` is what puts the mixer in a shipped game as well as the editor,
// which is the property the plugin form was supposed to provide and could not
// guarantee.
renzora::add!(RenzoraAudioPlugin);

// ── The backend itself ────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
mod plugin {
    use renzora_plugin::prelude::*;

    // Emits the descriptor and the state it needs. A macro rather than a generic
    // because the entry point must be a bare function pointer with nowhere to
    // carry state, so it needs a `static`, and a `static` cannot be generic over
    // the backend type.
    renzora_plugin::audio_backend!(crate::backend::RenzoraAudio);

    /// The C-ABI plugin `load_static` installs: one call, registering the
    /// descriptor above.
    pub struct Inner;

    impl Plugin for Inner {
        fn build(&self, app: &mut App) {
            app.add_audio_backend(audio_backend::desc());
        }
    }

    renzora_plugin::add!(Inner);
}

#[cfg(test)]
mod tests;
