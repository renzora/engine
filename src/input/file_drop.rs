use bevy::prelude::*;
use std::path::PathBuf;

use crate::core::{EditorEntity, SceneNode, SelectionState, HierarchyState, AssetBrowserState};
use crate::node_system::components::{MeshInstanceData, NodeTypeMarker};

/// Resource to track pending GLB loads
#[derive(Resource, Default)]
pub struct PendingGltfLoads {
    pub loads: Vec<PendingLoad>,
}

pub struct PendingLoad {
    pub handle: Handle<Gltf>,
    pub name: String,
    pub path: PathBuf,
    pub spawn_position: Option<Vec3>,
}

/// Resource to track pending model loads for existing MeshInstance entities (e.g., from scene loading)
#[derive(Resource, Default)]
pub struct PendingMeshInstanceLoads {
    pub loads: Vec<PendingMeshInstanceLoad>,
}

pub struct PendingMeshInstanceLoad {
    pub entity: Entity,
    pub handle: Handle<Gltf>,
}

/// Marker component to indicate a MeshInstance has had its model loading initiated
#[derive(Component)]
pub struct MeshInstanceModelLoading;

/// System to handle file drop events
pub fn handle_file_drop(
    mut events: MessageReader<FileDragAndDrop>,
    asset_server: Res<AssetServer>,
    mut pending_loads: ResMut<PendingGltfLoads>,
) {
    for event in events.read() {
        if let FileDragAndDrop::DroppedFile { path_buf, .. } = event {
            let extension = path_buf
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if extension == "glb" || extension == "gltf" {
                info!("Loading dropped file: {:?}", path_buf);

                // Get the file name for the entity name
                let name = path_buf
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Model")
                    .to_string();

                // Load the GLTF asset
                let handle: Handle<Gltf> = asset_server.load(path_buf.clone());

                pending_loads.loads.push(PendingLoad {
                    handle,
                    name,
                    path: path_buf.clone(),
                    spawn_position: None, // Regular file drop spawns at origin
                });
            }
        }
    }
}

/// System to handle assets dragged from the assets panel to viewport
pub fn handle_asset_panel_drop(
    mut assets: ResMut<AssetBrowserState>,
    asset_server: Res<AssetServer>,
    mut pending_loads: ResMut<PendingGltfLoads>,
) {
    if let Some((path, position)) = assets.pending_asset_drop.take() {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if extension == "glb" || extension == "gltf" {
            info!("Loading dropped asset: {:?} at position {:?}", path, position);

            // Get the file name for the entity name
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Model")
                .to_string();

            // Use absolute path for loading - asset server can handle absolute paths
            let load_path = path.clone();

            // Load the GLTF asset
            let handle: Handle<Gltf> = asset_server.load(load_path);

            pending_loads.loads.push(PendingLoad {
                handle,
                name,
                path,
                spawn_position: Some(position),
            });
        }
    }
}

/// System to spawn loaded GLTF models as children of MeshInstance nodes
pub fn spawn_loaded_gltfs(
    mut commands: Commands,
    mut pending_loads: ResMut<PendingGltfLoads>,
    gltf_assets: Res<Assets<Gltf>>,
    mut selection: ResMut<SelectionState>,
    mut hierarchy: ResMut<HierarchyState>,
) {
    let mut completed = Vec::new();

    for (index, pending) in pending_loads.loads.iter().enumerate() {
        if let Some(gltf) = gltf_assets.get(&pending.handle) {
            // Use spawn position or default to origin
            let transform = match pending.spawn_position {
                Some(pos) => Transform::from_translation(pos),
                None => Transform::default(),
            };

            // Store the model path as a string for MeshInstanceData
            let model_path_str = pending.path.to_string_lossy().to_string();

            // Get the scene handle to spawn
            let scene_handle = if let Some(default_scene) = &gltf.default_scene {
                Some(default_scene.clone())
            } else if !gltf.scenes.is_empty() {
                Some(gltf.scenes[0].clone())
            } else {
                warn!("GLTF file has no scenes: {:?}", pending.path);
                completed.push(index);
                continue;
            };

            if let Some(scene) = scene_handle {
                // Create the MeshInstance parent node
                let mesh_instance_entity = commands.spawn((
                    transform,
                    Visibility::default(),
                    EditorEntity {
                        name: pending.name.clone(),
                    },
                    SceneNode,
                    NodeTypeMarker {
                        type_id: "mesh.instance",
                    },
                    MeshInstanceData {
                        model_path: Some(model_path_str),
                    },
                )).id();

                // Spawn the GLTF scene as a child of the MeshInstance
                commands.spawn((
                    SceneRoot(scene),
                    Transform::default(),
                    Visibility::default(),
                    ChildOf(mesh_instance_entity),
                ));

                info!("Spawned MeshInstance '{}' with model as child", pending.name);

                // Auto-select the MeshInstance parent
                selection.selected_entity = Some(mesh_instance_entity);
                // Auto-expand the MeshInstance in hierarchy to show the model child
                hierarchy.expanded_entities.insert(mesh_instance_entity);
            }

            completed.push(index);
        }
    }

    // Remove completed loads (in reverse order to maintain indices)
    for index in completed.into_iter().rev() {
        pending_loads.loads.remove(index);
    }
}

/// System to detect MeshInstance entities that need their models loaded (e.g., after scene load)
pub fn check_mesh_instance_models(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut pending_loads: ResMut<PendingMeshInstanceLoads>,
    query: Query<(Entity, &MeshInstanceData), (Without<MeshInstanceModelLoading>, Without<Children>)>,
) {
    for (entity, mesh_data) in query.iter() {
        if let Some(model_path) = &mesh_data.model_path {
            // Mark this entity as having loading initiated
            commands.entity(entity).insert(MeshInstanceModelLoading);

            // Load the GLTF asset
            let handle: Handle<Gltf> = asset_server.load(model_path.clone());

            pending_loads.loads.push(PendingMeshInstanceLoad {
                entity,
                handle,
            });

            info!("Initiated model load for MeshInstance: {:?} -> {}", entity, model_path);
        }
    }
}

/// System to spawn loaded models as children of their MeshInstance entities
pub fn spawn_mesh_instance_models(
    mut commands: Commands,
    mut pending_loads: ResMut<PendingMeshInstanceLoads>,
    gltf_assets: Res<Assets<Gltf>>,
) {
    let mut completed = Vec::new();

    for (index, pending) in pending_loads.loads.iter().enumerate() {
        if let Some(gltf) = gltf_assets.get(&pending.handle) {
            // Get the scene handle to spawn
            let scene_handle = if let Some(default_scene) = &gltf.default_scene {
                Some(default_scene.clone())
            } else if !gltf.scenes.is_empty() {
                Some(gltf.scenes[0].clone())
            } else {
                warn!("GLTF file has no scenes for MeshInstance {:?}", pending.entity);
                completed.push(index);
                continue;
            };

            if let Some(scene) = scene_handle {
                // Spawn the GLTF scene as a child of the existing MeshInstance
                commands.spawn((
                    SceneRoot(scene),
                    Transform::default(),
                    Visibility::default(),
                    ChildOf(pending.entity),
                ));

                info!("Spawned model as child of MeshInstance {:?}", pending.entity);
            }

            completed.push(index);
        }
    }

    // Remove completed loads (in reverse order to maintain indices)
    for index in completed.into_iter().rev() {
        pending_loads.loads.remove(index);
    }
}
