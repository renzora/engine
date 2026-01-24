use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use std::path::PathBuf;

use crate::commands::{CommandHistory, SpawnMeshInstanceCommand, queue_command};
use crate::core::{EditorEntity, SceneNode, SelectionState, HierarchyState, AssetBrowserState, SceneTabId, AssetLoadingProgress};
use crate::node_system::components::{MeshInstanceData, NodeTypeMarker, SceneInstanceData};
use crate::node_system::registry::NodeRegistry;
use crate::node_system::scene::loader::spawn_node;
use crate::node_system::scene::format::SceneData;
use crate::node_system::SceneRoot;
use crate::project::CurrentProject;

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
    pub name: String,
}

/// Marker component to indicate a MeshInstance has had its model loading initiated
#[derive(Component)]
pub struct MeshInstanceModelLoading;

/// Marker component to indicate a SceneInstance has had its contents loaded
#[derive(Component)]
pub struct SceneInstanceLoaded;

/// Recursively collect all descendant entities
fn collect_descendants(world: &World, entity: Entity, result: &mut Vec<Entity>) {
    result.push(entity);
    if let Some(children) = world.get::<Children>(entity) {
        for child in children.iter() {
            collect_descendants(world, child, result);
        }
    }
}

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

/// Copy a file to the project's assets folder and return the relative path
fn copy_to_project_assets(source_path: &PathBuf, project: Option<&CurrentProject>) -> Option<String> {
    let project = project?;

    // Get the file name
    let file_name = source_path.file_name()?;

    // Create the assets/models directory if it doesn't exist
    let models_dir = project.path.join("assets").join("models");
    if let Err(e) = std::fs::create_dir_all(&models_dir) {
        error!("Failed to create models directory: {}", e);
        return None;
    }

    // Destination path
    let dest_path = models_dir.join(file_name);

    // Copy the file if it's not already in the project
    if !dest_path.exists() || source_path.canonicalize().ok() != dest_path.canonicalize().ok() {
        if let Err(e) = std::fs::copy(source_path, &dest_path) {
            error!("Failed to copy asset to project: {}", e);
            return None;
        }
        info!("Copied asset to project: {:?}", dest_path);
    }

    // Return relative path from project root (using forward slashes for cross-platform)
    Some(format!("assets/models/{}", file_name.to_string_lossy()))
}

/// System to spawn loaded GLTF models as children of MeshInstance nodes
pub fn spawn_loaded_gltfs(
    mut commands: Commands,
    mut pending_loads: ResMut<PendingGltfLoads>,
    gltf_assets: Res<Assets<Gltf>>,
    mut selection: ResMut<SelectionState>,
    mut hierarchy: ResMut<HierarchyState>,
    scene_roots: Query<(Entity, Option<&SceneTabId>), With<SceneRoot>>,
    current_project: Option<Res<CurrentProject>>,
    mut command_history: ResMut<CommandHistory>,
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

            // Copy to project assets folder and get relative path
            let model_path_str = if let Some(rel_path) = copy_to_project_assets(&pending.path, current_project.as_deref()) {
                rel_path
            } else {
                // Fallback to absolute path if no project or copy failed
                pending.path.to_string_lossy().to_string()
            };

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
                        visible: true,
                        locked: false,
                    },
                    SceneNode,
                    NodeTypeMarker {
                        type_id: "mesh.instance",
                    },
                    MeshInstanceData {
                        model_path: Some(model_path_str.clone()),
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

                // Create undo command for the spawn
                queue_command(
                    &mut command_history,
                    Box::new(SpawnMeshInstanceCommand::new(
                        mesh_instance_entity,
                        pending.name.clone(),
                        transform,
                        Some(model_path_str),
                        scene_root_entity,
                    )),
                );

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
    query: Query<(Entity, &MeshInstanceData), Without<MeshInstanceModelLoading>>,
    current_project: Option<Res<CurrentProject>>,
) {
    for (entity, mesh_data) in query.iter() {
        if let Some(model_path) = &mesh_data.model_path {
            // Resolve the path - if relative, make it absolute using project path
            let resolved_path = if std::path::Path::new(model_path).is_absolute() {
                PathBuf::from(model_path)
            } else if let Some(ref project) = current_project {
                project.path.join(model_path)
            } else {
                PathBuf::from(model_path)
            };

            // Get file name and size
            let name = resolved_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Model")
                .to_string();

            // Mark this entity as having loading initiated (do this after we know the path is valid)
            commands.entity(entity).insert(MeshInstanceModelLoading);

            // Check if file exists
            if !resolved_path.exists() {
                error!("Model file not found: {:?}", resolved_path);
                console_error!("Asset", "Model not found: {}", name);
                continue;
            }

            let file_size = std::fs::metadata(&resolved_path)
                .map(|m| m.len())
                .unwrap_or(0);

            // Load the GLTF asset using the resolved absolute path
            let handle: Handle<Gltf> = asset_server.load(resolved_path.clone());

            info!("Initiated model load for MeshInstance: {:?} -> {:?}", entity, resolved_path);
            console_info!("Asset", "Loading model: {}", name);

            // Track for progress bar
            loading_progress.track(&handle, name.clone(), file_size);

            pending_loads.loads.push(PendingMeshInstanceLoad {
                entity,
                handle,
                name,
            });
        }
    }
}

