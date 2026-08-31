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

/// Open a `.html` template requested from somewhere other than a drag — the
/// asset browser's double-click, via `renzora::core::OpenUiTemplateFile`.
///
/// Re-selects an existing canvas for that template rather than spawning a
/// second one. Opening the same file twice is the common case (you closed the
/// tab, you came back), and two canvases for one template both write back to it.
pub fn open_ui_template_request(
    request: Option<Res<renzora::core::OpenUiTemplateFile>>,
    cmds: Option<Res<EditorCommands>>,
    mut commands: Commands,
) {
    let (Some(request), Some(cmds)) = (request, cmds) else { return };
    let abs_path = request.path.clone();
    commands.remove_resource::<renzora::core::OpenUiTemplateFile>();
    cmds.push(move |world: &mut World| {
        if let Some(existing) = find_canvas_for_template(world, &abs_path) {
            if let Some(sel) = world.get_resource::<EditorSelection>() {
                sel.set(Some(existing));
            }
            return;
        }
        spawn_html_template(world, abs_path);
    });
}

/// The canvas already showing `path`, if there is one.
///
/// Compares against the project-relative form the canvas stores as well as the
/// absolute path it was opened with — the two entry points disagree about which
/// they hold, and a mismatch here means a duplicate canvas rather than a
/// selection.
fn find_canvas_for_template(world: &mut World, abs_path: &std::path::Path) -> Option<Entity> {
    let rel = world
        .get_resource::<renzora::core::CurrentProject>()
        .and_then(|p| p.make_relative(abs_path));
    let mut q = world.query::<(Entity, &renzora_ember::game_ui::HtmlTemplatePath)>();
    q.iter(world)
        .find(|(_, t)| {
            let stored = t.0.replace('\\', "/");
            rel.as_deref() == Some(stored.as_str())
                || abs_path.to_string_lossy().replace('\\', "/") == stored
        })
        .map(|(e, _)| e)
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
