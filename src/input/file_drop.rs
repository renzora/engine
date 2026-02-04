#![allow(dead_code)]

use bevy::prelude::*;
use bevy::camera::primitives::Aabb;
use bevy::scene::DynamicSceneRoot;
use std::path::PathBuf;

use crate::commands::{CommandHistory, SpawnMeshInstanceCommand, queue_command};
use crate::core::{EditorEntity, SceneNode, SelectionState, HierarchyState, AssetBrowserState, SceneTabId, AssetLoadingProgress, ViewportCamera, ViewportState};
use crate::shared::{MaterialData, MeshInstanceData, SceneInstanceData, GltfAnimations, GltfAnimationHandles, GltfAnimationStorage};
use crate::spawn::EditorSceneRoot;
use crate::project::CurrentProject;
use crate::shared::Sprite2DData;
use crate::blueprint::{
    BlueprintFile, compile_material_blueprint, create_material_from_blueprint,
    preview::{chain_has_procedural_pattern, generate_procedural_texture},
};

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

/// Marker component to indicate a MaterialData has had its material applied
/// Stores the path that was applied to detect when it needs to be reloaded
#[derive(Component)]
pub struct MaterialApplied {
    /// The path that was last applied (to detect changes)
    pub applied_path: Option<String>,
}

/// Recursively collect all descendant entities
fn collect_descendants(world: &World, entity: Entity, result: &mut Vec<Entity>) {
    result.push(entity);
    if let Some(children) = world.get::<Children>(entity) {
        for child in children.iter() {
            collect_descendants(world, child, result);
        }
    }
}

/// System to handle file drop events (processes files queued by egui UI)
pub fn handle_file_drop(
    asset_server: Res<AssetServer>,
    mut pending_loads: ResMut<PendingGltfLoads>,
    mut loading_progress: ResMut<AssetLoadingProgress>,
    mut assets: ResMut<AssetBrowserState>,
) {
    // Process files that were dropped in the viewport (queued by egui)
    let files_to_spawn = std::mem::take(&mut assets.files_to_spawn);

    for path_buf in files_to_spawn {
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
            loading_progress.track(&handle, file_size);

            pending_loads.loads.push(PendingLoad {
                handle,
                name,
                path: path_buf.clone(),
                spawn_position: None, // Regular file drop spawns at origin
            });
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
            loading_progress.track(&handle, file_size);

            pending_loads.loads.push(PendingLoad {
                handle,
                name,
                path,
                spawn_position: Some(position),
            });
        }
    }
}

/// System to handle image drops from the assets panel to viewport
/// Creates a Sprite2D in 2D mode or a textured plane in 3D mode
pub fn handle_image_panel_drop(
    mut commands: Commands,
    mut assets: ResMut<AssetBrowserState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    mut selection: ResMut<SelectionState>,
    mut hierarchy: ResMut<HierarchyState>,
    scene_roots: Query<(Entity, Option<&SceneTabId>), With<EditorSceneRoot>>,
    current_project: Option<Res<CurrentProject>>,
) {
    let Some(image_drop) = assets.pending_image_drop.take() else {
        return;
    };

    // Get the file name for the entity name
    let name = image_drop.path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Image")
        .to_string();

    // Copy to project assets folder and get relative path
    let texture_path = if let Some(rel_path) = copy_image_to_project_assets(&image_drop.path, current_project.as_deref()) {
        rel_path
    } else {
        // Fallback to absolute path if no project or copy failed
        image_drop.path.to_string_lossy().to_string()
    };

    // Find the scene root to parent new entities to
    let scene_root_entity = scene_roots.iter().next().map(|(e, _)| e);

    if image_drop.is_2d_mode {
        // Create a Sprite2D node
        let mut sprite_entity = commands.spawn((
            Transform::from_translation(image_drop.position),
            Visibility::default(),
            EditorEntity {
                name: name.clone(),
                tag: String::new(),
                visible: true,
                locked: false,
            },
            SceneNode,
            Sprite2DData {
                texture_path,
                color: Vec4::ONE,
                flip_x: false,
                flip_y: false,
                anchor: Vec2::new(0.5, 0.5),
            },
        ));

        // Parent to scene root if one exists
        if let Some(root) = scene_root_entity {
            sprite_entity.insert(ChildOf(root));
        }

        let entity = sprite_entity.id();

        info!("Spawned Sprite2D '{}' with texture", name);

        // Auto-select the new entity
        selection.selected_entity = Some(entity);
        if let Some(root) = scene_root_entity {
            hierarchy.expanded_entities.insert(root);
        }
    } else {
        // Create a textured plane in 3D mode
        // Load the texture
        let texture_handle: Handle<Image> = asset_server.load(image_drop.path.clone());

        // Create a plane mesh (default 1x1, lying on the ground facing up)
        let plane_mesh = meshes.add(Plane3d::default().mesh().size(1.0, 1.0));

        // Create a material with the texture
        let plane_material = materials.add(StandardMaterial {
            base_color_texture: Some(texture_handle),
            alpha_mode: AlphaMode::Blend, // Support transparent images
            unlit: false,
            double_sided: true,
            cull_mode: None,
            ..default()
        });

        // Create the plane entity
        let mut plane_entity = commands.spawn((
            Transform::from_translation(image_drop.position),
            Visibility::default(),
            EditorEntity {
                name: name.clone(),
                tag: String::new(),
                visible: true,
                locked: false,
            },
            SceneNode,
            TexturedPlaneData {
                texture_path,
                width: 1.0,
                height: 1.0,
            },
        ));

        // Parent to scene root if one exists
        if let Some(root) = scene_root_entity {
            plane_entity.insert(ChildOf(root));
        }

        let parent_entity = plane_entity.id();

        // Spawn the actual mesh as a child
        commands.spawn((
            Mesh3d(plane_mesh),
            MeshMaterial3d(plane_material),
            Transform::default(),
            Visibility::default(),
            ChildOf(parent_entity),
        ));

        info!("Spawned textured plane '{}' with image", name);

        // Auto-select the new entity
        selection.selected_entity = Some(parent_entity);
        if let Some(root) = scene_root_entity {
            hierarchy.expanded_entities.insert(root);
        }
        hierarchy.expanded_entities.insert(parent_entity);
    }
}

