//! The editor — every editor-only plugin crate, linked as rlibs and installed
//! by [`install`].
//!
//! This crate is what makes the editor removable. `renzora.exe` links only
//! `renzora_runtime`; `renzora-editor.exe` links this crate on top. Same engine,
//! two binaries, and a shipped game carries none of the editor.
//!
//! It is a loadable image again. It was one until Bevy went static — a bundle
//! has to share the host's Bevy, and with Bevy linked into each binary there was
//! no shared copy to attach to, so the bundle became a second executable. Bevy
//! is a shared image once more, so the boundary is sound again.
//!
//! # What is and is not shared
//!
//! `bevy_dylib`, `renzora_dylib` and `renzora_ember_dylib` are shared because
//! those crates keep **process-global statics** — the translation table, the
//! Console and Problems buffers, the theme palette and stylesheet — that must be
//! one thing per process or they fail silently.
//!
//! The other crates this image and the executable both depend on are simply
//! linked into each. That costs bytes and nothing else: a `TypeId` comes from a
//! crate's stable id, not from which artifact swallowed it, so both sides agree
//! about what a component is even with two copies of the code. It is the same
//! arrangement every native plugin already runs under.
//!
//! Third-party extensions are C-ABI plugins (`renzora_plugin`), which link no
//! Bevy at all and so have never had this constraint.

// Linked for their side effect only — see the dependency comment in
// `Cargo.toml`. Their presence is what makes `renzora` and `renzora_ember`
// resolve to the shared images instead of being embedded here, which is what
// keeps the translation table, the Console buffers and the theme palette one
// thing per process rather than one per image.
#[cfg(not(target_arch = "wasm32"))]
extern crate renzora_dylib;
#[cfg(not(target_arch = "wasm32"))]
extern crate renzora_ember_dylib;

#[cfg(feature = "editor")]
mod plugins;

/// The symbol the executable looks up in `renzora_editor.<dll|so|dylib>`.
///
/// Unmangled so it can be found by name, exactly as `renzora::plugin!` emits for
/// a native plugin — and for the same reason: every way of writing this by hand
/// fails identically, with the image loading, the symbol absent, and the editor
/// silently not installed.
///
/// Taking `&mut App` across a `dlopen` boundary is sound here only because the
/// executable and this image link one `bevy_dylib`, one `renzora_dylib`, one
/// `renzora_ember_dylib` and one `renzora_runtime_dylib` — so `App`, `World` and
/// every component type are the same types on both sides. See the module docs.
#[cfg(all(feature = "editor", not(target_arch = "wasm32")))]
#[unsafe(no_mangle)]
pub fn renzora_editor_install(app: &mut renzora::bevy::app::App) {
    install(app);
}

/// Install the whole editor into `app`.
///
/// Reached by symbol through [`renzora_editor_install`] on native, and called
/// directly on wasm, which has no dynamic linking and builds the editor into its
/// own `.wasm` bundle. Call it AFTER `add_engine_plugins`, so the editor layers
/// on top of the runtime foundation.
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
