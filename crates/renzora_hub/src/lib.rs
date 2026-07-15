//! Renzora Marketplace — marketplace browser, library, and asset installer.
//!
//! Provides two bevy_ui-native panels:
//! - **Marketplace** (`hub_store`): browse/search assets, preview themes live,
//!   and download/install via a destination-folder picker
//! - **My Library** (`hub_library`): view owned assets and install to project

pub mod install;
mod hub_lightbox;
mod install_overlay;
mod item_overlay;
mod material_viewer;
mod model_viewer;
mod native_library;
mod native_store;
mod thumbs;
mod upload_panel;

use bevy::prelude::*;

#[derive(Default)]
pub struct HubPlugin;

impl Plugin for HubPlugin {
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
