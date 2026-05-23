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
//! renzora_editor = { path = "..." } # editor panels, inspector
//! renzora_postprocess = { path = "..." }      # post-process effect derive
//! ```

// Re-export bevy so the `add!` macro can reach it as `$crate::bevy::...`
// and plugin authors can write `use renzora::bevy::prelude::*;` to skip
// a separate workspace dep if they want.
pub use bevy;

// Re-export inventory so the `add!` macro can reach it as
// `$crate::inventory::...`. Plugin authors don't need to add inventory as
// a direct dep.
pub use inventory;

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
pub use plugin_meta::{for_each_static_plugin, PluginScope, StaticPlugin};
// `add!` is registered at the crate root via `#[macro_export]` in plugin_meta.rs.

// ── Post-process framework ───────────────────────────────────────────────
// `PostProcessPlugin<T>`, `PostProcessEffect`, the unified render-graph node
// and the shared `PostProcessRegistry`. Folded in from the old standalone
// `renzora_postprocess` dylib so its symbols ship inside `renzora.dll`
// instead of a separate file — the ~50 effect plugins still resolve one
// shared copy (and one `PostProcessRegistry` TypeId) across the dlopen
// boundary, just from `renzora` now. The `renzora_postprocess` crate
// remains as a thin rlib re-export shim so existing `renzora_postprocess::…`
// paths (incl. those emitted by the `post_process` macro) keep compiling.
//
// Gated so non-rendering targets (mobile staticlib, wasm, headless server)
// don't pull the render-graph surface into the lean base crate.
#[cfg(feature = "postprocess")]
pub mod postprocess;

// ── App lifecycle state ──────────────────────────────────────────────────
//
// Coordination contract used by both the splash screen UI and the editor
// framework. Lives in the SDK so neither side has to depend on the other's
// implementation crate.

/// Top-level app phase. The splash UI runs while `Splash`, a loading
/// overlay during `Loading`, and the full editor while `Editor`.
#[derive(bevy::prelude::States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SplashState {
    #[default]
    Splash,
    Loading,
    Editor,
}

/// Marker request: open a different project. Inserted by the editor's File
/// menu; consumed by the splash plugin which shows the file dialog,
/// validates, updates recent projects, and transitions state.
#[derive(bevy::prelude::Resource)]
pub struct RequestOpenProject;
