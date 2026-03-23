//! Renzora Hub — marketplace browser, library, and asset installer.
//!
//! Provides two panels:
//! - **Store** (`hub_store`): browse, search, purchase marketplace assets
//! - **My Library** (`hub_library`): view owned assets and install to project

pub mod install;
pub mod library;
pub mod store;

use bevy::prelude::*;
use renzora_editor::AppEditorExt;

pub struct HubPlugin;

impl Plugin for HubPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] HubPlugin");
        app.register_panel(store::HubStorePanel::default());
        app.register_panel(library::HubLibraryPanel::default());
    }
}