/// Copy an image file to the project's assets folder and return the relative path
fn copy_image_to_project_assets(source_path: &PathBuf, project: Option<&CurrentProject>) -> Option<String> {
    let project = project?;

    // Get the file name
    let file_name = source_path.file_name()?;

    // Create the assets/textures directory if it doesn't exist
    let textures_dir = project.path.join("assets").join("textures");
    if let Err(e) = std::fs::create_dir_all(&textures_dir) {
        error!("Failed to create textures directory: {}", e);
        return None;
    }

    // Destination path
    let dest_path = textures_dir.join(file_name);

    // Copy the file if it's not already in the project
    if !dest_path.exists() || source_path.canonicalize().ok() != dest_path.canonicalize().ok() {
        if let Err(e) = std::fs::copy(source_path, &dest_path) {
            error!("Failed to copy image to project: {}", e);
            return None;
        }
        info!("Copied image to project: {:?}", dest_path);
    }

    // Return relative path from project root (using forward slashes for cross-platform)
    Some(format!("assets/textures/{}", file_name.to_string_lossy()))
}

/// Data component for textured plane nodes
#[derive(Component, Clone, Debug)]
pub struct TexturedPlaneData {
    /// Path to the texture file (relative to assets folder)
    pub texture_path: String,
    /// Plane width
    pub width: f32,
    /// Plane height
    pub height: f32,
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
    scene_roots: Query<(Entity, Option<&SceneTabId>), With<EditorSceneRoot>>,
    current_project: Option<Res<CurrentProject>>,
    mut command_history: ResMut<CommandHistory>,
    mut animation_storage: ResMut<GltfAnimationStorage>,
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
                // Extract animations from the GLTF
                let (gltf_animations, animation_handles) = if !gltf.named_animations.is_empty() {
                    let mut clip_names = Vec::new();
                    let mut clips = Vec::new();
                    for (name, handle) in &gltf.named_animations {
                        clip_names.push(name.to_string());
                        clips.push(handle.clone());
                    }
                    info!("Found {} animations in GLTF: {:?}", clip_names.len(), clip_names);
                    (Some(GltfAnimations::with_clip_names(clip_names)), Some(GltfAnimationHandles::with_clips(clips)))
                } else {
                    (None, None)
                };

                // Create the MeshInstance parent node
                let mut mesh_instance = commands.spawn((
                    transform,
                    Visibility::default(),
                    EditorEntity {
                        name: pending.name.clone(),
                        tag: String::new(),
                        visible: true,
                        locked: false,
                    },
                    SceneNode,
                    MeshInstanceData {
                        model_path: Some(model_path_str.clone()),
                    },
                ));

