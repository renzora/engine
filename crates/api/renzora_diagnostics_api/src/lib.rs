//! What the engine reports about itself.
//!
//! A layer of the contract hierarchy, above `renzora`. Plugins reach each module
//! at the path it always had (`renzora::profiling`, `renzora::load_timeline`, ...)
//! through `renzora_sdk`; the engine's own crates depend on this crate and write
//! `renzora_diagnostics_api::profiling::...`.
//!
//! Several of these hold process-global state, which is why they are in the
//! contract at all: `renzora_dylib` embeds this crate, so the editor, the
//! runtime and every plugin share one copy of each buffer.

// What the debug panels read. Here rather than in `renzora_shader` /
// `renzora_scripting` because those panels are a native plugin now, and a plugin
// reaches only `bevy`, `renzora` and `renzora_ember`. Collection stays with the
// crates doing the work; both re-export their old path. `MaterialPerfStats` and
// friends are panel vocabulary, so callers name the module.
#[cfg(feature = "diagnostics")]
pub mod diagnostics;

// The profiler's recorder: a log layer that timestamps every span (each Bevy
// system run is one) into per-thread buffers. In the contract because its switch
// and buffers must be one per process. See the module docs.
pub mod profiling;

// Debug readouts a process offers the companion: registered by any crate or
// plugin, built only while a companion watches them. See the module docs.
pub mod probes;

// A timestamped record of the last project load. Nearly every writer is a
// different crate, and the MCP plugin reads it.
pub mod load_timeline;

// The `LogPlugin::custom_layer` factory and global ring buffer behind the
// editor's Scene Diagnostics "Recent Runtime Warnings" feed. In the contract so
// the capture layer the lean runtime binary installs and the panel that reads it
// from the editor bundle touch ONE buffer across the dylib boundary. Gated off
// mobile (no editor there, and the bevy_log tracing-subscriber surface isn't
// guaranteed); desktop and the web editor keep it.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod runtime_warnings;

// Per-file problems with authored content, behind the editor's Problems panel.
// In the contract because the producer (the material resolver) and the reader
// (the code editor) must agree on one definition.
pub mod content_problems;
