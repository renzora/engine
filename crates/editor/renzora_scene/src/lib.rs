//! Renzora Scene — editor-side scene plugin that wires save/load to keybindings and splash state.
//!
//! The actual save/load and rehydration logic lives in `renzora_runtime::scene_io`.

use bevy::prelude::*;

use renzora_core::{CurrentProject, SaveSceneRequested, SaveAsSceneRequested, NewSceneRequested, OpenSceneRequested, HideInHierarchy, EditorCamera};
use renzora_keybindings::{EditorAction, KeyBindings};
use renzora_runtime::scene_io;
use renzora_splash::SplashState;

// Re-export so downstream code that was using `renzora_scene::{save_scene, load_scene, ...}` still works.
pub use scene_io::{save_scene, load_scene, save_current_scene, load_current_scene};

// ============================================================================
// Keybinding-driven save
// ============================================================================

fn detect_save_keybinding(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
) {
    if keybindings.just_pressed(EditorAction::SaveScene, &keyboard) {
        commands.insert_resource(SaveSceneRequested);
    }
}

fn save_scene_system(world: &mut World) {
    if world.remove_resource::<SaveSceneRequested>().is_some() {
        scene_io::save_current_scene(world);
    }
}

// ============================================================================
// Save As
// ============================================================================

fn save_as_scene_system(world: &mut World) {
    if world.remove_resource::<SaveAsSceneRequested>().is_none() {
        return;
    }

    let Some(project) = world.get_resource::<CurrentProject>() else {
        warn!("No project open — cannot Save As");
        return;
    };
    let scenes_dir = project.resolve_path("scenes");

    #[cfg(not(target_arch = "wasm32"))]
    {
        let file = rfd::FileDialog::new()
            .set_title("Save Scene As")
            .set_directory(&scenes_dir)
            .add_filter("Scene File", &["ron"])
            .set_file_name("new_scene.ron")
            .save_file();

        let Some(file_path) = file else { return };

        // Save the scene to the chosen path
        if let Err(e) = scene_io::save_scene(world, &file_path) {
            error!("Failed to save scene: {}", e);
            return;
        }

        // Update main_scene to point to the new file
        let mut project = world.resource_mut::<CurrentProject>();
        if let Some(relative) = project.make_relative(&file_path) {
            project.config.main_scene = relative;
            if let Err(e) = project.save_config() {
                warn!("Failed to save project.toml: {}", e);
            }
        }

        renzora_core::console_log::console_success(
            "Scene",
            format!("Saved scene as {}", file_path.display()),
        );
    }
}

// ============================================================================
// New Scene
// ============================================================================

fn new_scene_system(world: &mut World) {
    if world.remove_resource::<NewSceneRequested>().is_none() {
        return;
    }

    // Despawn all scene entities (keep editor infrastructure)
    let mut to_despawn = Vec::new();
    {
        let mut query = world.query_filtered::<Entity, (
            With<Name>,
            Without<EditorCamera>,
            Without<HideInHierarchy>,
        )>();
        for entity in query.iter(world) {
            to_despawn.push(entity);
        }
    }

    for entity in to_despawn {
        if world.get_entity(entity).is_ok() {
            world.despawn(entity);
        }
    }

    renzora_core::console_log::console_info("Scene", "New scene created (cleared all entities)");
}

// ============================================================================
// Open Scene
// ============================================================================

fn open_scene_system(world: &mut World) {
    if world.remove_resource::<OpenSceneRequested>().is_none() {
        return;
    }

    let Some(project) = world.get_resource::<CurrentProject>() else {
        warn!("No project open — cannot Open Scene");
        return;
    };
    let scenes_dir = project.resolve_path("scenes");

    #[cfg(not(target_arch = "wasm32"))]
    {
        let file = rfd::FileDialog::new()
            .set_title("Open Scene")
            .set_directory(&scenes_dir)
            .add_filter("Scene File", &["ron"])
            .pick_file();

        let Some(file_path) = file else { return };

        // Despawn current scene entities
        let mut to_despawn = Vec::new();
        {
            let mut query = world.query_filtered::<Entity, (
                With<Name>,
                Without<EditorCamera>,
                Without<HideInHierarchy>,
            )>();
            for entity in query.iter(world) {
                to_despawn.push(entity);
            }
        }
        for entity in to_despawn {
            if world.get_entity(entity).is_ok() {
                world.despawn(entity);
            }
        }

        // Load the new scene
        scene_io::load_scene(world, &file_path);

        // Update main_scene to point to the opened file
        let mut project = world.resource_mut::<CurrentProject>();
        if let Some(relative) = project.make_relative(&file_path) {
            project.config.main_scene = relative;
            if let Err(e) = project.save_config() {
                warn!("Failed to save project.toml: {}", e);
            }
        }

        renzora_core::console_log::console_success(
            "Scene",
            format!("Opened scene {}", file_path.display()),
        );
    }
}

// ============================================================================
// Load on entering editor
// ============================================================================

fn load_scene_on_enter(world: &mut World) {
    info!("load_scene_on_enter triggered");

    // Ensure the asset reader knows the project path before loading the scene.
    // OnEnter fires before Update, so sync_project_asset_path may not have run
    // yet — without this, rehydration asset loads (e.g. GLB models) fail with
    // "Path not found" because the reader's project path is still None.
    if let Some(project) = world.get_resource::<CurrentProject>() {
        let path = project.path.clone();
        if let Some(asset_path) = world.get_resource::<renzora_runtime::ProjectAssetPath>() {
            info!("[scene] Syncing project asset path: {}", path.display());
            asset_path.set(path);
        }
    }

    scene_io::load_current_scene(world);
}

// ============================================================================
// Plugin
// ============================================================================

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ScenePlugin");
        app.add_systems(OnEnter(SplashState::Editor), load_scene_on_enter)
            .add_systems(
                Update,
                (
                    scene_io::rehydrate_meshes,
                    scene_io::rehydrate_cameras,
                    scene_io::rehydrate_suns,
                    scene_io::rehydrate_visibility,
                    scene_io::rehydrate_mesh_instances,
                    scene_io::finish_mesh_instance_rehydrate,
                    detect_save_keybinding,
                    save_scene_system,
                    save_as_scene_system,
                    new_scene_system,
                    open_scene_system,
                )
                    .run_if(in_state(SplashState::Editor)),
            );
    }
}
