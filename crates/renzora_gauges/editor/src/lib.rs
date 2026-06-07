//! Editor-only half of `renzora_gauges` — the Gauges inspector entry plus the
//! native (ember) live attribute debug panel.
//!
//! `renzora_gauges` compiles lean (no `editor` feature, no renzora_ember). This
//! crate holds the inspector (renzora editor contract) and the native debug
//! panel (renzora_ember, reads `renzora_gauges::GaugesSnapshot`), registered
//! `renzora::add!(GaugesEditorPlugin, Editor)` and linked only by the editor
//! bundle.

use bevy::prelude::*;
use renzora::AppEditorExt;

mod inspector;
mod native;

/// Editor-scope companion to `renzora_gauges::GaugesPlugin`.
#[derive(Default)]
pub struct GaugesEditorPlugin;

impl Plugin for GaugesEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] GaugesEditorPlugin");
        app.register_inspector(inspector::gauges_inspector_entry());
        // Native (ember) Gauges debug panel.
        app.add_plugins(native::NativeGauges);
    }
}

renzora::add!(GaugesEditorPlugin, Editor);
