//! Camera rehydration, active-camera arbitration, and the editor's
//! effects-mirroring bridge.
//!
//! Who renders depends on which build this is, and getting it wrong is
//! expensive rather than broken: an over-eager `is_active` renders the whole
//! scene a second time behind the editor chrome, invisible but costing about a
//! third of the frame. [`enforce_single_active_camera`] is the pin that stops
//! that, and it deliberately keys off "is an editor camera present" rather than
//! a cargo feature — see the note inside [`rehydrate_cameras`].

use bevy::camera::Hdr;
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::ecs::world::FilteredEntityRef;
use bevy::light::AtmosphereEnvironmentMapLight;
#[cfg(feature = "render_3d")]
use bevy::pbr::AtmosphereSettings;
use bevy::prelude::*;
use renzora::{
    DefaultCamera, EditorCamera, PlayModeCamera, PlayModeState, SceneCamera, ViewportRenderTarget,
};
use std::collections::BTreeSet;

/// Ensure parent entities have `Visibility` so transform/visibility propagation works.
/// Fixes groups/empty parents that were saved without `Visibility`.
pub fn rehydrate_visibility(
    mut commands: Commands,
    query: Query<Entity, (With<Children>, Without<Visibility>)>,
) {
    for entity in &query {
        commands.entity(entity).try_insert(Visibility::default());
    }
}

