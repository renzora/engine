//! The editor — every editor-only plugin crate, linked as rlibs and installed
//! by [`install`].
//!
//! This crate is what makes the editor removable. `renzora.exe` links only
//! `renzora_runtime`; `renzora-editor.exe` links this crate on top. Same engine,
//! two binaries, and a shipped game carries none of the editor.
//!
//! It was a `dlopen`'d cdylib until Bevy went static. A loadable bundle needs to
//! share the host's Bevy, which meant a shared `bevy_dylib` and a `World`
//! `TypeId` check on both sides; with Bevy linked statically there is no second
//! `bevy_dylib` to share, so the bundle became a second binary and the whole
//! ABI-matching apparatus went away with it. Third-party extensions are C-ABI
//! plugins (`renzora_plugin`), which link no Bevy at all and so have no such
//! constraint.

#[cfg(feature = "editor")]
mod plugins;

/// Install the whole editor into `app`.
///
/// Called directly by the `renzora-editor` binary, which links this crate as an
/// rlib. Call it AFTER `add_engine_plugins`, so the editor layers on top of the
/// runtime foundation.
///
/// The three foundation plugins below must go first and in this order: they
/// init the shared registries (AssetRegistry → editor registries → KeyBindings)
/// that every Editor-scope plugin reads inside its own `build()`.
#[cfg(feature = "editor")]
pub fn install(app: &mut renzora::bevy::app::App) {
    app.add_plugins(renzora_asset_registry::AssetRegistryPlugin);
    app.add_plugins(renzora_editor_framework::RenzoraEditorPlugin);
    app.add_plugins(renzora_keybindings::KeybindingsPlugin);

    plugins::add_editor_plugins(app);
}