                // Add GltfAnimations component if animations were found
                if let Some(anims) = gltf_animations {
                    mesh_instance.insert(anims);
                }

                // Parent to scene root if one exists
                if let Some(root) = scene_root_entity {
                    mesh_instance.insert(ChildOf(root));
                }

                let mesh_instance_entity = mesh_instance.id();

                // Store animation handles in the resource (separate from component for Bevy compatibility)
                if let Some(handles) = animation_handles {
                    animation_storage.handles.insert(mesh_instance_entity, handles);
                }

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
            loading_progress.track(&handle, file_size);

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
    mut animation_storage: ResMut<GltfAnimationStorage>,
    children_query: Query<&Children>,
    scene_roots: Query<Entity, With<SceneRoot>>,
    mut gltf_anims_query: Query<&mut GltfAnimations>,
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
            // Check if this entity already has a SceneRoot child (to prevent duplicates)
            let already_has_scene = if let Ok(children) = children_query.get(pending.entity) {
                children.iter().any(|child| scene_roots.get(child).is_ok())
            } else {
                false
            };

            if already_has_scene {
                info!("MeshInstance {:?} already has a SceneRoot child, skipping spawn", pending.entity);
                completed.push(index);
                continue;
            }

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

            // Extract animations from the GLTF if present
            if !gltf.named_animations.is_empty() {
                let mut clip_names = Vec::new();
                let mut clips = Vec::new();
                for (name, handle) in &gltf.named_animations {
                    clip_names.push(name.to_string());
                    clips.push(handle.clone());
                }
                info!("Found {} animations in GLTF for MeshInstance {:?}: {:?}",
                    clip_names.len(), pending.entity, clip_names);

                // Store animation handles in the resource
                animation_storage.handles.insert(pending.entity, GltfAnimationHandles::with_clips(clips));

                // Check if entity already has GltfAnimations component
                if let Ok(mut existing) = gltf_anims_query.get_mut(pending.entity) {
                    // Entity already has GltfAnimations (e.g., loaded from scene)
                    // Reset initialized to trigger graph setup, and ensure clip_names are set
                    existing.initialized = false;
                    if existing.clip_names.is_empty() {
                        existing.clip_names = clip_names;
                    }
                    info!("Reset GltfAnimations for {:?} to trigger animation setup", pending.entity);
                } else {
                    // Add new GltfAnimations component
                    commands.entity(pending.entity).insert(GltfAnimations::with_clip_names(clip_names));
                }
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
                tag: String::new(),
                visible: true,
                locked: false,
            },
            SceneNode,
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

/// Exclusive system to load scene instance contents using Bevy's DynamicScene format
/// This runs as an exclusive system because it needs to spawn entities
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

    // Load scene instances using Bevy's DynamicScene system
    world.resource_scope(|world, asset_server: Mut<AssetServer>| {
        for (instance_entity, scene_path) in instances_to_load {
            // Mark as loaded immediately to prevent re-processing
            if let Ok(mut entity_mut) = world.get_entity_mut(instance_entity) {
                entity_mut.insert(SceneInstanceLoaded);
            }

            // Convert scene_path to .ron format path
            let scene_file_path = PathBuf::from(&scene_path);
            let load_path = if scene_file_path.extension().map_or(false, |e| e == "ron") {
                scene_file_path.clone()
            } else {
                scene_file_path.with_extension("ron")
            };

            // Check if file exists
            if !load_path.exists() {
                error!("Scene file not found for instance: {:?}", load_path);
                continue;
            }

            // Load the scene using asset server
            let scene_handle: Handle<DynamicScene> = asset_server.load(load_path.clone());

            // Spawn a DynamicSceneRoot as a child of the instance entity
            // When the scene loads, its contents will become children of this root
            world.spawn((
                DynamicSceneRoot(scene_handle),
                Transform::default(),
                Visibility::default(),
                ChildOf(instance_entity),
            ));

            info!("Loading scene instance from: {}", load_path.display());
        }
    });
}

