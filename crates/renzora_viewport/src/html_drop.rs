//! Drag-and-drop HTML template spawning — when a `.html` asset is dropped on the
//! viewport, spawn a UI Canvas with a child `HtmlTemplatePath` entity (the same
//! shape as the "+ Add Entity → HTML Template" preset). The runtime observer in
//! `renzora_hui` turns the path into an `HtmlNode` and bevy_hui builds the markup
//! beneath it. UI is screen-space, so the 3D drop point is ignored.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::egui;

use renzora_editor::{EditorCommands, EditorSelection};
use renzora_ui::asset_drag::AssetDragPayload;

const HTML_EXTENSIONS: &[&str] = &["html"];

/// Native (bevy_ui viewport) counterpart of [`check_viewport_html_drop`]: on
/// release of an `.html` asset-drag over the native viewport, spawn the UI
/// template. The native viewport overrides the egui panel body, so the egui
/// `check_viewport_html_drop` no longer runs — this is the only handler then.
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

/// Called from the viewport panel's `ui()` each frame. On release of an `.html`
/// drag-drop payload over the viewport, queues a deferred command that spawns a
/// UI Canvas + template-carrying child.
pub fn check_viewport_html_drop(ui: &mut egui::Ui, world: &World, viewport_rect: egui::Rect) {
    let Some(payload) = world.get_resource::<AssetDragPayload>() else {
        return;
    };
    if !payload.is_detached || !payload.matches_extensions(HTML_EXTENSIONS) {
        return;
    }

    let pointer_pos = ui.ctx().pointer_latest_pos();
    if !pointer_pos.is_some_and(|p| viewport_rect.contains(p)) {
        return;
    }
    // Wait for the pointer to be released over the viewport.
    if ui.ctx().input(|i| i.pointer.any_down()) {
        return;
    }

    let abs_path = payload.path.clone();
    if let Some(commands) = world.get_resource::<EditorCommands>() {
        commands.push(move |world: &mut World| spawn_html_template(world, abs_path));
    }
}

fn spawn_html_template(world: &mut World, abs_path: PathBuf) {
    // Shared spawn: a draggable, absolutely-positioned instance under a canvas
    // (path stored project-relative; markup built under a child HtmlNode).
    let instance = renzora_game_ui::spawn::spawn_html_template_at(world, &abs_path, None);
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(instance));
    }
}
