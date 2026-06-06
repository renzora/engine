//! Renzora Hub — marketplace browser, library, and asset installer.
//!
//! Provides two bevy_ui-native panels:
//! - **Marketplace** (`hub_store`): browse, search, purchase marketplace assets
//! - **My Library** (`hub_library`): view owned assets and install to project

pub mod install;
mod native_library;
mod native_store;
mod thumbs;

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
    }
}

renzora::add!(HubPlugin, Editor);