/// System to handle material blueprint drops from the assets panel to viewport
/// Picks the mesh entity under the cursor and adds a MaterialData component
pub fn handle_material_panel_drop(
    mut commands: Commands,
    mut assets: ResMut<AssetBrowserState>,
    viewport: Res<ViewportState>,
    camera_query: Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
    mesh_query: Query<(Entity, &GlobalTransform, Option<&Aabb>), With<Mesh3d>>,
    parent_query: Query<&ChildOf>,
    editor_entity_query: Query<&EditorEntity>,
) {
    let Some(drop) = assets.pending_material_drop.take() else {
        return;
    };

    // Pick the mesh entity under the cursor
    let target_entity = pick_mesh_at_cursor(
        drop.cursor_pos,
        &viewport,
        &camera_query,
        &mesh_query,
    );

    let Some(mesh_entity) = target_entity else {
        console_warn!("Material", "No mesh found under cursor to apply material");
        return;
    };

    // Get the material path as a string
    let material_path = drop.path.to_string_lossy().to_string();

    // Add MaterialData component to the entity - the apply system will handle compilation
    commands.entity(mesh_entity).insert(MaterialData {
        material_path: Some(material_path.clone()),
    });

    // Get a display name for the entity
    let entity_name = get_entity_display_name(
        mesh_entity,
        &parent_query,
        &editor_entity_query,
    );

    console_info!("Material", "Added material to {}", entity_name);
    info!("Added MaterialData '{}' to entity {:?}", material_path, mesh_entity);
}

/// System to apply materials to entities with MaterialData components
/// Watches for new or changed MaterialData and compiles/applies the material
pub fn apply_material_data(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &MaterialData, Option<&MaterialApplied>)>,
    parent_query: Query<&ChildOf>,
    editor_entity_query: Query<&EditorEntity>,
    current_project: Option<Res<CurrentProject>>,
) {
    for (entity, material_data, applied) in query.iter() {
        // Check if we need to apply/update the material
        let needs_apply = match (&material_data.material_path, applied) {
            (Some(_), None) => true, // New material, not yet applied
            (Some(path), Some(applied_marker)) => {
                // Material changed
                applied_marker.applied_path.as_ref() != Some(path)
            }
            (None, Some(_)) => true, // Material removed, need to clear
            (None, None) => false,   // No material, nothing to do
        };

        if !needs_apply {
            continue;
        }

        let Some(material_path) = &material_data.material_path else {
            // Material was removed, remove the applied marker
            commands.entity(entity).remove::<MaterialApplied>();
            continue;
        };

        // Load and compile the blueprint
        let path = PathBuf::from(material_path);
        let blueprint_file = match BlueprintFile::load(&path) {
            Ok(file) => file,
            Err(e) => {
                console_error!("Material", "Failed to load blueprint: {}", e);
                // Mark as applied (with error) to prevent repeated attempts
                commands.entity(entity).insert(MaterialApplied {
                    applied_path: Some(material_path.clone()),
                });
                continue;
            }
        };

        // Compile the material blueprint
        let compiled = compile_material_blueprint(&blueprint_file.graph);
        if !compiled.is_ok() {
            for err in &compiled.errors {
                console_error!("Material", "Compilation error: {}", err);
            }
            // Mark as applied (with error) to prevent repeated attempts
            commands.entity(entity).insert(MaterialApplied {
                applied_path: Some(material_path.clone()),
            });
            continue;
        }

        // Log any warnings
        for warn in &compiled.warnings {
            console_warn!("Material", "Compilation warning: {}", warn);
        }

        // Get the project path for resolving relative texture paths
        let project_path = current_project.as_ref().map(|p| &p.path);

        // Create and apply the material (extracts PBR values from the graph)
        let mut material = create_material_from_blueprint(&blueprint_file.graph, &compiled, &asset_server, project_path);

        // Check if the material has procedural patterns - if so, generate a texture
        if let Some(output_node) = blueprint_file.graph.nodes.iter().find(|n| {
            n.node_type == "shader/pbr_output" || n.node_type == "shader/unlit_output"
        }) {
            if chain_has_procedural_pattern(&blueprint_file.graph, output_node, "base_color") {
                console_info!("Material", "Generating procedural texture for '{}'...", compiled.name);

                // Generate a 256x256 texture from the procedural material
                if let Some(proc_image) = generate_procedural_texture(
                    &blueprint_file.graph,
                    output_node,
                    "base_color",
                    256,
                ) {
                    let texture_handle = images.add(proc_image);
                    material.base_color_texture = Some(texture_handle);
                    material.base_color = Color::WHITE; // Let texture control color
                    console_success!("Material", "Procedural texture generated for '{}'", compiled.name);
                }
            }
        }

        let material_handle = materials.add(material);

        commands.entity(entity).insert((
            MeshMaterial3d(material_handle),
            MaterialApplied {
                applied_path: Some(material_path.clone()),
            },
        ));

        // Get a display name for the entity
        let entity_name = get_entity_display_name(
            entity,
            &parent_query,
            &editor_entity_query,
        );

        console_success!("Material", "Applied '{}' to {}", compiled.name, entity_name);
        info!("Applied material blueprint '{}' to entity {:?}", compiled.name, entity);
    }
}

