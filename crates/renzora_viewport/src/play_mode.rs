//! Play mode — switches from editor camera to game camera for in-editor playtesting.

use bevy::camera::RenderTarget;
use bevy::core_pipeline::prepass::{
    DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass,
};
use bevy::light::AtmosphereEnvironmentMapLight;
use bevy::light::atmosphere::ScatteringMedium;
use bevy::light::Atmosphere;
use bevy::pbr::AtmosphereSettings;
use bevy::prelude::*;
use bevy::camera::Hdr;
use bevy::window::{CursorGrabMode, CursorOptions};
use renzora::core::{
    DefaultCamera, EditorCamera, EditorCamera2d, PlayModeCamera, PlayModeState, PlayState,
    SceneCamera, ViewportRenderTarget,
};
use renzora_editor_framework::camera::EditorUiCamera;
use renzora_editor_framework::EditorSettings;

use crate::external_runtime::{
    self, find_runtime_binary, replace_child, spawn_runtime, ExternalRuntime,
};

/// Handles play mode transitions each frame.
pub fn handle_play_mode_transitions(world: &mut World) {
    // Try the external-runtime path first. If the user has the
    // "external_play_window" setting on AND the runtime binary is
    // discoverable, route Play/Stop to spawning/killing the child instead
    // of doing the in-editor camera switch. The editor's own
    // `PlayModeState` stays in `Editing` while the runtime owns the game.
    //
    // If the binary isn't discoverable (e.g. `cargo run` from a workspace
    // with no `dist/{platform}/runtime/` sibling), this returns false and
    // we fall through to in-editor play, so Play always does *something*.
    if try_handle_external_runtime(world) {
        return;
    }

    let Some(mut play_mode) = world.remove_resource::<PlayModeState>() else {
        return;
    };

    if play_mode.request_play && play_mode.is_editing() {
        play_mode.request_play = false;
        enter_play_mode(world, &mut play_mode);
    } else if play_mode.request_stop && play_mode.is_in_play_mode() {
        play_mode.request_stop = false;
        exit_play_mode(world, &mut play_mode);
    } else if play_mode.request_pause {
        play_mode.request_pause = false;
        match play_mode.state {
            PlayState::Playing => play_mode.state = PlayState::Paused,
            PlayState::Paused => play_mode.state = PlayState::Playing,
            _ => {}
        }
    } else {
        play_mode.request_play = false;
        play_mode.request_stop = false;
        play_mode.request_pause = false;
    }

    world.insert_resource(play_mode);
}

/// External-runtime mode handler. Returns true if it consumed the
/// play/stop request — caller should bail out without falling through to
/// the in-editor path. Returns false if the feature is off or
/// inapplicable (no binary found, no project loaded, no pending request).
fn try_handle_external_runtime(world: &mut World) -> bool {
    use renzora::core::console_log::*;

    // Feature gate: only act when the user opted in.
    let enabled = world
        .get_resource::<EditorSettings>()
        .map(|s| s.external_play_window)
        .unwrap_or(false);
    if !enabled {
        return false;
    }

    // Inspect the request flags before mutating anything else.
    let Some(play_mode) = world.get_resource::<PlayModeState>() else {
        return false;
    };
    let runtime_alive = world
        .get_resource::<ExternalRuntime>()
        .map(|r| r.is_alive())
        .unwrap_or(false);
    let pending_play = play_mode.request_play && play_mode.is_editing();
    let pending_stop = play_mode.request_stop;
    // While a child runtime is alive, *every* play/stop request collapses
    // to "stop." That keeps the title bar's Play button (which doesn't
    // know about external runtime state) from accidentally spawning a
    // second instance.
    let want_stop = runtime_alive && (pending_play || pending_stop);
    let want_play = !runtime_alive && pending_play;

    if !want_play && !want_stop {
        return false;
    }

    if want_stop {
        if let Some(mut runtime) = world.get_resource_mut::<ExternalRuntime>() {
            if external_runtime::kill_runtime(&mut runtime) {
                console_info("PlayMode", "External runtime killed");
            }
        }
        if let Some(mut pm) = world.get_resource_mut::<PlayModeState>() {
            pm.request_stop = false;
            pm.request_play = false;
        }
        return true;
    }

    // want_play — try to locate and spawn the runtime. If we can't, log
    // and let the caller fall through to the in-editor path so Play still
    // does something.
    let Some(binary) = find_runtime_binary() else {
        console_info(
            "PlayMode",
            "External play requested but runtime binary not found — falling back to in-editor play",
        );
        return false;
    };

    let project_path = world
        .get_resource::<renzora::CurrentProject>()
        .map(|p| p.path.clone());
    let Some(project_path) = project_path else {
        console_error(
            "PlayMode",
            "External play requested but no project is loaded",
        );
        // Eat the request so we don't repeatedly attempt this — the user
        // hasn't loaded a project, the in-editor path can't help either.
        if let Some(mut pm) = world.get_resource_mut::<PlayModeState>() {
            pm.request_play = false;
        }
        return true;
    };

    // Save scene before launching, mirroring `enter_play_mode`. The
    // runtime reads from disk, so unsaved edits would otherwise be
    // invisible to it.
    world.trigger(renzora::core::SaveCurrentScene);

    console_info(
        "PlayMode",
        format!(
            "Spawning external runtime: {} --project {}",
            binary.display(),
            project_path.display()
        ),
    );

    match spawn_runtime(&binary, &project_path) {
        Ok(child) => {
            if let Some(mut runtime) = world.get_resource_mut::<ExternalRuntime>() {
                replace_child(&mut runtime, child);
                // Raise the "Preparing export runtime" overlay and pause the
                // editor until the runtime window closes.
                runtime.begin_preparing();
            }
            if let Some(mut pm) = world.get_resource_mut::<PlayModeState>() {
                pm.request_play = false;
            }
            true
        }
        Err(e) => {
            console_error(
                "PlayMode",
                format!("Failed to spawn runtime: {} — falling back", e),
            );
            // Don't eat the request; let in-editor play take over.
            false
        }
    }
}

