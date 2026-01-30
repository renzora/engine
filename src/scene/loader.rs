//! Scene loading using Bevy's DynamicScene system

#![allow(dead_code)]

use bevy::prelude::*;
use bevy::scene::{DynamicSceneRoot, SceneInstanceReady};
use std::path::Path;

use crate::core::{EditorEntity, SceneTabId, HierarchyState, OrbitCameraState};
use crate::shared::{
    MeshNodeData, MeshPrimitiveType,
    PointLightData, DirectionalLightData, SpotLightData,
};
use crate::{console_info, console_warn};

use super::saver::EditorSceneMetadata;

/// Marker component for scenes that are still loading
#[derive(Component)]
pub struct PendingSceneLoad {
    /// The scene tab this scene belongs to
    pub tab_id: usize,
}

/// Result of loading a scene using Bevy's scene system
pub struct BevySceneLoadResult {
    /// Handle to the loading scene
    pub scene_handle: Handle<DynamicScene>,
    /// The entity that holds the scene root
    pub root_entity: Entity,
}

/// Load a scene using Bevy's DynamicScene system
/// Returns a handle to the scene being loaded asynchronously
pub fn load_scene_bevy(
    commands: &mut Commands,
    asset_server: &AssetServer,
    path: &Path,
    tab_id: usize,
) -> BevySceneLoadResult {
    // Load the .ron scene file
    let scene_path = path.with_extension("ron");
    let scene_handle: Handle<DynamicScene> = asset_server.load(scene_path);

    // Spawn the scene root entity
    let root_entity = commands.spawn((
        DynamicSceneRoot(scene_handle.clone()),
        PendingSceneLoad { tab_id },
    )).id();

    BevySceneLoadResult {
        scene_handle,
        root_entity,
    }
}

/// System to finalize loaded Bevy scenes
/// This observer runs when a scene instance is ready
pub fn on_bevy_scene_ready(
    trigger: On<SceneInstanceReady>,
    mut commands: Commands,
    pending_query: Query<&PendingSceneLoad>,
    children_query: Query<&Children>,
    child_of_query: Query<(Entity, &ChildOf), With<EditorEntity>>,
    editor_entities: Query<&EditorEntity>,
    mut hierarchy_state: ResMut<HierarchyState>,
    mut orbit_camera: ResMut<OrbitCameraState>,
    editor_meta: Option<Res<EditorSceneMetadata>>,
) {
    let root_entity = trigger.entity;

    console_info!("Scene", "SceneInstanceReady triggered for entity {:?}", root_entity);

    if let Ok(pending) = pending_query.get(root_entity) {
        let tab_id = pending.tab_id;
        console_info!("Scene", "Processing scene load for tab {}", tab_id);

        // Apply editor metadata from the scene resource (if present)
        if let Some(ref meta) = editor_meta {
            // Restore camera state
            orbit_camera.focus = meta.camera_focus;
            orbit_camera.distance = meta.camera_distance;
            orbit_camera.yaw = meta.camera_yaw;
            orbit_camera.pitch = meta.camera_pitch;
        }

        // Get the direct children of the DynamicSceneRoot
        // These are the root entities of our scene (entities that had no parent in the scene file)
        if let Ok(children) = children_query.get(root_entity) {
            let direct_children: Vec<Entity> = children.iter().collect();
            console_info!("Scene", "DynamicSceneRoot has {} direct children", direct_children.len());

            // Collect ALL entities that need SceneTabId by traversing ChildOf relationships
            // This is more reliable than Children component which may not be populated yet
            let mut all_scene_entities: Vec<Entity> = direct_children.clone();

            // Find all entities that have ChildOf pointing to any of our scene entities
            // Keep iterating until we find no more children
            let mut entities_to_check = direct_children.clone();
            while !entities_to_check.is_empty() {
                let mut found_children: Vec<Entity> = Vec::new();
                for (entity, child_of) in child_of_query.iter() {
                    if entities_to_check.contains(&child_of.0) && !all_scene_entities.contains(&entity) {
                        found_children.push(entity);
                        all_scene_entities.push(entity);
                    }
                }
                entities_to_check = found_children;
            }

            console_info!("Scene", "Found {} total scene entities", all_scene_entities.len());

            // Process all scene entities
            for entity in &all_scene_entities {
                // Add SceneTabId to all entities
                commands.entity(*entity).insert(SceneTabId(tab_id));

                // Log entity name if available
                if let Ok(editor_entity) = editor_entities.get(*entity) {
                    console_info!("Scene", "  Added SceneTabId to: {:?} ('{}')", entity, editor_entity.name);
                }
            }

            // Remove ChildOf from direct children of DynamicSceneRoot (making them roots)
            for child in &direct_children {
                commands.entity(*child).remove::<ChildOf>();
            }

            // Mark expanded entities by name (now that we have all entities)
            if let Some(ref meta) = editor_meta {
                for entity in &all_scene_entities {
                    if let Ok(editor_entity) = editor_entities.get(*entity) {
                        if meta.expanded_entities.contains(&editor_entity.name) {
                            hierarchy_state.expanded_entities.insert(*entity);
                        }
                    }
                }
            }
        } else {
            console_warn!("Scene", "DynamicSceneRoot {:?} has no children!", root_entity);
        }

        // Remove the metadata resource (it's been applied)
        if editor_meta.is_some() {
            commands.remove_resource::<EditorSceneMetadata>();
        }

        // Despawn the DynamicSceneRoot container
        console_info!("Scene", "Despawning DynamicSceneRoot {:?}", root_entity);
        commands.entity(root_entity).despawn();
    }
}