/// System to spawn loaded models as children of their MeshInstance entities
pub fn spawn_mesh_instance_models(
    mut commands: Commands,
    mut pending_loads: ResMut<PendingMeshInstanceLoads>,
    gltf_assets: Res<Assets<Gltf>>,
    asset_server: Res<AssetServer>,
    mut logged_pending: Local<bool>,
) {
    use bevy::asset::LoadState;

    // Log once when we have pending loads
    if !pending_loads.loads.is_empty() && !*logged_pending {
        info!("Pending mesh instance loads: {}", pending_loads.loads.len());
        *logged_pending = true;
    } else if pending_loads.loads.is_empty() {
        *logged_pending = false;
    }

    let mut completed = Vec::new();

    for (index, pending) in pending_loads.loads.iter().enumerate() {
        // Check load state first for debugging
        let load_state = asset_server.get_load_state(&pending.handle);

        // First try to get the asset directly - this is the most reliable check
        if let Some(gltf) = gltf_assets.get(&pending.handle) {
            // Get the scene handle to spawn
            let scene_handle = if let Some(default_scene) = &gltf.default_scene {
                Some(default_scene.clone())
            } else if !gltf.scenes.is_empty() {
                Some(gltf.scenes[0].clone())
            } else {
                warn!("GLTF file has no scenes for MeshInstance {:?}", pending.entity);
                console_warn!("Asset", "GLTF has no scenes");
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
                console_success!("Asset", "Model loaded: {}", pending.name);
            }

            completed.push(index);
        } else {
            // Check load state for errors
            match load_state {
                Some(LoadState::Failed(err)) => {
                    error!("Failed to load model for MeshInstance {:?}: {:?}", pending.entity, err);
                    console_error!("Asset", "Failed to load: {:?}", err);
                    completed.push(index);
                }
                Some(LoadState::Loading) => {
                    // Still loading, this is normal
                }
                Some(LoadState::NotLoaded) => {
                    // Not started loading yet
                    info!("Asset not yet loading for {:?}", pending.entity);
                }
                Some(LoadState::Loaded) => {
                    // Asset is loaded according to AssetServer but not in Assets<Gltf>
                    // This can happen if there's a type mismatch or the asset is still being processed
                    warn!("Asset marked as loaded but not in Assets<Gltf> for {:?}", pending.entity);
                }
                None => {
                    // Handle doesn't exist or was dropped
                    warn!("Asset handle no longer valid for MeshInstance {:?}", pending.entity);
                    console_error!("Asset", "Asset handle lost");
                    completed.push(index);
                }
            }
        }
    }

    // Remove completed loads (in reverse order to maintain indices)
    for index in completed.into_iter().rev() {
        pending_loads.loads.remove(index);
    }
}

// Re-export console macros for use in this module
use crate::{console_info, console_success, console_error, console_warn};

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
                visible: true,
                locked: false,
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

