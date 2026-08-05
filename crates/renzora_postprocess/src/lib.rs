//! Re-export shim for the post-process framework.
//!
//! The framework now lives in `renzora::postprocess` (so it ships inside
//! `renzora.dll` instead of a standalone `renzora_postprocess.dll`). This
//! crate exists only to keep `renzora_postprocess::…` paths — used by the
//! ~50 effect plugins and emitted by the `post_process` attribute macro —
//! resolving without change. It carries no symbols of its own; the types it
//! re-exports belong to `renzora`, so every consumer shares one
//! `PostProcessRegistry` and matching `TypeId`s via `renzora.dll`.
pub mod plugin_bridge;
/// The plugin custom-material pipeline. `bevy_pbr`-only — it implements bevy's
/// `Material` trait — so a 2D-only export drops it. Install it through
/// [`add_plugin_material`] rather than naming the plugin directly, so callers
/// don't each need their own `cfg`.
#[cfg(feature = "render_3d")]
pub mod plugin_material;

pub use renzora::postprocess::*;

/// Install the plugin custom-material pipeline, if this build has a 3D renderer.
///
/// A no-op without `render_3d`. Exists so `src/main.rs` and the editor binary can
/// call it unconditionally: `renzora_app` has no `render_3d` feature of its own
/// (the exporter builds it with just `--features runtime`), so a `#[cfg]` at the
/// call site had nothing to test. The plugin host degrades the same way —
/// `MaterialSlot::Custom` finds no `CustomMaterialApplier` and logs instead of
/// attaching the wrong material.
pub fn add_plugin_material(app: &mut renzora::bevy::app::App) {
    #[cfg(feature = "render_3d")]
    app.add_plugins(plugin_material::PluginMaterialPlugin);
    #[cfg(not(feature = "render_3d"))]
    let _ = app;
}