/// Rehydrate scene cameras — spawns `Camera3d` for entities that have `SceneCamera` but no `Camera3d`.
///
/// In runtime mode (no editor), the `DefaultCamera` is active; if none is marked,
/// the first scene camera wins. All others are inactive.
/// In editor mode, all scene cameras are inactive (the editor camera renders).
/// In play mode, the default camera becomes the active play mode camera with the
/// viewport render target.
///
/// `Without<Camera2d>` is critical: `Camera2d` and `Camera3d` are mutually
/// exclusive markers and stacking them on the same entity makes Bevy pick
/// the 3D pipeline, breaking sprite rendering. Authored 2D scene cameras
/// stay 2D-only.
pub fn rehydrate_cameras(
    mut commands: Commands,
    query: Query<
        (Entity, Option<&DefaultCamera>),
        (With<SceneCamera>, Without<Camera3d>, Without<Camera2d>),
    >,
    play_mode: Option<Res<PlayModeState>>,
    render_target: Option<Res<ViewportRenderTarget>>,
    editor_session: Option<Res<renzora::EditorSession>>,
    quality: Option<Res<renzora::ResolvedGraphicsQuality>>,
) {
    if query.is_empty() {
        return;
    }

    // Atmosphere-derived IBL probe face size. Bevy re-bakes and re-prefilters
    // this cubemap EVERY frame with no dirty check, so its cost (≈75 M cube
    // fetches/frame at the 512² default — the single largest scene-independent
    // GPU cost) scales with the square of the face size. A blurry procedural-sky
    // reflection needs far less; the tier drops it to 256/128/64. Kept in lockstep
    // with `renzora_environment_map::sync_environment_map`, which rewrites this
    // component every frame — a mismatched size there would re-allocate the probe
    // texture per frame.
    let ibl_face = quality.as_ref().map(|q| q.0.ibl_face_size()).unwrap_or(128);

    let in_play_mode = play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode());
    // We're in editor mode when the runtime `EditorSession` flag is set.
    //
    // This used to read `cfg!(feature = "editor")`, but after the
    // editor/runtime crate split `renzora_engine` compiles lean (no
    // `editor` cargo feature), so that flag is now ALWAYS false here —
    // which made rehydrate treat the editor like a runtime export and set
    // `is_active: true` on the default scene camera. Combined with a
    // play→stop cycle (which strips `Camera3d`, so the `Without<Camera3d>`
    // filter re-matches), that reactivated the scene camera to render the
    // whole scene to the primary window BEHIND the editor chrome — a
    // second full-scene pass on top of the editor camera, ~33% frame-time
    // regression with shadows on.
    //
    // `EditorSession` is inserted at boot by `add_engine_plugins(is_editor)`
    // (the same signal `RuntimePlugin` branches on), so it's already correct
    // when rehydrate runs during `SplashState::Loading` — no editor-camera
    // spawn race. Absent ⇒ game build ⇒ `false` (the safe shipping default).
    let is_editor = editor_session.as_ref().map(|s| s.0).unwrap_or(false) && !in_play_mode;

    // Find which entity should be the active camera in runtime mode
    let default_entity = query
        .iter()
        .find(|(_, dc)| dc.is_some())
        .or_else(|| query.iter().next())
        .map(|(e, _)| e);

    for (entity, _) in &query {
        let is_active = !is_editor && default_entity == Some(entity);

        commands.entity(entity).try_insert((
            Camera3d::default(),
            Camera {
                is_active,
                ..default()
            },
        ));

        // When this scene camera will be the active runtime camera (i.e.
        // we're not in editor mode and this is the default camera), it
        // needs the full prepass + atmosphere stack the editor camera
        // has. `Msaa::Off` is required because atmosphere binds depth
        // as non-multisampled. `DeferredPrepass` is added separately by
        // `ensure_deferred_prepass_on_cameras` (the single source of
        // truth for the Forward/Deferred toggle across editor, play
        // mode, and runtime) — that's why it's not in this tuple.
        //
        // Mirrors `renzora_viewport::play_mode::enter_play_mode` for the
        // 3D-only setup. (See `renzora_engine::camera::spawn_editor_camera`
        // for why all other prepass markers must be attached at spawn.)
        if is_active {
            commands.entity(entity).try_insert((
                Hdr,
                NormalPrepass,
                DepthPrepass,
                MotionVectorPrepass,
                AtmosphereEnvironmentMapLight {
                    intensity: 0.0,
                    size: UVec2::splat(ibl_face),
                    ..default()
                },
                // ContactShadows intentionally omitted — see camera.rs (bevy 0.19
                // deferred + area_light_luts bind-group-layout conflict).
                Msaa::Off,
            ));
            // Per-view atmosphere render mode — bevy_pbr, render_3d only.
            #[cfg(feature = "render_3d")]
            commands
                .entity(entity)
                .insert(AtmosphereSettings::default());
            // NOTE: no `Atmosphere` entity is spawned here. `renzora_atmosphere`
            // owns the sky: `sync_atmosphere` maintains exactly one
            // `AtmospherePlanet` entity, spawns it when missing, and drives its
            // medium from the World Environment's enabled state.
            //
            // This used to spawn its own "Sky Atmosphere" — a second entity
            // carrying `Atmosphere` but neither `AtmospherePlanet` nor
            // `HideInHierarchy`. 0.19 re-extracts `Atmosphere` from every entity
            // that has one, so the two skies fought, and the duplicate was
            // unmanaged: `sync_atmosphere` only ever touches the entity it owns,
            // so the copy kept a default medium and ignored the environment's
            // on/off state entirely.
            //
            // It was also lifetime-coupled to the camera respawning with the
            // scene ("a scene clear recycles it"). A camera in a global/autoload
            // scene is `Persistent`, so it never re-enters this query's
            // `Without<Camera3d>` filter — the duplicate got scene-cleared once
            // and never came back.
        }

        // During play mode, configure the default camera as the play mode camera
        // with the viewport render target (mirrors what enter_play_mode does).
        if in_play_mode && is_active {
            use renzora::console_log::*;
            let name = commands.entity(entity).id();
            console_info(
                "Rehydration",
                format!(
                    "Play mode active — configuring {:?} as play mode camera",
                    name
                ),
            );
            commands.entity(entity).try_insert(PlayModeCamera);
            if let Some(img) = render_target.as_ref().and_then(|rt| rt.image.as_ref()) {
                commands
                    .entity(entity)
                    .try_insert(bevy::camera::RenderTarget::Image(
                        Handle::<Image>::clone(img).into(),
                    ));
            }
        }
    }
}

/// Keeps `PlayModeState.active_game_camera` in sync when a scene transition
/// during play mode spawns a new `PlayModeCamera` entity.
pub fn sync_play_mode_camera(
    query: Query<Entity, Added<PlayModeCamera>>,
    play_mode: Option<ResMut<PlayModeState>>,
) {
    let Some(mut play_mode) = play_mode else {
        return;
    };
    for entity in &query {
        if play_mode.active_game_camera != Some(entity) {
            renzora::console_log::console_info(
                "PlayMode",
                format!(
                    "Play mode camera updated: {:?} -> {:?}",
                    play_mode.active_game_camera, entity
                ),
            );
            play_mode.active_game_camera = Some(entity);
        }
    }
}

