//! Drag-and-drop HTML template spawning — when a `.html` asset is dropped on the
//! viewport, spawn a `UiCanvas` carrying that template in its `HtmlTemplatePath`
//! field (the unified "template lives on a canvas" shape). The runtime pipeline
//! builds the markup as the canvas's content; the scene hierarchy shows just the
//! clean canvas. UI is screen-space by default (flip its Render Space to world for
//! a 3D plane), so the 3D drop point is ignored.

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
    // Drop → a clean `UiCanvas` with the template stored in its `HtmlTemplatePath`
    // field (no nested instance/wrapper). Select the canvas so its inspector — with
    // the Template row and the Render Space (screen/world) dropdown — is right there.
    let canvas = renzora_ember::game_ui::spawn::spawn_ui_canvas_with_template(world, &abs_path);
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(canvas));
    }
}
