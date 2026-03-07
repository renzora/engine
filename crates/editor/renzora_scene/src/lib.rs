//! Renzora Scene — editor-side scene plugin that wires save/load to keybindings and splash state.
//!
//! The actual save/load and rehydration logic lives in `renzora_runtime::scene_io`.

use bevy::prelude::*;

use renzora_core::SaveSceneRequested;
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
// Load on entering editor
// ============================================================================

fn load_scene_on_enter(world: &mut World) {
    info!("load_scene_on_enter triggered");
    scene_io::load_current_scene(world);
}

// ============================================================================
// Plugin
// ============================================================================

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(SplashState::Editor), load_scene_on_enter)
            .add_systems(
                Update,
                (scene_io::rehydrate_meshes, scene_io::rehydrate_cameras, scene_io::rehydrate_suns, scene_io::sync_scene_camera_to_editor_camera, detect_save_keybinding, save_scene_system)
                    .run_if(in_state(SplashState::Editor)),
            );
    }
}
