//! Particle Editor Panel & Preview
//!
//! Full-featured editor for bevy_hanabi particle effects with live preview.

mod editor_panel;
mod graph_editor;
mod graph_panel;
mod preview;
mod preview_panel;
mod widgets;

use bevy::prelude::*;
use renzora::editor::AppEditorExt;

#[derive(Default)]
pub struct ParticleEditorPlugin;

impl Plugin for ParticleEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ParticleEditorPlugin");
        app.add_plugins(preview::ParticlePreviewPlugin);
        app.register_panel(editor_panel::ParticleEditorPanel::default());
        app.register_panel(graph_panel::ParticleGraphPanel);
        app.register_panel(preview_panel::ParticlePreviewPanel);
    }
}

renzora::add!(ParticleEditorPlugin);
