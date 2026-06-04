//! Renzora Hub — marketplace browser, library, and asset installer.
//!
//! Provides two panels:
//! - **Marketplace** (`hub_store`): browse, search, purchase marketplace assets
//! - **My Library** (`hub_library`): view owned assets and install to project

pub mod images;
pub mod install;
pub mod library;
mod native_library;
pub mod overlay;
pub mod preview;
pub mod store;
mod thumbs;

use bevy::prelude::*;
use renzora_editor::AppEditorExt;

#[derive(Default)]
pub struct HubPlugin;

impl Plugin for HubPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] HubPlugin");
        app.add_plugins(preview::HubPreviewPlugin);
        app.register_panel(store::HubStorePanel::default());
        app.register_panel(library::HubLibraryPanel::default());
        // bevy_ui-native: shared thumbnail cache + the My Library panel.
        app.init_resource::<thumbs::HubThumbs>();
        app.add_systems(Update, thumbs::poll_thumbs);
        app.add_plugins(native_library::NativeHubLibrary);
    }
}

renzora::add!(HubPlugin, Editor);
