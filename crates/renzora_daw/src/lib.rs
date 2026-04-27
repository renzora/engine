//! DAW (Digital Audio Workstation) panel for the Renzora editor.
//!
//! Provides a timeline-based audio arrangement view with track lanes,
//! waveform display, clip editing, and transport controls.
//!
//! Disabled on WASM — depends on the native Kira audio backend.

use bevy::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
mod drop;
#[cfg(not(target_arch = "wasm32"))]
mod panel;
#[cfg(not(target_arch = "wasm32"))]
mod waveform_cache;

#[derive(Default)]
pub struct DawPlugin;

#[cfg(target_arch = "wasm32")]
impl Plugin for DawPlugin {
    fn build(&self, _app: &mut App) {
        info!("[editor] DawPlugin (disabled on WASM)");
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Plugin for DawPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] DawPlugin");
        use crate::panel::{apply_intents, request_clip_waveforms, DawIntentBuffer, DawPanel};
        use crate::waveform_cache::WaveformCache;
        use renzora_editor::AppEditorExt;

        app.init_resource::<DawIntentBuffer>();
        app.init_resource::<WaveformCache>();
        app.register_panel(DawPanel::default());
        // Apply panel intents before the audio scheduler sees them.
        app.add_systems(Update, apply_intents);
        app.add_systems(Update, request_clip_waveforms);
    }
}

