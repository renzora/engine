use bevy::prelude::*;
use std::path::PathBuf;

use crate::core::{EditorEntity, SceneNode, SelectionState, HierarchyState, AssetBrowserState, SceneTabId, AssetLoadingProgress};
use crate::node_system::components::{MeshInstanceData, NodeTypeMarker, SceneInstanceData};
use crate::node_system::SceneRoot;

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
    mut loading_progress: ResMut<AssetLoadingProgress>,
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

                // Get file size
                let file_size = std::fs::metadata(&path_buf)
                    .map(|m| m.len())
                    .unwrap_or(0);

                // Load the GLTF asset
                let handle: Handle<Gltf> = asset_server.load(path_buf.clone());

                // Track for progress bar
                loading_progress.track(&handle, name.clone(), file_size);

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
    mut loading_progress: ResMut<AssetLoadingProgress>,
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

            // Get file size
            let file_size = std::fs::metadata(&path)
                .map(|m| m.len())
                .unwrap_or(0);

            // Use absolute path for loading - asset server can handle absolute paths
            let load_path = path.clone();

            // Load the GLTF asset
            let handle: Handle<Gltf> = asset_server.load(load_path);

            // Track for progress bar
            loading_progress.track(&handle, name.clone(), file_size);

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
    scene_roots: Query<(Entity, Option<&SceneTabId>), With<SceneRoot>>,
) {
    let mut completed = Vec::new();

    // Find the scene root to parent new meshes to (use first available for now)
    let scene_root_entity = scene_roots.iter().next().map(|(e, _)| e);

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
                let mut mesh_instance = commands.spawn((
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
                ));

                // Parent to scene root if one exists
                if let Some(root) = scene_root_entity {
                    mesh_instance.insert(ChildOf(root));
                }

                let mesh_instance_entity = mesh_instance.id();

                // Spawn the GLTF scene as a child of the MeshInstance
                commands.spawn((
                    bevy::scene::SceneRoot(scene),
                    Transform::default(),
                    Visibility::default(),
                    ChildOf(mesh_instance_entity),
                ));

                info!("Spawned MeshInstance '{}' with model as child", pending.name);

                // Auto-select the MeshInstance parent
                selection.selected_entity = Some(mesh_instance_entity);
                // Auto-expand the scene root and MeshInstance in hierarchy
                if let Some(root) = scene_root_entity {
                    hierarchy.expanded_entities.insert(root);
                }
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
    mut loading_progress: ResMut<AssetLoadingProgress>,
    query: Query<(Entity, &MeshInstanceData), (Without<MeshInstanceModelLoading>, Without<Children>)>,
) {
    for (entity, mesh_data) in query.iter() {
        if let Some(model_path) = &mesh_data.model_path {
            // Mark this entity as having loading initiated
            commands.entity(entity).insert(MeshInstanceModelLoading);

            // Get file name and size
            let path = std::path::Path::new(model_path);
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Model")
                .to_string();
            let file_size = std::fs::metadata(path)
                .map(|m| m.len())
                .unwrap_or(0);

            // Load the GLTF asset
            let handle: Handle<Gltf> = asset_server.load(model_path.clone());

            // Track for progress bar
            loading_progress.track(&handle, name, file_size);

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

/// System to handle scene files dropped into the hierarchy
/// Creates a SceneInstance node that references the scene file instead of expanding all nodes
pub fn handle_scene_hierarchy_drop(
    mut commands: Commands,
    mut assets: ResMut<AssetBrowserState>,
    mut selection: ResMut<SelectionState>,
    mut hierarchy: ResMut<HierarchyState>,
) {
    if let Some((scene_path, parent_entity)) = assets.pending_scene_drop.take() {
        info!("Creating scene instance from: {:?}", scene_path);

        // Get the scene name from the file path
        let scene_name = scene_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Scene")
            .to_string();

        // Create a SceneInstance node that references the scene file
        let mut scene_instance = commands.spawn((
            Transform::default(),
            Visibility::default(),
            EditorEntity {
                name: scene_name,
            },
            SceneNode,
            NodeTypeMarker {
                type_id: "scene.instance",
            },
            SceneInstanceData {
                scene_path: scene_path.to_string_lossy().to_string(),
                is_open: false,
            },
        ));

        // Parent to the target entity if specified
        if let Some(parent) = parent_entity {
            scene_instance.insert(ChildOf(parent));
            hierarchy.expanded_entities.insert(parent);
        }

        let entity = scene_instance.id();

        // Select the new scene instance
        selection.selected_entity = Some(entity);

        info!("Scene instance created: {:?}", entity);
    }
}
