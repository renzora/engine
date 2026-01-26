//! Scene loading using Bevy's DynamicScene system

use bevy::prelude::*;
use bevy::scene::{DynamicSceneRoot, SceneInstanceReady};
use std::path::Path;

use crate::core::{EditorEntity, SceneTabId, HierarchyState, OrbitCameraState};
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
