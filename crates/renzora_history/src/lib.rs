//! History panel — view and jump through the undo/redo stack.
//!
//! The panel is bevy_ui (ember) native; see [`native`].

pub mod native;

use bevy::prelude::*;

#[derive(Default)]
pub struct HistoryPanelPlugin;

impl Plugin for HistoryPanelPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] HistoryPanelPlugin");
        native::register_native_history(app);
    }
}

renzora::add!(HistoryPanelPlugin, Editor);
