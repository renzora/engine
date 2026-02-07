//! Scene management system for saving, loading, and managing scene tabs.
//!
//! Uses Bevy's DynamicScene format (.ron) with reflection-based serialization.
//! Editor metadata is stored as a resource within the scene file.

mod primitives;
mod setup;
pub mod loader;
pub mod manager;
pub mod saver;
pub mod saveable;

// Editor setup exports
pub use primitives::{spawn_primitive, PrimitiveType};
#[allow(unused_imports)]
pub use setup::{setup_editor_camera, EditorOnly, UiCamera};

// Scene management exports
pub use loader::{
    load_scene_bevy, on_bevy_scene_ready, rehydrate_mesh_components,
    rehydrate_point_lights, rehydrate_directional_lights, rehydrate_spot_lights, rehydrate_sun_lights,
    rehydrate_terrain_chunks, apply_terrain_materials, rebuild_children_from_child_of,
    add_raytracing_to_meshes, prepare_meshes_for_solari,
    rehydrate_cameras_3d, rehydrate_camera_rigs, rehydrate_cameras_2d,
};
pub use manager::{assign_scene_tab_ids, handle_scene_requests, handle_save_shortcut, handle_make_default_camera, auto_save_scene};
pub use saver::EditorSceneMetadata;
pub use saveable::{SceneSaveableRegistry, create_default_registry};

use bevy::prelude::*;

#[allow(dead_code)]
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, _app: &mut App) {
        // Scene loading is now done via OnEnter(AppState::Editor) in main.rs
    }
}
