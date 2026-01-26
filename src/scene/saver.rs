//! Scene saving using Bevy's DynamicScene system

use bevy::prelude::*;
use bevy::scene::DynamicSceneBuilder;
use std::path::Path;

use crate::core::{EditorEntity, SceneNode, SceneTabId, SceneManagerState, HierarchyState, OrbitCameraState, WorldEnvironmentMarker};
use crate::shared::{
    MeshNodeData, CameraNodeData, CameraRigData, MeshInstanceData, SceneInstanceData,
    PhysicsBodyData, CollisionShapeData, Sprite2DData, Camera2DData,
    UIPanelData, UILabelData, UIButtonData, UIImageData,
};
use crate::scripting::ScriptComponent;

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
        let mut query = world.query_filtered::<(Entity, &SceneTabId), With<SceneNode>>();
        query
            .iter(world)
            .filter(|(_, tab_id)| tab_id.0 == current_tab)
            .map(|(e, _)| e)
            .collect()
    };

    // Collect editor metadata and insert as resource temporarily
    let meta = collect_editor_meta(world, current_tab);
    world.insert_resource(meta);

    // Build the dynamic scene with allowed components and resources
    let scene = DynamicSceneBuilder::from_world(world)
        .deny_all()
        // Editor metadata resource
        .allow_resource::<EditorSceneMetadata>()
        // Core Bevy components
        .allow_component::<Transform>()
        .allow_component::<Name>()
        // Note: We only save ChildOf, not Children. Children references may point to
        // entities that aren't saved (e.g., spawned GLB meshes). Bevy reconstructs
        // Children automatically from ChildOf relationships when the scene loads.
        .allow_component::<ChildOf>()
        // Editor components
        .allow_component::<EditorEntity>()
        .allow_component::<SceneNode>()
        // Shared game components
        .allow_component::<MeshNodeData>()
        .allow_component::<CameraNodeData>()
        .allow_component::<CameraRigData>()
        .allow_component::<MeshInstanceData>()
        .allow_component::<SceneInstanceData>()
        .allow_component::<PhysicsBodyData>()
        .allow_component::<CollisionShapeData>()
        .allow_component::<Sprite2DData>()
        .allow_component::<Camera2DData>()
        .allow_component::<UIPanelData>()
        .allow_component::<UILabelData>()
        .allow_component::<UIButtonData>()
        .allow_component::<UIImageData>()
        // Scripting
        .allow_component::<ScriptComponent>()
        // Environment
        .allow_component::<WorldEnvironmentMarker>()
        // Lights (Bevy built-in)
        .allow_component::<PointLight>()
        .allow_component::<DirectionalLight>()
        .allow_component::<SpotLight>()
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