/// Recursively add SceneTabId to all entities
fn add_tab_ids_recursive(
    commands: &mut Commands,
    children: &Children,
    children_query: &Query<&Children>,
    tab_id: usize,
) {
    for child in children.iter() {
        commands.entity(child).insert(SceneTabId(tab_id));

        if let Ok(grandchildren) = children_query.get(child) {
            add_tab_ids_recursive(commands, grandchildren, children_query, tab_id);
        }
    }
}

/// Recursively mark entities as expanded based on their names
fn mark_expanded_entities_recursive(
    children: &Children,
    children_query: &Query<&Children>,
    editor_entities: &Query<&EditorEntity>,
    expanded_names: &[String],
    hierarchy_state: &mut HierarchyState,
) {
    for child in children.iter() {
        // Check if this entity should be expanded
        if let Ok(editor_entity) = editor_entities.get(child) {
            if expanded_names.contains(&editor_entity.name) {
                hierarchy_state.expanded_entities.insert(child);
            }
        }

        // Recurse into children
        if let Ok(grandchildren) = children_query.get(child) {
            mark_expanded_entities_recursive(
                grandchildren,
                children_query,
                editor_entities,
                expanded_names,
                hierarchy_state,
            );
        }
    }
}

/// System to rehydrate mesh components after scene loading.
/// When scenes are saved, only MeshNodeData is stored (data component).
/// This system creates the actual Mesh3d and MeshMaterial3d components
/// needed for rendering based on the MeshNodeData.
pub fn rehydrate_mesh_components(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<(Entity, &MeshNodeData), Without<Mesh3d>>,
) {
    for (entity, mesh_data) in query.iter() {
        // Create mesh based on type
        let mesh = match mesh_data.mesh_type {
            MeshPrimitiveType::Cube => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            MeshPrimitiveType::Sphere => meshes.add(Sphere::new(0.5).mesh().ico(5).unwrap()),
            MeshPrimitiveType::Cylinder => meshes.add(Cylinder::new(0.5, 1.0)),
            MeshPrimitiveType::Plane => meshes.add(Plane3d::default().mesh().size(2.0, 2.0)),
        };

        // Create standard material
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.7, 0.7),
            perceptual_roughness: 0.9,
            ..default()
        });

        // Add rendering components
        commands.entity(entity).insert((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Visibility::default(),
        ));

        console_info!("Scene", "Rehydrated mesh for entity {:?}", entity);
    }
}

/// System to rehydrate point light components after scene loading.
pub fn rehydrate_point_lights(
    mut commands: Commands,
    query: Query<(Entity, &PointLightData), Without<PointLight>>,
) {
    for (entity, light_data) in query.iter() {
        commands.entity(entity).insert(PointLight {
            color: Color::srgb(light_data.color.x, light_data.color.y, light_data.color.z),
            intensity: light_data.intensity,
            range: light_data.range,
            radius: light_data.radius,
            shadows_enabled: light_data.shadows_enabled,
            ..default()
        });

        console_info!("Scene", "Rehydrated point light for entity {:?}", entity);
    }
}

/// System to rehydrate directional light components after scene loading.
pub fn rehydrate_directional_lights(
    mut commands: Commands,
    query: Query<(Entity, &DirectionalLightData), Without<DirectionalLight>>,
) {
    for (entity, light_data) in query.iter() {
        commands.entity(entity).insert(DirectionalLight {
            color: Color::srgb(light_data.color.x, light_data.color.y, light_data.color.z),
            illuminance: light_data.illuminance,
            shadows_enabled: light_data.shadows_enabled,
            ..default()
        });

        console_info!("Scene", "Rehydrated directional light for entity {:?}", entity);
    }
}

/// System to rehydrate spot light components after scene loading.
pub fn rehydrate_spot_lights(
    mut commands: Commands,
    query: Query<(Entity, &SpotLightData), Without<SpotLight>>,
) {
    for (entity, light_data) in query.iter() {
        commands.entity(entity).insert(SpotLight {
            color: Color::srgb(light_data.color.x, light_data.color.y, light_data.color.z),
            intensity: light_data.intensity,
            range: light_data.range,
            radius: light_data.radius,
            inner_angle: light_data.inner_angle,
            outer_angle: light_data.outer_angle,
            shadows_enabled: light_data.shadows_enabled,
            ..default()
        });

        console_info!("Scene", "Rehydrated spot light for entity {:?}", entity);
    }
}