/// Ensures only the default scene camera is active in runtime mode.
///
/// Runs every frame (cheap — early-exits if no changes). Handles cameras that
/// were deserialized with `Camera3d` already present (so [`rehydrate_cameras`] skipped them).
pub fn enforce_single_active_camera(
    mut cameras: Query<(Entity, &mut Camera, Option<&DefaultCamera>), With<SceneCamera>>,
    editor_camera: Query<(), With<EditorCamera>>,
    play_mode: Option<Res<PlayModeState>>,
) {
    if cameras.is_empty() {
        return;
    }

    let _ = play_mode;
    // Whenever the editor is running, scene cameras stay inactive — INCLUDING
    // in-panel play mode. Play renders through the editor cameras
    // (`drive_editor_camera_in_play` mirrors the game camera's pose onto
    // them), so an active scene camera would render the entire game a second
    // time straight to the window, invisible behind the editor UI. Only the
    // shipped runtime (no editor camera) activates the default scene camera.
    let in_editor = !editor_camera.is_empty();

    if in_editor {
        // Editor view: every scene camera should be inactive — the
        // editor camera owns the viewport. Without this pin, the
        // SceneCamera authored in `main.ron` ends up `is_active: true`
        // because Bevy auto-inserts a default `Camera` (which defaults
        // to active) when the scene reflects in its required-component
        // graph, and `try_insert` in `rehydrate_cameras` doesn't
        // always win that race. With this enforced, the whole scene
        // stops rendering twice in editor — recovers ~33% frame time
        // in heavy scenes with shadows on.
        for (_, mut camera, _) in &mut cameras {
            if camera.is_active {
                camera.is_active = false;
            }
        }
        return;
    }

    // Runtime / play mode: pick the DefaultCamera (or first scene
    // camera) and make sure only it is active.
    let default_entity = cameras
        .iter()
        .find(|(_, _, dc)| dc.is_some())
        .or_else(|| cameras.iter().next())
        .map(|(e, _, _)| e);

    for (entity, mut camera, _) in &mut cameras {
        let should_be_active = default_entity == Some(entity);
        if camera.is_active != should_be_active {
            camera.is_active = should_be_active;
        }
    }
}

fn should_sync(type_path: &str) -> bool {
    type_path.ends_with("Settings")
}

/// Tracks the previous sync state so we only log when something changes.
#[derive(Resource, Default)]
struct SceneCameraSyncState {
    prev_src: Option<Entity>,
    prev_synced: BTreeSet<&'static str>,
}

