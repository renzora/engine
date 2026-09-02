//! The Renzora audio engine.
//!
//! The engine ships an audio *API* — the bus graph, the components, the command
//! queue, the timeline — and no audio. This crate is the audio: it decodes,
//! mixes, spatialises, and hands blocks to the sound card. Drop it in `plugins/`
//! and the game makes noise; leave it out and the same binary runs silent,
//! carrying none of the cost.
//!
//! That split is the reason this crate links neither Bevy nor the engine. It
//! speaks in `f32` samples, bus keys and positions, which is all a mixer needs
//! to know, and it is why a second backend — WebAudio on wasm, where the browser
//! supplies the graph and the decoders — can implement the same contract without
//! sharing a line of code with this one.
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

/// The engine's audio backend, over the mixer below. Native only — a browser
/// build implements the same contract against WebAudio instead.
#[cfg(not(target_arch = "wasm32"))]
pub mod backend;
/// Microphone and line input. Native only, for the same reason as [`device`] —
/// and more sharply: cpal's wasm hosts return an error from
/// `build_input_stream_raw`, so capture on the web needs `getUserMedia` and a
/// backend written against it.
#[cfg(not(target_arch = "wasm32"))]
pub mod capture;
pub mod decode;
/// Reverb and delay, as shared send effects.
pub mod effects;
/// The sound card. Native only — a browser build gets a WebAudio backend
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

// ── The plugin entry point ───────────────────────────────────────────────────
//
// Native only: a wasm build is linked into the binary rather than dlopen'd, and
// gets the WebAudio backend instead of this one.
#[cfg(not(target_arch = "wasm32"))]
mod plugin {
    use renzora_plugin::prelude::*;

    // Emits the `extern "C"` entry point and the state it needs. A macro rather
    // than a generic because the entry point must be a bare function pointer
    // with nowhere to carry state, so it needs a `static` — and a `static`
    // cannot be generic over the backend type.
    renzora_plugin::audio_backend!(crate::backend::RenzoraAudio);

    pub struct RenzoraAudioPlugin;

    impl Plugin for RenzoraAudioPlugin {
        fn build(&self, app: &mut App) {
            app.add_audio_backend(audio_backend::desc());
        }
    }

    renzora_plugin::add!(RenzoraAudioPlugin);
}

// ── Reachable at the crate root ──────────────────────────────────────────────
//
// A `dlopen`'d plugin is found by SYMBOL, and `#[no_mangle]` exports one from
// wherever it happens to be written — so the entry point sat happily inside the
// private module above for as long as that was the only way in.
//
// A plugin compiled INTO the binary is found by PATH. The lean exporter's
// generated aggregator writes `init: audio::renzora_plugin_init`, and the
// `static_link` build drops `#[no_mangle]` precisely so many plugins can share
// one binary — which leaves the function `pub` inside a private module, i.e.
// unreachable. The failure reads as two unrelated messages:
//
//     error[E0425]: cannot find value `renzora_plugin_init` in crate `audio`
//     warning: struct `RenzoraAudioPlugin` is never constructed
//
// The second is the same cause seen from the other side: an entry point nothing
// can reach is dead code, and it takes everything it constructs down with it.
//
// So the crate root re-exports both halves. That is the contract a linked-in
// plugin has to meet, and it costs one line.
#[cfg(not(target_arch = "wasm32"))]
pub use plugin::{renzora_plugin_init, renzora_plugin_scope};

#[cfg(test)]
mod tests;
