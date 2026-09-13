//! Re-export shim for the post-process framework.
//!
//! The framework now lives in `renzora::postprocess` (so it ships inside
//! `renzora.dll` instead of a standalone `renzora_postprocess.dll`). This
//! crate exists only to keep `renzora_postprocess::…` paths — used by the
//! ~50 effect plugins and emitted by the `post_process` attribute macro —
//! resolving without change. It carries no symbols of its own; the types it
//! re-exports belong to `renzora`, so every consumer shares one
//! `PostProcessRegistry` and matching `TypeId`s via `renzora.dll`.
pub use renzora::postprocess::*;