/// Sync post-process (and other reflected) components from the **default**
/// SceneCamera entity to the EditorCamera.
///
/// In editor mode the viewport renders through the EditorCamera, but users attach
/// effects to the SceneCamera entity. This system mirrors those components so they
/// take effect during editing.
///
/// Skipped during play mode (the play-mode camera receives effects via
/// `RenderTarget` + the individual `sync_*` systems).
pub fn sync_scene_camera_to_editor_camera(world: &mut World) {
    // Skip during play mode — effects route through RenderTarget instead.
    let is_playing = world
        .get_resource::<PlayModeState>()
        .is_some_and(|pm| pm.is_in_play_mode());
    if is_playing {
        return;
    }

    // Find the editor camera.
    let mut q = world.query_filtered::<Entity, With<EditorCamera>>();
    let editor_cam = q.iter(world).next();
    let Some(dst) = editor_cam else {
        return;
    };

    // Find the scene camera — prefer DefaultCamera, fall back to first SceneCamera.
    let mut default_cam = None;
    let mut first_cam = None;
    let mut q = world.query_filtered::<(Entity, Option<&DefaultCamera>), With<SceneCamera>>();
    for (e, dc) in q.iter(world) {
        if dc.is_some() {
            default_cam = Some(e);
            break;
        }
        if first_cam.is_none() {
            first_cam = Some(e);
        }
    }
    let scene_cam = default_cam.or(first_cam);

    // If no scene camera exists, remove all synced components from the editor camera.
    let Some(src) = scene_cam else {
        let type_registry = world.resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let mut to_remove: Vec<bevy::ecs::reflect::ReflectComponent> = Vec::new();
        let editor_ref = world.entity(dst);
        for reg in registry.iter() {
            let Some(reflect_component) = reg.data::<bevy::ecs::reflect::ReflectComponent>() else {
                continue;
            };
            let type_path = reg.type_info().type_path();
            if !should_sync(type_path) {
                continue;
            }
            if reflect_component.contains(FilteredEntityRef::from(editor_ref)) {
                to_remove.push(reflect_component.clone());
            }
        }
        drop(registry);
        for reflect_component in &to_remove {
            reflect_component.remove(&mut world.entity_mut(dst));
        }
        return;
    };

    if src == dst {
        return;
    }

    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();

    // Collect reflected component data from the scene camera.
    let mut components_to_sync: Vec<(bevy::ecs::reflect::ReflectComponent, Box<dyn Reflect>)> =
        Vec::new();
    let mut synced_type_paths: Vec<&'static str> = Vec::new();

    let entity_ref = world.entity(src);
    for reg in registry.iter() {
        let Some(reflect_component) = reg.data::<bevy::ecs::reflect::ReflectComponent>() else {
            continue;
        };
        let type_path = reg.type_info().type_path();
        if !should_sync(type_path) {
            continue;
        }
        if let Some(reflected) = reflect_component.reflect(FilteredEntityRef::from(entity_ref)) {
            if let Ok(cloned) = reflected.reflect_clone() {
                components_to_sync.push((reflect_component.clone(), cloned));
                synced_type_paths.push(type_path);
            }
        }
    }
    drop(registry);

    // Apply collected components to the editor camera.
    {
        let registry = type_registry.read();
        for (reflect_component, value) in &components_to_sync {
            let mut entity_mut = world.entity_mut(dst);
            if reflect_component.contains(entity_mut.as_readonly()) {
                reflect_component.apply(entity_mut, value.as_partial_reflect());
            } else {
                reflect_component.insert(&mut entity_mut, value.as_partial_reflect(), &registry);
            }
        }
    }

    // Remove components from editor camera that were removed from scene camera.
    let registry = type_registry.read();
    let mut to_remove: Vec<(bevy::ecs::reflect::ReflectComponent, &'static str)> = Vec::new();
    let editor_ref = world.entity(dst);
    for reg in registry.iter() {
        let Some(reflect_component) = reg.data::<bevy::ecs::reflect::ReflectComponent>() else {
            continue;
        };
        let type_path = reg.type_info().type_path();
        if !should_sync(type_path) {
            continue;
        }
        if reflect_component.contains(FilteredEntityRef::from(editor_ref))
            && !synced_type_paths.contains(&type_path)
        {
            to_remove.push((reflect_component.clone(), type_path));
        }
    }
    drop(registry);

    // Only log when the source camera or set of synced types actually changes.
    let current_set: BTreeSet<&'static str> = synced_type_paths.iter().copied().collect();
    let removed_paths: Vec<&str> = to_remove.iter().map(|(_, p)| *p).collect();

    let mut state = world
        .remove_resource::<SceneCameraSyncState>()
        .unwrap_or_default();

    let src_changed = state.prev_src != Some(src);
    let set_changed = state.prev_synced != current_set;
    let has_removals = !removed_paths.is_empty();

    if src_changed || set_changed || has_removals {
        crate::debug_log::log_scene_camera_sync(
            Some(src),
            Some(dst),
            &synced_type_paths,
            &removed_paths,
        );
        if src_changed {
            let src_name = world
                .get::<Name>(src)
                .map(|n| n.to_string())
                .unwrap_or_else(|| "unnamed".into());
            let has_default = world.get::<DefaultCamera>(src).is_some();
            renzora::console_log::console_info(
                "PostProcess",
                format!(
                    "Sync source camera: {:?} \"{}\" default={}",
                    src, src_name, has_default
                ),
            );
        }
        state.prev_src = Some(src);
        state.prev_synced = current_set;
    }

    world.insert_resource(state);

    for (reflect_component, _) in &to_remove {
        reflect_component.remove(&mut world.entity_mut(dst));
    }
}
