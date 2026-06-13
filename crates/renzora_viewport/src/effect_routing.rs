//! Builds the [`EffectRouting`] table each frame based on editor state.
//!
//! Routes:
//! - **Editing**: EditorCamera ← [DefaultCamera entity, all non-camera entities]
//! - **Editing + preview**: CameraPreviewMarker ← [previewing entity, all non-camera entities]
//! - **Play mode**: PlayModeCamera ← [play mode source camera, all non-camera entities]
//!
//! Note: the UI authoring viewport mode reuses the editor camera as its
//! 3D backdrop (see `renzora_game_ui::canvas`), so it inherits the
//! EditorCamera route — no dedicated UI canvas preview camera exists.

use bevy::prelude::*;
use renzora::core::{
    DefaultCamera, EffectRouting, HideInHierarchy, PlayModeCamera, PlayModeState, SceneCamera,
    ViewportCamera,
};

use crate::camera_preview::{CameraPreviewMarker, CameraPreviewState};

/// Query alias for entities that can act as "global" effect sources — 3D scene
/// entities that might carry a `*Settings` component (e.g. the World
/// Environment entity).
///
/// Exclusions, all of which are things that never hold effect settings but DO
/// churn every frame (which rebuilt the routing table — and re-ran every
/// effect's sync — each frame, and even fed back on itself):
/// - `Without<Node>` — `bevy_ui` entities. Editor panels spawn named UI nodes
///   constantly; notably each **console log row** is a named `Node`, so logging
///   the routing rebuild spawned a row, which bumped the source count, which
///   rebuilt the routing, which logged again…
/// - `Without<HideInHierarchy>` — non-UI editor chrome (gizmos, preview rigs).
type GlobalSourceQuery<'w, 's> = Query<
    'w,
    's,
    Entity,
    (
        Without<Camera>,
        With<Name>,
        Without<HideInHierarchy>,
        Without<Node>,
    ),
>;

/// Collects the global effect-source entities, sorted by a stable key.
///
/// Sorting makes the comparison in [`update_effect_routing`] order-independent
/// (a query's iteration order shuffles when any matched entity changes
/// archetype), so the routing table only rebuilds when the source *set* changes
/// rather than every frame.
fn collect_global_sources(non_camera_entities: &GlobalSourceQuery) -> Vec<Entity> {
    let mut sources: Vec<Entity> = non_camera_entities.iter().collect();
    sources.sort_unstable_by_key(|e| e.to_bits());
    sources
}

/// Populates [`EffectRouting`] based on current editor/play mode state.
pub fn update_effect_routing(
    mut routing: ResMut<EffectRouting>,
    play_mode: Option<Res<PlayModeState>>,
    viewport_cameras: Query<(Entity, &ViewportCamera)>,
    scene_cameras: Query<(Entity, Option<&DefaultCamera>), With<SceneCamera>>,
    preview_cameras: Query<Entity, With<CameraPreviewMarker>>,
    preview_state: Option<Res<CameraPreviewState>>,
    play_mode_cameras: Query<Entity, With<PlayModeCamera>>,
    non_camera_entities: GlobalSourceQuery,
) {
    let mut routes: Vec<(Entity, Vec<Entity>)> = Vec::new();
    let global_sources = collect_global_sources(&non_camera_entities);

    let is_play_mode = play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode());

    if is_play_mode {
        // Play mode: route to the PlayModeCamera
        if let Ok(play_cam) = play_mode_cameras.single() {
            // Find the source scene camera (stored in PlayModeState)
            let source_cam = play_mode.as_ref().and_then(|pm| pm.active_game_camera);

            let mut sources = Vec::new();
            if let Some(src) = source_cam {
                sources.push(src);
            }
            sources.extend(&global_sources);
            routes.push((play_cam, sources));
        }
    } else {
        // Editing mode: route the same sources to EVERY viewport camera so all
        // effects fan out to all views automatically. Each effect's sync system
        // decides what to do per target: post-process effects (bloom, AO, fog,
        // tonemapping, …) insert themselves per-camera at runtime, while the
        // spawn-time-fragile atmosphere/IBL only *update* the one camera that
        // already carries them (the bake camera) and share their result to the
        // rest via a Skybox. This is the generic mechanism — a new effect that
        // routes through `EffectRouting` is shared across all views for free.
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

        // Sort the target cameras too: the viewport-camera query order shuffles
        // when the env-probe gating toggles a component on the editor camera,
        // which would otherwise reorder `routes` and trip `is_changed()` every
        // frame even though the routing is unchanged.
        let mut cams: Vec<Entity> = viewport_cameras.iter().map(|(e, _)| e).collect();
        cams.sort_unstable_by_key(|e| e.to_bits());
        for cam in cams {
            routes.push((cam, sources.clone()));
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
        routes.push((preview_cam, sources));
    }

    if routing.routes != routes {
        routing.routes = routes;
    }
}
