//! Scene saving using Bevy's DynamicScene system

use bevy::prelude::*;
use bevy::scene::DynamicSceneBuilder;
use std::path::Path;

use crate::core::{EditorEntity, SceneTabId, SceneManagerState, HierarchyState, OrbitCameraState};
use crate::project::CurrentProject;
use crate::component_system::{MaterialData, MeshInstanceData, SceneInstanceData, Sprite2DData};

use super::saveable::SceneSaveableRegistry;

/// Editor-only metadata stored as a resource in the scene file.
/// This is stripped during export so it doesn't appear in the runtime.
#[derive(Resource, Reflect, Clone, Debug, Default)]
#[reflect(Resource)]
pub struct EditorSceneMetadata {
    /// Editor camera orbit focus point
    pub camera_focus: Vec3,
    /// Editor camera distance from focus
    pub camera_distance: f32,
    /// Editor camera yaw (horizontal rotation)
    pub camera_yaw: f32,
    /// Editor camera pitch (vertical rotation)
    pub camera_pitch: f32,
    /// Names of entities that are expanded in the hierarchy
    pub expanded_entities: Vec<String>,
}

/// Save the current scene using Bevy's DynamicScene system
/// Creates a single .ron file containing both game data and editor metadata
pub fn save_scene_bevy(
    path: &Path,
    world: &mut World,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get current tab
    let current_tab = {
        let scene_state = world.resource::<SceneManagerState>();
        scene_state.active_scene_tab
    };

    // Collect entities for this scene tab
    let entities: Vec<Entity> = {
        let mut query = world.query_filtered::<(Entity, &SceneTabId), With<crate::core::SceneNode>>();
        query
            .iter(world)
            .filter(|(_, tab_id)| tab_id.0 == current_tab)
            .map(|(e, _)| e)
            .collect()
    };

    // Normalize all asset paths to project-relative before serialization
    normalize_asset_paths(world, &entities);

    // Collect editor metadata and insert as resource temporarily
    let meta = collect_editor_meta(world, current_tab);
    world.insert_resource(meta);

    // Get the saveable registry to know which components to save
    let saveable_registry = world.resource::<SceneSaveableRegistry>();

    // Build the dynamic scene with allowed components and resources
    let builder = DynamicSceneBuilder::from_world(world)
        .deny_all()
        // Editor metadata resource
        .allow_resource::<EditorSceneMetadata>();

    // Apply all registered saveable components (takes and returns ownership)
    let builder = saveable_registry.allow_all(builder);

    let scene = builder
        .extract_entities(entities.into_iter())
        .extract_resources()
        .build();

    // Remove the temporary resource
    world.remove_resource::<EditorSceneMetadata>();

    // Serialize the scene to RON format
    let type_registry = world.resource::<AppTypeRegistry>();
    let registry = type_registry.read();
    let serialized = scene.serialize(&registry)?;

    // Write scene file (.ron)
    let scene_path = path.with_extension("ron");
    std::fs::write(&scene_path, &serialized)?;

    info!("Scene saved to: {}", scene_path.display());
    Ok(())
}

/// Collect editor metadata (camera state, expanded entities)
fn collect_editor_meta(world: &World, current_tab: usize) -> EditorSceneMetadata {
    // Get editor camera state
    let orbit = world.resource::<OrbitCameraState>();

    // Get expanded entities (by name, since entity IDs change on reload)
    let hierarchy_state = world.resource::<HierarchyState>();
    let mut expanded_names = Vec::new();

    for entity in hierarchy_state.expanded_entities.iter() {
        // Check if entity belongs to current tab and get its name
        if let Some(editor_entity) = world.get::<EditorEntity>(*entity) {
            if let Some(tab_id) = world.get::<SceneTabId>(*entity) {
                if tab_id.0 == current_tab {
                    expanded_names.push(editor_entity.name.clone());
                }
            }
        }
    }

    EditorSceneMetadata {
        camera_focus: orbit.focus,
        camera_distance: orbit.distance,
        camera_yaw: orbit.yaw,
        camera_pitch: orbit.pitch,
        expanded_entities: expanded_names,
    }
}

/// Normalize all asset paths on scene entities to be project-relative.
/// This is a safety net that catches any absolute paths before they get serialized.
fn normalize_asset_paths(world: &mut World, entities: &[Entity]) {
    let project = world.get_resource::<CurrentProject>().cloned();
    let Some(project) = project else {
        return;
    };

    for &entity in entities {
        // MaterialData.material_path
        if let Some(mut data) = world.get_mut::<MaterialData>(entity) {
            if let Some(ref path_str) = data.material_path {
                let p = std::path::Path::new(path_str.as_str());
                if p.is_absolute() {
                    if let Some(rel) = project.make_relative(p) {
                        info!("Scene save: normalized material path '{}' -> '{}'", path_str, rel);
                        data.material_path = Some(rel);
                    } else {
                        warn!("Scene save: material path '{}' is absolute but not inside project", path_str);
                    }
                }
            }
        }

        // MeshInstanceData.model_path
        if let Some(mut data) = world.get_mut::<MeshInstanceData>(entity) {
            if let Some(ref path_str) = data.model_path {
                let p = std::path::Path::new(path_str.as_str());
                if p.is_absolute() {
                    if let Some(rel) = project.make_relative(p) {
                        info!("Scene save: normalized model path '{}' -> '{}'", path_str, rel);
                        data.model_path = Some(rel);
                    } else {
                        warn!("Scene save: model path '{}' is absolute but not inside project", path_str);
                    }
                }
            }
        }

        // SceneInstanceData.scene_path
        if let Some(mut data) = world.get_mut::<SceneInstanceData>(entity) {
            let p = std::path::Path::new(data.scene_path.as_str());
            if p.is_absolute() {
                if let Some(rel) = project.make_relative(p) {
                    info!("Scene save: normalized scene path '{}' -> '{}'", data.scene_path, rel);
                    data.scene_path = rel;
                } else {
                    warn!("Scene save: scene path '{}' is absolute but not inside project", data.scene_path);
                }
            }
        }

        // Sprite2DData.texture_path
        if let Some(mut data) = world.get_mut::<Sprite2DData>(entity) {
            let p = std::path::Path::new(data.texture_path.as_str());
            if p.is_absolute() {
                if let Some(rel) = project.make_relative(p) {
                    info!("Scene save: normalized texture path '{}' -> '{}'", data.texture_path, rel);
                    data.texture_path = rel;
                } else {
                    warn!("Scene save: texture path '{}' is absolute but not inside project", data.texture_path);
                }
            }
        }
    }
}
