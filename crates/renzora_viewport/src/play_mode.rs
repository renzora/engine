//! Play mode — switches from editor camera to game camera for in-editor playtesting.

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::core_pipeline::prepass::NormalPrepass;
use bevy::light::AtmosphereEnvironmentMapLight;
use bevy::pbr::{Atmosphere, AtmosphereSettings, ScatteringMedium};
use bevy::render::view::Hdr;
use bevy::window::{CursorGrabMode, CursorOptions};
use renzora_editor::camera::EditorUiCamera;
use renzora::core::{
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
    use renzora::core::console_log::*;
    console_info("PlayMode", "=== ENTERING PLAY MODE ===");

    // Save scene before entering play mode (observed by renzora_engine)
    world.trigger(renzora::core::SaveCurrentScene);
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

    if !had_cam3d {
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
        console_info("PlayMode", format!("Configured camera {:?}: active=true order=0", cam_entity));
    }

    world.entity_mut(cam_entity).insert(RenderTarget::default());
    console_info("PlayMode", format!("Camera {:?} target set to primary window", cam_entity));

    // Disable the egui UI camera
    let mut ui_cam_q = world.query_filtered::<Entity, With<EditorUiCamera>>();
    let ui_cams: Vec<Entity> = ui_cam_q.iter(world).collect();
    for entity in ui_cams {
        if let Some(mut camera) = world.get_mut::<Camera>(entity) {
            camera.is_active = false;
        }
    }

    // Match the editor camera's render setup. Atmosphere components must
    // be attached at the moment Bevy first renders this camera —
    // attaching them later would expand the bind group layout and crash
    // wgpu with a binding mismatch. `EffectRouting` later replaces these
    // values with whatever a `WorldEnvironment` entity authored, so the
    // play camera ends up rendering identically to the editor camera.
    let medium_handle = world
        .resource_mut::<Assets<ScatteringMedium>>()
        .add(ScatteringMedium::default());
    world.entity_mut(cam_entity).insert((
        Hdr,
        NormalPrepass,
        Atmosphere {
            bottom_radius: 6_360_000.0,
            top_radius: 6_460_000.0,
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

    world.entity_mut(cam_entity).insert(PlayModeCamera);
    console_info("PlayMode", format!("Inserted PlayModeCamera marker on {:?}", cam_entity));

    // Reset script states and unpause physics (via decoupled events)
    world.trigger(renzora::core::ResetScriptStates);
    world.trigger(renzora::core::UnpausePhysics);

    play_mode.active_game_camera = Some(cam_entity);
    play_mode.state = PlayState::Playing;

    console_success("PlayMode", format!("=== PLAY MODE ACTIVE (camera: {:?} \"{}\") ===", cam_entity, cam_name));
    info!("Entered play mode (camera: {:?})", cam_entity);
}

fn exit_play_mode(world: &mut World, play_mode: &mut PlayModeState) {
    use renzora::core::console_log::*;
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
        let mut e = world.entity_mut(entity);
        e.remove::<PlayModeCamera>();
        e.remove::<Camera>();
        e.remove::<Camera3d>();
        // Strip everything we added on entry so the authored `SceneCamera`
        // returns to its baseline state. Bevy's bind group layout for the
        // camera also goes away when `Camera3d` is removed, so re-adding
        // these on a future play-mode entry rebuilds it cleanly.
        e.remove::<Hdr>();
        e.remove::<NormalPrepass>();
        e.remove::<Atmosphere>();
        e.remove::<AtmosphereSettings>();
        e.remove::<AtmosphereEnvironmentMapLight>();
        e.remove::<Msaa>();
    }

    // Re-enable editor camera and restore its render target
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
