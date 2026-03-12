//! Builds the [`EffectRouting`] table each frame based on editor state.
//!
//! Routes:
//! - **Editing**: EditorCamera ← [DefaultCamera entity, all non-camera entities]
//! - **Editing + preview**: CameraPreviewMarker ← [previewing entity, all non-camera entities]
//! - **UI canvas preview**: UiCanvasPreviewCamera ← same sources as camera preview
//! - **Play mode**: PlayModeCamera ← [play mode source camera, all non-camera entities]

use bevy::prelude::*;
use renzora_core::{
    DefaultCamera, EditorCamera, EffectRouting, PlayModeCamera, PlayModeState, SceneCamera,
    UiCanvasPreviewCamera,
};

use crate::camera_preview::{CameraPreviewMarker, CameraPreviewState};

/// Collects all non-camera entities that might hold Settings components.
/// These are "global" effect sources (e.g. World Environment entity).
fn collect_global_sources(
    non_camera_entities: &Query<Entity, (Without<Camera>, With<Name>)>,
) -> Vec<Entity> {
    non_camera_entities.iter().collect()
}

/// Populates [`EffectRouting`] based on current editor/play mode state.
pub fn update_effect_routing(
    mut routing: ResMut<EffectRouting>,
    play_mode: Option<Res<PlayModeState>>,
    editor_cameras: Query<Entity, With<EditorCamera>>,
    scene_cameras: Query<(Entity, Option<&DefaultCamera>), With<SceneCamera>>,
    preview_cameras: Query<Entity, With<CameraPreviewMarker>>,
    preview_state: Option<Res<CameraPreviewState>>,
    play_mode_cameras: Query<Entity, With<PlayModeCamera>>,
    canvas_preview_cameras: Query<Entity, With<UiCanvasPreviewCamera>>,
    non_camera_entities: Query<Entity, (Without<Camera>, With<Name>)>,
) {
    let mut routes: Vec<(Entity, Vec<Entity>)> = Vec::new();
    let global_sources = collect_global_sources(&non_camera_entities);

    let is_play_mode = play_mode
        .as_ref()
        .is_some_and(|pm| pm.is_in_play_mode());

    if is_play_mode {
        // Play mode: route to the PlayModeCamera
        if let Ok(play_cam) = play_mode_cameras.single() {
            // Find the source scene camera (stored in PlayModeState)
            let source_cam = play_mode
                .as_ref()
                .and_then(|pm| pm.active_game_camera);

            let mut sources = Vec::new();
            if let Some(src) = source_cam {
                sources.push(src);
            }
            sources.extend(&global_sources);
            routes.push((play_cam, sources));
        }
    } else {
        // Editing mode: route to EditorCamera from default scene camera + globals
        if let Ok(editor_cam) = editor_cameras.single() {
            // Find default scene camera, fall back to first
            let default_cam = scene_cameras
                .iter()
                .find(|(_, dc)| dc.is_some())
                .map(|(e, _)| e)
                .or_else(|| scene_cameras.iter().next().map(|(e, _)| e));

            let mut sources = Vec::new();
            if let Some(src) = default_cam {
                sources.push(src);
            }
            sources.extend(&global_sources);
            routes.push((editor_cam, sources));
        }
    }

    // Preview camera: always route from the previewing entity + globals
    if let Ok(preview_cam) = preview_cameras.single() {
        let previewing = preview_state.as_ref().and_then(|ps| ps.previewing);

        let mut sources = Vec::new();
        if let Some(src) = previewing {
            sources.push(src);
        }
        sources.extend(&global_sources);
        routes.push((preview_cam, sources.clone()));

        // UI canvas preview camera: same sources as the camera preview
        if let Ok(canvas_cam) = canvas_preview_cameras.single() {
            routes.push((canvas_cam, sources));
        }
    }

    if routing.routes != routes {
        routing.routes = routes;
    }
}
