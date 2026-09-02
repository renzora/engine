//! History panel — view and jump through the undo/redo stack.
//!
//! The panel itself is bevy_ui (ember); see [`panel`].

pub mod panel;

use bevy::prelude::*;

#[derive(Default)]
pub struct HistoryPanelPlugin;

impl Plugin for HistoryPanelPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] HistoryPanelPlugin");
        panel::register(app);
    }
}

renzora::add!(HistoryPanelPlugin, Editor);
