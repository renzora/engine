//! UI Canvas shared state.
//!
//! The egui WYSIWYG canvas panel that used to live here has been removed; its
//! native (bevy_ui) replacement lives in the `renzora_game_ui_editor` crate.
//! This module retains only the runtime/editor-shared resources that other
//! crates (e.g. `renzora_game_ui_editor`, `renzora_viewport`) still consume.

use bevy::prelude::*;

// ── Canvas backdrop toggle ───────────────────────────────────────────────────

/// Whether the editor viewport's scene render is shown behind the UI canvas.
/// Toggled by the canvas panel toolbar; the backdrop image comes from
/// `ViewportRenderTarget` (the shared slot-0 image — the 3D render, or the 2D
/// render when UI view was entered from 2D; see `LastSceneView` in
/// `renzora_viewport`), so flipping this off just hides the blit — no camera
/// spawn/despawn.
///
/// Default reset from `EditorSettings::ui_preview_by_default` whenever the
/// UI workspace is entered.
#[derive(Resource)]
pub struct UiCanvasPreviewEnabled(pub bool);

impl Default for UiCanvasPreviewEnabled {
    fn default() -> Self {
        Self(true)
    }
}
