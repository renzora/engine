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