/// Exclusive system to load scene instance contents
/// This runs as an exclusive system because it needs to spawn entities and access the registry
/// Also handles reloading scene instances when their source scene file is saved
pub fn load_scene_instances(world: &mut World) {
    // First, check for recently saved scenes and mark affected instances for reload
    let (recently_saved, active_tab) = {
        let mut scene_state = world.resource_mut::<crate::core::SceneManagerState>();
        (
            std::mem::take(&mut scene_state.recently_saved_scenes),
            scene_state.active_scene_tab,
        )
    };

    if !recently_saved.is_empty() {
        // Find scene instances that reference any of the saved scenes
        // Only reload instances in OTHER tabs (the active tab already has the correct content)
        let mut query = world.query::<(Entity, &SceneInstanceData, &SceneTabId, &Children)>();
        let instances_to_reload: Vec<(Entity, Vec<Entity>)> = query
            .iter(world)
            .filter_map(|(entity, data, tab_id, children)| {
                // Skip instances in the active tab - they don't need reloading
                if tab_id.0 == active_tab {
                    return None;
                }

                let instance_path = PathBuf::from(&data.scene_path);
                // Check if this instance references any of the recently saved scenes
                let needs_reload = recently_saved.iter().any(|saved_path| {
                    // Compare canonical paths to handle different path representations
                    if let (Ok(saved_canonical), Ok(instance_canonical)) =
                        (saved_path.canonicalize(), instance_path.canonicalize()) {
                        saved_canonical == instance_canonical
                    } else {
                        // Fallback to string comparison
                        saved_path.to_string_lossy() == data.scene_path
                    }
                });
                if needs_reload {
                    Some((entity, children.iter().collect()))
                } else {
                    None
                }
            })
            .collect();

        // Remove SceneInstanceLoaded marker and despawn children for instances that need reload
        for (instance_entity, children_to_despawn) in instances_to_reload {
            // Remove the loaded marker so it gets reprocessed
            if let Ok(mut entity_mut) = world.get_entity_mut(instance_entity) {
                entity_mut.remove::<SceneInstanceLoaded>();
            }

            // Collect all descendants recursively before despawning
            let mut all_descendants = Vec::new();
            for child in children_to_despawn {
                collect_descendants(world, child, &mut all_descendants);
            }

            // Despawn in reverse order (children before parents) to avoid issues
            for entity in all_descendants.into_iter().rev() {
                world.despawn(entity);
            }

            info!("Marked scene instance {:?} for reload", instance_entity);
        }
    }

    // Find all scene instances that haven't been loaded yet
    let instances_to_load: Vec<(Entity, String)> = {
        let mut query = world.query_filtered::<(Entity, &SceneInstanceData), Without<SceneInstanceLoaded>>();
        query.iter(world)
            .map(|(e, data)| (e, data.scene_path.clone()))
            .collect()
    };

    if instances_to_load.is_empty() {
        return;
    }

    for (instance_entity, scene_path) in instances_to_load {
        // Mark as loaded immediately to prevent re-processing
        if let Ok(mut entity_mut) = world.get_entity_mut(instance_entity) {
            entity_mut.insert(SceneInstanceLoaded);
        }

        // Load and parse the scene file
        let scene_data = match std::fs::read_to_string(&scene_path) {
            Ok(content) => {
                match ron::from_str::<SceneData>(&content) {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Failed to parse scene file {}: {}", scene_path, e);
                        continue;
                    }
                }
            }
            Err(e) => {
                error!("Failed to read scene file {}: {}", scene_path, e);
                continue;
            }
        };

        // Spawn the scene contents as children of the instance
        world.resource_scope(|world, registry: Mut<NodeRegistry>| {
            world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
                world.resource_scope(|world, mut materials: Mut<Assets<StandardMaterial>>| {
                    let mut command_queue = CommandQueue::default();
                    let mut commands = Commands::new(&mut command_queue, world);

                    let mut expanded_entities = Vec::new();

                    // Spawn all root nodes from the scene as children of the instance
                    for node_data in &scene_data.root_nodes {
                        spawn_node(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            &registry,
                            node_data,
                            Some(instance_entity),
                            &mut expanded_entities,
                        );
                    }

                    command_queue.apply(world);

                    info!("Loaded scene instance contents from: {} ({} root nodes)",
                          scene_path, scene_data.root_nodes.len());
                });
            });
        });
    }
}