fn enter_play_mode(world: &mut World, play_mode: &mut PlayModeState) {
    use renzora::core::console_log::*;
    console_info("PlayMode", "=== ENTERING PLAY MODE ===");

    // Save scene before entering play mode (observed by renzora_engine)
    world.trigger(renzora::core::SaveCurrentScene);
    console_info("PlayMode", "Scene saved before play mode");

    // Find the game camera: prefer DefaultCamera, then first SceneCamera
    let mut q = world.query_filtered::<(Entity, Option<&DefaultCamera>), With<SceneCamera>>();
    let candidates: Vec<(Entity, bool)> = q
        .iter(world)
        .map(|(e, dc): (Entity, Option<&DefaultCamera>)| (e, dc.is_some()))
        .collect();

    console_info(
        "PlayMode",
        format!("Scene camera candidates: {}", candidates.len()),
    );
    for (e, is_default) in &candidates {
        let name = world
            .get::<Name>(*e)
            .map(|n| n.to_string())
            .unwrap_or_else(|| "unnamed".into());
        console_info(
            "PlayMode",
            format!("  {:?} \"{}\" default={}", e, name, is_default),
        );
    }

    let game_camera = candidates
        .iter()
        .find(|(_, is_default)| *is_default)
        .map(|(e, _)| *e)
        .or_else(|| candidates.first().map(|(e, _)| *e));

    let Some(cam_entity) = game_camera else {
        console_error("PlayMode", "No scene camera found — cannot enter play mode");
        warn!("Play mode: no scene camera found");
        return;
    };

    let cam_name = world
        .get::<Name>(cam_entity)
        .map(|n| n.to_string())
        .unwrap_or_else(|| "unnamed".into());
    let is_2d_camera = world.get::<Camera2d>(cam_entity).is_some();
    console_info(
        "PlayMode",
        format!(
            "Selected game camera: {:?} \"{}\" mode={}",
            cam_entity,
            cam_name,
            if is_2d_camera { "2D" } else { "3D" }
        ),
    );

    // Disable both 3D and 2D editor cameras — either could be active
    // depending on which view the user was in when they hit Play.
    let mut editor_q = world.query_filtered::<Entity, With<EditorCamera>>();
    let editor_entities: Vec<Entity> = editor_q.iter(world).collect();
    let mut editor_2d_q = world.query_filtered::<Entity, With<EditorCamera2d>>();
    let editor_2d_entities: Vec<Entity> = editor_2d_q.iter(world).collect();
    for entity in editor_entities.iter().chain(editor_2d_entities.iter()) {
        console_info("PlayMode", format!("Disabling editor camera {:?}", entity));
    }
    for entity in editor_entities.iter().chain(editor_2d_entities.iter()) {
        if let Some(mut camera) = world.get_mut::<Camera>(*entity) {
            camera.is_active = false;
        }
    }

    // Set up game camera for rendering
    let had_cam3d = world.get::<Camera3d>(cam_entity).is_some();
    let had_camera = world.get::<Camera>(cam_entity).is_some();
    console_info(
        "PlayMode",
        format!(
            "Game camera {:?} state before setup: Camera3d={} Camera2d={} Camera={}",
            cam_entity, had_cam3d, is_2d_camera, had_camera
        ),
    );

    // Only force-insert Camera3d for 3D play. A Camera2d already has its
    // own pipeline; adding Camera3d would attempt to attach the 3D bind
    // group on top and crash wgpu.
    if !is_2d_camera && !had_cam3d {
        world.entity_mut(cam_entity).insert(Camera3d::default());
        console_info("PlayMode", format!("Inserted Camera3d on {:?}", cam_entity));
    }

    if !had_camera {
        world.entity_mut(cam_entity).insert(Camera::default());
        console_info("PlayMode", format!("Inserted Camera on {:?}", cam_entity));
    }

    if let Some(mut cam) = world.get_mut::<Camera>(cam_entity) {
        cam.is_active = true;
        cam.order = 0;
        console_info(
            "PlayMode",
            format!("Configured camera {:?}: active=true order=0", cam_entity),
        );
    }

    world.entity_mut(cam_entity).insert(RenderTarget::default());
    console_info(
        "PlayMode",
        format!("Camera {:?} target set to primary window", cam_entity),
    );

    // Disable the egui UI camera
    let mut ui_cam_q = world.query_filtered::<Entity, With<EditorUiCamera>>();
    let ui_cams: Vec<Entity> = ui_cam_q.iter(world).collect();
    for entity in ui_cams {
        if let Some(mut camera) = world.get_mut::<Camera>(entity) {
            camera.is_active = false;
        }
    }

    // 3D-only render setup: HDR + atmosphere + normal prepass. Atmosphere
    // components must be attached at the moment Bevy first renders this
    // camera — attaching them later would expand the bind group layout
    // and crash wgpu with a binding mismatch. `EffectRouting` later
    // replaces these values with whatever a `WorldEnvironment` entity
    // authored, so the play camera ends up rendering identically to the
    // editor 3D camera. None of this applies to a 2D camera, which uses
    // its own pipeline and has no atmosphere bind group.
    if !is_2d_camera {
        let medium_handle = world
            .resource_mut::<Assets<ScatteringMedium>>()
            .add(ScatteringMedium::default());
        // Match the editor camera's far plane (100km, see
        // `renzora_engine::camera::spawn_editor_camera`). Default
        // Bevy `PerspectiveProjection` has `far: 1000.0`, which clips
        // atmosphere / sky / distant terrain in play mode and is the
        // main reason "play view looks totally different to editor".
        // Preserve any other authored projection fields the user set
        // (fov, aspect, near) — just override the far.
        if let Some(mut proj) = world.get_mut::<Projection>(cam_entity) {
            if let Projection::Perspective(ref mut p) = *proj {
                p.far = 100_000.0;
            }
        } else {
            world
                .entity_mut(cam_entity)
                .insert(Projection::Perspective(PerspectiveProjection {
                    far: 100_000.0,
                    ..default()
                }));
        }
        world.entity_mut(cam_entity).insert((
            Hdr,
            NormalPrepass,
            // Mirrors the editor camera: depth + motion vectors at spawn so
            // SSGI / Lumen `ScreenSpace` can read them (see
            // `renzora_engine::camera` for the Bevy 0.18
            // prepass-specialization rationale).
            DepthPrepass,
            MotionVectorPrepass,
            Atmosphere {
                inner_radius: 6_360_000.0,
                outer_radius: 6_460_000.0,
                ground_albedo: Vec3::splat(0.3),
                medium: medium_handle,
            },
            AtmosphereSettings::default(),
            AtmosphereEnvironmentMapLight {
                intensity: 0.0,
                ..default()
            },
            Msaa::Off,
        ));

        // DeferredPrepass attached only in Deferred rendering mode -- mirrors
        // the editor camera. With Forward mode, it stays absent and shading
        // runs forward; with Deferred, the G-buffer gets generated and SSR
        // / albedo-prepass-readers work.
        let deferred = world
            .get_resource::<renzora::ResolvedRenderingMode>()
            .map(|m| m.is_deferred())
            .unwrap_or(false);
        if deferred {
            world.entity_mut(cam_entity).insert(DeferredPrepass);
        }
    }

    world.entity_mut(cam_entity).insert(PlayModeCamera);
    console_info(
        "PlayMode",
        format!("Inserted PlayModeCamera marker on {:?}", cam_entity),
    );

    // Reset script states and unpause physics (via decoupled events)
    world.trigger(renzora::core::ResetScriptStates);
    world.trigger(renzora::core::UnpausePhysics);

    play_mode.active_game_camera = Some(cam_entity);
    play_mode.state = PlayState::Playing;

    console_success(
        "PlayMode",
        format!(
            "=== PLAY MODE ACTIVE (camera: {:?} \"{}\") ===",
            cam_entity, cam_name
        ),
    );
    info!("Entered play mode (camera: {:?})", cam_entity);
}