/// Pick the mesh entity closest to the camera at the given cursor position
fn pick_mesh_at_cursor(
    cursor_pos: Vec2,
    viewport: &ViewportState,
    camera_query: &Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
    mesh_query: &Query<(Entity, &GlobalTransform, Option<&Aabb>), With<Mesh3d>>,
) -> Option<Entity> {
    // Get ray from camera through cursor
    let ray = get_cursor_ray_from_pos(cursor_pos, viewport, camera_query)?;

    // Check all mesh entities for intersection
    let mut closest_hit: Option<(Entity, f32)> = None;

    for (entity, transform, aabb_opt) in mesh_query.iter() {
        // Use AABB if available, otherwise use a default bounding box
        let (center, half_extents) = if let Some(aabb) = aabb_opt {
            (aabb.center, aabb.half_extents)
        } else {
            // Default small bounding box for meshes without AABB
            (Vec3A::ZERO, Vec3A::splat(0.5))
        };

        // Transform AABB to world space
        let world_center = transform.transform_point(Vec3::from(center));
        let scale = transform.compute_transform().scale;
        let world_half_extents = Vec3::from(half_extents) * scale;

        // Ray-AABB intersection test
        if let Some(t) = ray_aabb_intersection(&ray, world_center, world_half_extents) {
            if closest_hit.map_or(true, |(_, d)| t < d) {
                closest_hit = Some((entity, t));
            }
        }
    }

    closest_hit.map(|(e, _)| e)
}

/// Get a ray from a cursor position in viewport coordinates
fn get_cursor_ray_from_pos(
    cursor_pos: Vec2,
    viewport: &ViewportState,
    camera_query: &Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
) -> Option<Ray3d> {
    let viewport_pos = viewport.position;
    let viewport_size = viewport.size;

    // Convert screen position to local viewport position
    let local_x = cursor_pos.x - viewport_pos[0];
    let local_y = cursor_pos.y - viewport_pos[1];

    // Check if within viewport bounds
    if local_x < 0.0 || local_y < 0.0 || local_x > viewport_size[0] || local_y > viewport_size[1] {
        return None;
    }

    let viewport_cursor = Vec2::new(local_x, local_y);

    let (camera, camera_transform) = camera_query.single().ok()?;
    camera.viewport_to_world(camera_transform, viewport_cursor).ok()
}

/// Ray-AABB intersection test, returns distance to intersection or None
fn ray_aabb_intersection(ray: &Ray3d, center: Vec3, half_extents: Vec3) -> Option<f32> {
    let min = center - half_extents;
    let max = center + half_extents;

    let inv_dir = Vec3::new(
        if ray.direction.x.abs() > 1e-6 { 1.0 / ray.direction.x } else { f32::MAX },
        if ray.direction.y.abs() > 1e-6 { 1.0 / ray.direction.y } else { f32::MAX },
        if ray.direction.z.abs() > 1e-6 { 1.0 / ray.direction.z } else { f32::MAX },
    );

    let t1 = (min - ray.origin) * inv_dir;
    let t2 = (max - ray.origin) * inv_dir;

    let t_min = t1.min(t2);
    let t_max = t1.max(t2);

    let t_enter = t_min.x.max(t_min.y).max(t_min.z);
    let t_exit = t_max.x.min(t_max.y).min(t_max.z);

    if t_enter <= t_exit && t_exit > 0.0 {
        Some(if t_enter > 0.0 { t_enter } else { t_exit })
    } else {
        None
    }
}

/// Get a display name for an entity, checking parent for EditorEntity name
fn get_entity_display_name(
    entity: Entity,
    parent_query: &Query<&ChildOf>,
    editor_entity_query: &Query<&EditorEntity>,
) -> String {
    // First check if this entity has an EditorEntity name
    if let Ok(editor) = editor_entity_query.get(entity) {
        return editor.name.clone();
    }

    // Check parent entity for EditorEntity name
    if let Ok(child_of) = parent_query.get(entity) {
        if let Ok(parent_editor) = editor_entity_query.get(child_of.0) {
            return format!("{} (child mesh)", parent_editor.name);
        }
    }

    // Fallback to entity ID
    format!("Entity {:?}", entity)
}
