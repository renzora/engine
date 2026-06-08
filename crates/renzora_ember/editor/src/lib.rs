//! Editor-only markup editor (folded in from renzora_hui) — the HTML-template editor integration
//! (entity preset, hierarchy icons, template-path inspector) plus the bevy_ui
//! style-component inspectors (Node/Background/Border/TextFont/TextColor/
//! ImageNode) with markup writeback.
//!
//! `renzora_hui` compiles lean (no `editor` feature, no egui-phosphor). This
//! crate holds the editor registrations, which read/write the `pub`
//! `renzora_hui` runtime types (`HtmlTemplatePath`, `MarkupSource`,
//! `MarkupImage`) and call `renzora_ember::markup::writeback::write_attr_to_markup`.
//! Registered `renzora::add!(HuiEditorPlugin, Editor)` and linked only by the
//! editor bundle.
//!
//! No `renzora_ember` dep: the HUI editor surface is pure editor-contract
//! (`InspectorEntry`/`EntityPreset`/`ComponentIconEntry`) — there is no native
//! ember panel to relocate.

use bevy::prelude::*;

mod editor;
mod inspector;

pub use editor::HuiEditorPlugin;
pub use inspector::HuiInspectorPlugin;

/// Editor-scope companion to `renzora_ember::markup::MarkupPlugin`. Reproduces the
/// registrations the runtime plugin did under `#[cfg(feature = "editor")]`:
/// the HTML-template editor integration and the bevy_ui component inspectors
/// with markup writeback.
#[derive(Default)]
pub struct HuiEditorBundlePlugin;

impl Plugin for HuiEditorBundlePlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] HuiEditorBundlePlugin");
        app.add_plugins((HuiEditorPlugin, HuiInspectorPlugin));
    }
}

renzora::add!(HuiEditorBundlePlugin, Editor);