fn exit_play_mode(world: &mut World, play_mode: &mut PlayModeState) {
    use renzora::core::console_log::*;
    console_info("PlayMode", "=== EXITING PLAY MODE ===");

    // Remove PlayModeCamera and deactivate the game camera
    let mut play_cam_q = world.query_filtered::<Entity, With<PlayModeCamera>>();
    let play_cams: Vec<Entity> = play_cam_q.iter(world).collect();

    // Snapshot which play cameras are 2D — the 3D-specific component
    // strip below would otherwise no-op for 2D, but keep the data clean
    // so we don't rip Camera2d off an authored 2D scene camera.
    let play_cam_kinds: Vec<(Entity, bool)> = play_cams
        .iter()
        .map(|e| (*e, world.get::<Camera2d>(*e).is_some()))
        .collect();

    for (entity, is_2d) in &play_cam_kinds {
        let name = world
            .get::<Name>(*entity)
            .map(|n| n.to_string())
            .unwrap_or_else(|| "unnamed".into());
        console_info(
            "PlayMode",
            format!(
                "Tearing down play camera {:?} \"{}\" mode={}",
                entity,
                name,
                if *is_2d { "2D" } else { "3D" }
            ),
        );
    }

    for (entity, is_2d) in play_cam_kinds {
        if let Some(mut cam) = world.get_mut::<Camera>(entity) {
            cam.is_active = false;
        }
        let mut e = world.entity_mut(entity);
        e.remove::<PlayModeCamera>();
        e.remove::<Camera>();
        // `enter_play_mode` pointed this camera at the primary window via
        // `RenderTarget::default()`. Strip it so the authored SceneCamera
        // returns to its inactive editor baseline and can never paint the
        // window behind the editor chrome if a later system reactivates it.
        e.remove::<RenderTarget>();
        if !is_2d {
            // Strip 3D-only components we added on entry so the authored
            // `SceneCamera` returns to its baseline. The bind group layout
            // for the camera also goes away when `Camera3d` is removed,
            // so a future play-mode entry rebuilds it cleanly. Camera2d
            // is authored content — leave it intact.
            e.remove::<Camera3d>();
            e.remove::<Hdr>();
            e.remove::<NormalPrepass>();
            e.remove::<DepthPrepass>();
            e.remove::<MotionVectorPrepass>();
            e.remove::<DeferredPrepass>();
            e.remove::<Atmosphere>();
            e.remove::<AtmosphereSettings>();
            e.remove::<AtmosphereEnvironmentMapLight>();
            e.remove::<Msaa>();
        }
    }

    // Re-enable both editor camera flavours and restore their render
    // targets. Either could have been deactivated on entry.
    let viewport_image = world
        .get_resource::<ViewportRenderTarget>()
        .and_then(|vrt| vrt.image.clone());

    let mut editor_q = world.query_filtered::<Entity, With<EditorCamera>>();
    let editor_entities: Vec<Entity> = editor_q.iter(world).collect();
    let mut editor_2d_q = world.query_filtered::<Entity, With<EditorCamera2d>>();
    let editor_2d_entities: Vec<Entity> = editor_2d_q.iter(world).collect();
    for entity in editor_entities.iter().chain(editor_2d_entities.iter()) {
        console_info(
            "PlayMode",
            format!("Re-enabling editor camera {:?}", entity),
        );
    }
    for entity in editor_entities.iter().chain(editor_2d_entities.iter()) {
        if let Some(mut camera) = world.get_mut::<Camera>(*entity) {
            camera.is_active = true;
        }
        if let Some(ref img) = viewport_image {
            world
                .entity_mut(*entity)
                .insert(RenderTarget::Image(Handle::<Image>::clone(img).into()));
        }
    }

    // Re-enable the egui UI camera
    let mut ui_cam_q = world.query_filtered::<Entity, With<EditorUiCamera>>();
    let ui_cams: Vec<Entity> = ui_cam_q.iter(world).collect();
    for entity in ui_cams {
        if let Some(mut camera) = world.get_mut::<Camera>(entity) {
            camera.is_active = true;
        }
    }

    // Re-pause physics (via decoupled event)
    world.trigger(renzora::core::PausePhysics);

    // Restore cursor
    let mut cursor_q = world.query::<&mut CursorOptions>();
    if let Ok(mut cursor) = cursor_q.single_mut(world) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }

    play_mode.active_game_camera = None;
    play_mode.state = PlayState::Editing;

    console_success("PlayMode", "=== PLAY MODE EXITED — back to editing ===");
    info!("Exited play mode");
}
