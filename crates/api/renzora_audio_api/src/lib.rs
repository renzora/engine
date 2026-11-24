//! The audio boundary: `AudioLink`, the handle types it allocates, and the mixer
//! backend contract.
//!
//! A layer of the contract hierarchy, above `renzora`. Plugins reach it as
//! `renzora::audio` and `renzora::audio_backend` through `renzora_sdk`, so any
//! plugin can play a sound; the engine's own crates depend on this crate and
//! write `renzora_audio_api::audio::...`.

// `AudioLink`, the engine side of the boundary. The mixer, timeline and emitters
// are `renzora_audio`; only the link is here. `SoundId` and `VoiceId` are too
// generic for a crate root, so callers name the module.
pub mod audio;

// The mixer contract, beside `audio` because its link holds these types.
pub mod audio_backend;

// The game's audio settings in the runtime settings store
// (`CurrentProject.config.audio`): the one place a bus volume, master included,
// is decided. Here rather than in `renzora_audio` so scripts and plugins can
// set a volume without linking the mixer; reached as `renzora::audio::settings`.
pub mod settings;
