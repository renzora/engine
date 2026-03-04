//! Scene loading systems for scripting
//!
//! Processes scene load and prefab spawn requests from scripts.

use bevy::prelude::*;
use bevy::scene::DynamicSceneRoot;

use crate::core::{EditorEntity, PlayModeState, SceneNode};
use crate::project::CurrentProject;
use crate::scripting::resources::{
    PendingPrefabSpawn, PrefabSpawnRequest, RuntimePrefabInstance, SceneCommandQueue,
};

/// System to process prefab spawn and unload requests
pub fn process_prefab_spawns(
    mut commands: Commands,
    mut scene_queue: ResMut<SceneCommandQueue>,
    asset_server: Res<AssetServer>,
    current_project: Option<Res<CurrentProject>>,
    play_mode: Res<PlayModeState>,
    time: Res<Time>,
    prefab_query: Query<Entity, With<RuntimePrefabInstance>>,
) {
    // Only process during play mode
    if !play_mode.is_scripts_running() {
        return;
    }

    // Process unload requests first (before spawning new ones)
    let unload_requests = scene_queue.take_unload_requests();
    for request in unload_requests {
        match request.entity {
            Some(entity) => {
                // Despawn specific entity
                bevy::log::info!("[Script] Unloading entity {:?}", entity);
                commands.entity(entity).try_despawn();
            }
            None => {
                // Despawn all runtime prefabs
                bevy::log::info!("[Script] Unloading all runtime prefabs");
                for entity in prefab_query.iter() {
                    commands.entity(entity).try_despawn();
                }
            }
        }
    }

    let Some(project) = current_project else {
        return;
    };

    // Process prefab spawn requests
    let spawn_requests = scene_queue.take_spawn_requests();
    for request in spawn_requests {
        spawn_prefab_internal(
            &mut commands,
            &asset_server,
            &project.path,
            &request,
            time.elapsed_secs(),
        );
    }

    // Note: Scene loading (replacing entire scene) is more complex and typically
    // wouldn't be done during play mode. For now we just log these requests.
    let load_requests = scene_queue.take_load_requests();
    for request in load_requests {
        bevy::log::warn!(
            "[Script] LoadScene request ignored during play mode: {:?}",
            request.path
        );
    }
}

/// Internal function to spawn a prefab
fn spawn_prefab_internal(
    commands: &mut Commands,
    asset_server: &AssetServer,
    project_path: &std::path::Path,
    request: &PrefabSpawnRequest,
    spawn_time: f32,
) {
    // Resolve the prefab path relative to the project
    let prefab_path = if request.path.is_absolute() {
        request.path.clone()
    } else {
        project_path.join(&request.path)
    };

    // Add .ron extension if not present
    let scene_path = if prefab_path.extension().map_or(true, |ext| ext != "ron") {
        prefab_path.with_extension("ron")
    } else {
        prefab_path.clone()
    };

    bevy::log::info!("[Script] Spawning prefab: {:?} at {:?}", scene_path, request.position);

    // Create rotation quaternion from euler angles
    let rotation = Quat::from_euler(
        EulerRot::XYZ,
        request.rotation.x.to_radians(),
        request.rotation.y.to_radians(),
        request.rotation.z.to_radians(),
    );

    // Load the scene file
    let scene_handle: Handle<DynamicScene> = asset_server.load(scene_path.clone());

    // Create a container entity for the prefab instance
    let prefab_entity = commands
        .spawn((
            Transform::from_translation(request.position).with_rotation(rotation),
            Visibility::default(),
            EditorEntity {
                name: format!(
                    "Prefab_{}",
                    request
                        .path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Unknown".to_string())
                ),
                tag: "runtime_prefab".to_string(),
                visible: true,
                locked: false,
            },
            SceneNode,
            RuntimePrefabInstance {
                source_path: request.path.clone(),
                spawn_time,
            },
            DynamicSceneRoot(scene_handle),
            PendingPrefabSpawn {
                request: request.clone(),
            },
        ))
        .id();

    // If a parent was specified, set up the hierarchy
    if let Some(parent) = request.parent {
        commands.entity(prefab_entity).insert(ChildOf(parent));
    }
}

/// System to clean up prefab markers after loading
#[allow(dead_code)]
pub fn cleanup_loaded_prefabs(
    _commands: Commands,
    _query: Query<(Entity, &PendingPrefabSpawn), Without<bevy::scene::SceneInstance>>,
) {
    // This is a placeholder - actual cleanup happens via on_prefab_ready observer
    // Keeping for potential future use with polling-based approach
}

/// System to finalize spawned prefabs (runs after SceneInstanceReady)
pub fn on_prefab_ready(
    trigger: On<bevy::scene::SceneInstanceReady>,
    mut commands: Commands,
    pending_query: Query<&PendingPrefabSpawn>,
) {
    let entity = trigger.entity;

    if pending_query.get(entity).is_ok() {
        bevy::log::info!("[Script] Prefab instance ready: {:?}", entity);

        // Remove the pending marker
        commands.entity(entity).remove::<PendingPrefabSpawn>();
    }
}

/// System to clear scene queue when play mode stops
pub fn clear_scene_queue_on_stop(
    play_mode: Res<PlayModeState>,
    mut scene_queue: ResMut<SceneCommandQueue>,
    mut last_playing: Local<bool>,
) {
    let currently_playing = play_mode.is_in_play_mode();

    // Detect transition from playing to editing
    if *last_playing && !currently_playing {
        scene_queue.load_requests.clear();
        scene_queue.spawn_requests.clear();
        scene_queue.unload_requests.clear();
    }

    *last_playing = currently_playing;
}

/// System to despawn runtime prefabs when play mode stops
pub fn despawn_runtime_prefabs_on_stop(
    mut commands: Commands,
    play_mode: Res<PlayModeState>,
    prefab_query: Query<Entity, With<RuntimePrefabInstance>>,
    mut last_playing: Local<bool>,
) {
    let currently_playing = play_mode.is_in_play_mode();

    // Detect transition from playing to editing
    if *last_playing && !currently_playing {
        for entity in prefab_query.iter() {
            // Use try_despawn to handle cases where entity might already be gone
            commands.entity(entity).try_despawn();
        }
    }

    *last_playing = currently_playing;
}
