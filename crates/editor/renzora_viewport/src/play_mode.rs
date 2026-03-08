//! Play mode — switches from editor camera to game camera for in-editor playtesting.

use bevy::prelude::*;
use bevy::camera::RenderTarget;
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
    // Save scene before entering play mode so we can restore on stop
    renzora_runtime::scene_io::save_current_scene(world);

    // Find the game camera: prefer DefaultCamera, then first SceneCamera
    let mut q = world.query_filtered::<(Entity, Option<&DefaultCamera>), With<SceneCamera>>();
    let candidates: Vec<(Entity, bool)> = q.iter(world).map(|(e, dc): (Entity, Option<&DefaultCamera>)| (e, dc.is_some())).collect();

    let game_camera = candidates.iter().find(|(_, is_default)| *is_default).map(|(e, _)| *e)
        .or_else(|| candidates.first().map(|(e, _)| *e));

    let Some(cam_entity) = game_camera else {
        warn!("Play mode: no scene camera found");
        return;
    };

    // Get viewport render target
    let render_target: Option<Handle<Image>> = world.get_resource::<ViewportRenderTarget>()
        .and_then(|vrt| vrt.image.clone());

    // Disable editor camera
    let mut editor_q = world.query_filtered::<Entity, With<EditorCamera>>();
    let editor_entities: Vec<Entity> = editor_q.iter(world).collect();
    for entity in editor_entities {
        if let Some(mut camera) = world.get_mut::<Camera>(entity) {
            camera.is_active = false;
        }
    }

    // Set up game camera for rendering
    // Insert Camera3d if not present
    if world.get::<Camera3d>(cam_entity).is_none() {
        world.entity_mut(cam_entity).insert(Camera3d::default());
    }

    // Insert Camera if not present
    if world.get::<Camera>(cam_entity).is_none() {
        world.entity_mut(cam_entity).insert(Camera::default());
    }

    // Configure camera
    if let Some(mut cam) = world.get_mut::<Camera>(cam_entity) {
        cam.is_active = true;
        cam.order = 0;
    }

    // Point at viewport render target
    if let Some(ref img) = render_target {
        let rt = RenderTarget::Image(Handle::<Image>::clone(img).into());
        world.entity_mut(cam_entity).insert(rt);
    }

    world.entity_mut(cam_entity).insert(PlayModeCamera);

    play_mode.active_game_camera = Some(cam_entity);
    play_mode.state = PlayState::Playing;

    info!("Entered play mode (camera: {:?})", cam_entity);
}

fn exit_play_mode(world: &mut World, play_mode: &mut PlayModeState) {
    // Remove PlayModeCamera and deactivate the game camera
    let mut play_cam_q = world.query_filtered::<Entity, With<PlayModeCamera>>();
    let play_cams: Vec<Entity> = play_cam_q.iter(world).collect();

    for entity in play_cams {
        if let Some(mut cam) = world.get_mut::<Camera>(entity) {
            cam.is_active = false;
        }
        let mut em = world.entity_mut(entity);
        em.remove::<PlayModeCamera>();
        em.remove::<RenderTarget>();
        em.remove::<Camera>();
        em.remove::<Camera3d>();
    }

    // Re-enable editor camera and restore its render target
    let viewport_image = world.get_resource::<ViewportRenderTarget>()
        .and_then(|vrt| vrt.image.clone());

    let mut editor_q = world.query_filtered::<Entity, With<EditorCamera>>();
    let editor_entities: Vec<Entity> = editor_q.iter(world).collect();
    for entity in editor_entities {
        if let Some(mut camera) = world.get_mut::<Camera>(entity) {
            camera.is_active = true;
        }
        if let Some(ref img) = viewport_image {
            world.entity_mut(entity)
                .insert(RenderTarget::Image(Handle::<Image>::clone(img).into()));
        }
    }

    play_mode.active_game_camera = None;
    play_mode.state = PlayState::Editing;

    info!("Exited play mode");
}
