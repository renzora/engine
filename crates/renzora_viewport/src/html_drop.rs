//! Drag-and-drop HTML template spawning — when a `.html` asset is dropped on the
//! viewport, spawn a UI Canvas with a child `HtmlTemplatePath` entity (the same
//! shape as the "+ Add Entity → HTML Template" preset). The runtime observer in
//! `renzora_hui` turns the path into an `HtmlNode` and bevy_hui builds the markup
//! beneath it. UI is screen-space, so the 3D drop point is ignored.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora_editor_framework::{EditorCommands, EditorSelection};
use renzora_ui::asset_drag::AssetDragPayload;

const HTML_EXTENSIONS: &[&str] = &["html"];

/// On release of an `.html` asset-drag over the native viewport, spawn the UI
/// template.
pub fn native_html_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<AssetDragPayload>>,
    window: Query<&Window, With<PrimaryWindow>>,
    viewport: Res<crate::ViewportState>,
    cmds: Option<Res<EditorCommands>>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let (Some(payload), Some(cmds)) = (payload, cmds) else { return };
    if !payload.is_detached || !payload.matches_extensions(HTML_EXTENSIONS) {
        return;
    }
    let over_viewport = window
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .map(|c| {
            let min = viewport.screen_position;
            let max = min + viewport.screen_size;
            c.x >= min.x && c.y >= min.y && c.x <= max.x && c.y <= max.y
        })
        .unwrap_or(false);
    if !over_viewport {
        return;
    }
    let abs_path = payload.path.clone();
    cmds.push(move |world: &mut World| spawn_html_template(world, abs_path));
}

fn spawn_html_template(world: &mut World, abs_path: PathBuf) {
    // Shared spawn: a draggable, absolutely-positioned instance under a canvas
    // (path stored project-relative; markup built under a child HtmlNode).
    let instance = renzora_game_ui::spawn::spawn_html_template_at(world, &abs_path, None);
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(instance));
    }
}
