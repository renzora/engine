//! Renzora — the contracts crate. Types, events, components, resources.
//!
//! `renzora` is the foundation every other crate in the engine depends on.
//! It has zero dependencies of its own beyond Bevy + serde, so it cannot
//! introduce circular dependencies and any crate is free to swap any other
//! crate as long as both honor the contracts defined here.
//!
//! Plugins that want extra functionality (post-process effects, editor
//! framework, theming, etc.) depend on those crates explicitly:
//!
//! ```toml
//! [dependencies]
//! bevy = { workspace = true }
//! renzora = { path = "..." }                 # types + events
//! renzora_editor_framework = { path = "..." } # editor panels, inspector
//! renzora_postprocess = { path = "..." }      # post-process effect derive
//! ```

// Re-export bevy so the `add!` macro can reach it as `$crate::bevy::...`
// and plugin authors can write `use renzora::bevy::prelude::*;` to skip
// a separate workspace dep if they want.
pub use bevy;

// ── Core types ───────────────────────────────────────────────────────────
// Everything that used to live in `renzora_core`. Re-exported at the crate
// root so callers write `renzora::Foo` instead of `renzora::core::Foo`.
pub mod core;
pub use core::*;

// ── Dynamic plugin FFI macro ────────────────────────────────────────────
// `renzora::add!(MyPlugin)` exports the FFI symbols a Renzora editor /
// runtime loader needs to instantiate the plugin from a `.dll` / `.so` /
// `.dylib`. Originally lived in a separate `dynamic_plugin_meta` crate;
// folded in here so plugin authors only ever need `bevy` + `renzora`.
mod plugin_meta;
pub use plugin_meta::PluginScope;
// `add!` is registered at the crate root via `#[macro_export]` in plugin_meta.rs.
