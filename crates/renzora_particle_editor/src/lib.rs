//! Particle Editor Panel & Preview
//!
//! Full-featured editor for bevy_hanabi particle effects with live preview.

mod native_editor_panel;
mod native_graph;
mod native_preview_panel;
mod preview;

use bevy::prelude::*;

#[derive(Default)]
pub struct ParticleEditorPlugin;

impl Plugin for ParticleEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ParticleEditorPlugin");
        app.add_plugins(preview::ParticlePreviewPlugin);
        app.add_plugins(native_preview_panel::NativeParticlePreview);
        app.add_plugins(native_editor_panel::NativeParticleEditor);
        app.add_plugins(native_graph::NativeParticleGraph);
    }
}

renzora::add!(ParticleEditorPlugin, Editor);
