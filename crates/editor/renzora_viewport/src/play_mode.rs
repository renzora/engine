//! Play mode — switches from editor camera to game camera for in-editor playtesting.

use bevy::prelude::*;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::window::{CursorGrabMode, CursorOptions};
use renzora_editor::camera::EditorUiCamera;
use renzora_runtime::{
    DefaultCamera, EditorCamera, PlayModeCamera, PlayModeState, PlayState,
    SceneCamera, ViewportRenderTarget,
};

/// Handles play mode transitions each frame.
pub fn handle_play_mode_transitions(world: &mut World) {
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

fn enter_play_mode(world: &mut World, play_mode: &mut PlayModeState) {
    use renzora_core::console_log::*;
    console_info("PlayMode", "=== ENTERING PLAY MODE ===");

    // Save scene before entering play mode so we can restore on stop
    renzora_runtime::scene_io::save_current_scene(world);
    console_info("PlayMode", "Scene saved before play mode");

    // Find the game camera: prefer DefaultCamera, then first SceneCamera
    let mut q = world.query_filtered::<(Entity, Option<&DefaultCamera>), With<SceneCamera>>();
    let candidates: Vec<(Entity, bool)> = q.iter(world).map(|(e, dc): (Entity, Option<&DefaultCamera>)| (e, dc.is_some())).collect();

    console_info("PlayMode", format!("Scene camera candidates: {}", candidates.len()));
    for (e, is_default) in &candidates {
        let name = world.get::<Name>(*e).map(|n| n.to_string()).unwrap_or_else(|| "unnamed".into());
        console_info("PlayMode", format!("  {:?} \"{}\" default={}", e, name, is_default));
    }

    let game_camera = candidates.iter().find(|(_, is_default)| *is_default).map(|(e, _)| *e)
        .or_else(|| candidates.first().map(|(e, _)| *e));

    let Some(cam_entity) = game_camera else {
        console_error("PlayMode", "No scene camera found — cannot enter play mode");
        warn!("Play mode: no scene camera found");
        return;
    };

    let cam_name = world.get::<Name>(cam_entity).map(|n| n.to_string()).unwrap_or_else(|| "unnamed".into());
    console_info("PlayMode", format!("Selected game camera: {:?} \"{}\"", cam_entity, cam_name));

    // Disable editor camera
    let mut editor_q = world.query_filtered::<Entity, With<EditorCamera>>();
    let editor_entities: Vec<Entity> = editor_q.iter(world).collect();
    for entity in &editor_entities {
        console_info("PlayMode", format!("Disabling editor camera {:?}", entity));
    }
    for entity in editor_entities {
        if let Some(mut camera) = world.get_mut::<Camera>(entity) {
            camera.is_active = false;
        }
    }

    // Set up game camera for rendering
    let had_cam3d = world.get::<Camera3d>(cam_entity).is_some();
    let had_camera = world.get::<Camera>(cam_entity).is_some();
    console_info("PlayMode", format!(
        "Game camera {:?} state before setup: Camera3d={} Camera={}",
        cam_entity, had_cam3d, had_camera
    ));

    // Insert Camera3d if not present
    if !had_cam3d {
        world.entity_mut(cam_entity).insert(Camera3d::default());
        console_info("PlayMode", format!("Inserted Camera3d on {:?}", cam_entity));
    }

    // Insert Camera if not present
    if !had_camera {
        world.entity_mut(cam_entity).insert(Camera::default());
        console_info("PlayMode", format!("Inserted Camera on {:?}", cam_entity));
    }

    // Configure camera
    if let Some(mut cam) = world.get_mut::<Camera>(cam_entity) {
        cam.is_active = true;
        cam.order = 0;
        console_info("PlayMode", format!("Configured camera {:?}: active=true order=0", cam_entity));
    }

    // Render directly to the primary window (replace any offscreen render target)
    world.entity_mut(cam_entity).insert(RenderTarget::default());
    console_info("PlayMode", format!("Camera {:?} target set to primary window", cam_entity));

    // Disable the egui UI camera so it doesn't paint over the game output
    let mut ui_cam_q = world.query_filtered::<Entity, With<EditorUiCamera>>();
    let ui_cams: Vec<Entity> = ui_cam_q.iter(world).collect();
    for entity in ui_cams {
        if let Some(mut camera) = world.get_mut::<Camera>(entity) {
            camera.is_active = false;
        }
    }

    world.entity_mut(cam_entity).insert(PlayModeCamera);
    console_info("PlayMode", format!("Inserted PlayModeCamera marker on {:?}", cam_entity));

    // Unpause physics simulation
    renzora_physics::unpause(world);

    play_mode.active_game_camera = Some(cam_entity);
    play_mode.state = PlayState::Playing;

    console_success("PlayMode", format!("=== PLAY MODE ACTIVE (camera: {:?} \"{}\") ===", cam_entity, cam_name));
    info!("Entered play mode (camera: {:?})", cam_entity);
}

fn exit_play_mode(world: &mut World, play_mode: &mut PlayModeState) {
    use renzora_core::console_log::*;
    console_info("PlayMode", "=== EXITING PLAY MODE ===");

    // Remove PlayModeCamera and deactivate the game camera
    let mut play_cam_q = world.query_filtered::<Entity, With<PlayModeCamera>>();
    let play_cams: Vec<Entity> = play_cam_q.iter(world).collect();

    for entity in &play_cams {
        let name = world.get::<Name>(*entity).map(|n| n.to_string()).unwrap_or_else(|| "unnamed".into());
        console_info("PlayMode", format!(
            "Tearing down play camera {:?} \"{}\": removing PlayModeCamera, Camera, Camera3d, RenderTarget",
            entity, name
        ));
    }

    for entity in play_cams {
        if let Some(mut cam) = world.get_mut::<Camera>(entity) {
            cam.is_active = false;
        }
        world.entity_mut(entity).remove::<PlayModeCamera>();
        world.entity_mut(entity).remove::<Camera>();
        world.entity_mut(entity).remove::<Camera3d>();
    }

    // Re-enable editor camera and restore its render target to viewport image
    let viewport_image = world.get_resource::<ViewportRenderTarget>()
        .and_then(|vrt| vrt.image.clone());

    let mut editor_q = world.query_filtered::<Entity, With<EditorCamera>>();
    let editor_entities: Vec<Entity> = editor_q.iter(world).collect();
    for entity in &editor_entities {
        console_info("PlayMode", format!("Re-enabling editor camera {:?}", entity));
    }
    for entity in editor_entities {
        if let Some(mut camera) = world.get_mut::<Camera>(entity) {
            camera.is_active = true;
        }
        if let Some(ref img) = viewport_image {
            world.entity_mut(entity)
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

    // Re-pause physics simulation
    renzora_physics::pause(world);

    // Restore cursor (in case a script locked it during play mode)
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
