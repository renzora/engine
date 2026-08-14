//! Renzora Marketplace — marketplace browser, library, and asset installer.
//!
//! Provides two bevy_ui-native panels:
//! - **Marketplace** (`hub_store`): browse/search assets, preview themes live,
//!   and download/install via a destination-folder picker
//! - **My Library** (`hub_library`): view owned assets and install to project

// ── Web: the marketplace is absent, not broken ───────────────────────────────
// Everything here talks to renzora.com through `renzora_auth`, which is itself
// native-only because `renzora_net` is: the engine's HTTP client is blocking
// calls on worker threads, and the browser offers neither. So on wasm the whole
// crate collapses to a plugin that installs nothing.
//
// The TYPE has to survive, which is why this isn't simply dropped from the
// build: `crates/renzora_editor/src/plugins.rs` is generated from the `add!`
// below and CI fails if regenerating it produces a diff, so `HubPlugin` must
// exist on every target the editor compiles for. Hollowing the plugin keeps the
// generated list byte-identical and needs no change to the generator.
//
// Restoring this is one dependency deep: give `renzora_net` a `fetch`-backed
// web path and the marketplace, library, publish and sign-in all light up
// together, since they share that single client.
#[cfg(not(target_arch = "wasm32"))]
pub mod install;
#[cfg(not(target_arch = "wasm32"))]
mod hub_lightbox;
#[cfg(not(target_arch = "wasm32"))]
mod install_overlay;
#[cfg(not(target_arch = "wasm32"))]
mod item_overlay;
#[cfg(not(target_arch = "wasm32"))]
mod material_viewer;
#[cfg(not(target_arch = "wasm32"))]
mod model_viewer;
#[cfg(not(target_arch = "wasm32"))]
mod native_library;
#[cfg(not(target_arch = "wasm32"))]
mod native_store;
#[cfg(not(target_arch = "wasm32"))]
mod thumbs;
#[cfg(not(target_arch = "wasm32"))]
mod upload_panel;

use bevy::prelude::*;

#[derive(Default)]
pub struct HubPlugin;

impl Plugin for HubPlugin {
    #[cfg(target_arch = "wasm32")]
    fn build(&self, _app: &mut App) {
        info!("[editor] HubPlugin: marketplace unavailable on the web (no HTTP client)");
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn build(&self, app: &mut App) {
        info!("[editor] HubPlugin");
        // bevy_ui-native: shared thumbnail cache + the Marketplace / My Library panels.
        app.init_resource::<thumbs::HubThumbs>();
        app.add_systems(Update, thumbs::poll_thumbs);
        app.add_plugins(native_library::NativeHubLibrary);
        app.add_plugins(native_store::NativeHubStore);
        // The Publish (asset/game uploader) panel — opened by the store's
        // "Upload Asset" button and from the command palette.
        app.add_plugins(upload_panel::UploaderPanel);
        // Offscreen 3D turntable for model/animation assets in the item overlay.
        app.add_plugins(model_viewer::ModelViewerPlugin);
        // Offscreen live material/shader preview (selectable shape + @param controls).
        app.add_plugins(material_viewer::MaterialViewerPlugin);
    }
}

renzora::add!(HubPlugin, Editor);
