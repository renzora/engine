//! DAW-style mixer panel for the Renzora editor.
//!
//! Disabled on WASM (depends on native Kira audio).

use bevy::prelude::*;

#[derive(Default)]
pub struct MixerPlugin;

// On WASM: no-op plugin (no audio backend).
#[cfg(target_arch = "wasm32")]
impl Plugin for MixerPlugin {
    fn build(&self, _app: &mut App) {
        info!("[editor] MixerPlugin (disabled on WASM)");
    }
}

// On native: full mixer panel with Kira audio integration.
#[cfg(not(target_arch = "wasm32"))]
mod inspectors;
#[cfg(not(target_arch = "wasm32"))]
mod render;
#[cfg(not(target_arch = "wasm32"))]
mod native;

#[cfg(not(target_arch = "wasm32"))]
impl Plugin for MixerPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] MixerPlugin");
        native::build(app);
    }
}

