//! Scene loading using Bevy's DynamicScene system

#![allow(dead_code)]

use bevy::prelude::*;
use bevy::scene::{DynamicSceneRoot, SceneInstanceReady};
#[cfg(feature = "solari")]
use bevy::solari::scene::RaytracingMesh3d;
use std::path::Path;

use crate::component_system::components::clouds::CloudDomeMarker;
use crate::core::{EditorEntity, NodeIcon, SceneNode, SceneTabId, HierarchyState, OrbitCameraState};
use crate::gizmo::meshes::GizmoMesh;
use crate::component_system::{
    MeshNodeData, MeshPrimitiveType,
    PointLightData, DirectionalLightData, SpotLightData, SunData,
    CameraNodeData, CameraRigData, Camera2DData,
};
use crate::terrain::{TerrainData, TerrainChunkData, TerrainChunkOf, generate_chunk_mesh};
use crate::component_system::components::terrain::DEFAULT_TERRAIN_MATERIAL;
use crate::component_system::MaterialData;
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
        let mesh = crate::spawn::meshes::create_mesh_for_type(&mut meshes, mesh_data.mesh_type);

        // Create standard material
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.7, 0.7),
            perceptual_roughness: 0.9,
            ..default()
        });

        // Add rendering components
        // Note: RaytracingMesh3d is NOT added here - it's managed by sync_rendering_settings
        // based on whether Solari is enabled in the scene
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

/// System to rehydrate sun components after scene loading.
pub fn rehydrate_sun_lights(
    mut commands: Commands,
    query: Query<(Entity, &SunData), Without<DirectionalLight>>,
) {
    for (entity, sun_data) in query.iter() {
        let dir = sun_data.direction();
        commands.entity(entity).insert((
            DirectionalLight {
                color: Color::srgb(sun_data.color.x, sun_data.color.y, sun_data.color.z),
                illuminance: sun_data.illuminance,
                shadows_enabled: sun_data.shadows_enabled,
                ..default()
            },
            Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, dir)),
        ));

        console_info!("Scene", "Rehydrated sun light for entity {:?}", entity);
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

/// System to rehydrate terrain chunk meshes after scene loading.
/// Handles two cases:
/// 1. Chunks were saved with their TerrainChunkData - just create the mesh
/// 2. Only TerrainData was saved (no chunks) - create new flat chunks
pub fn rehydrate_terrain_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    terrain_query: Query<(Entity, &TerrainData)>,
    // Chunks that have data but no mesh yet (loaded from scene)
    // Note: TerrainChunkOf isn't serialized, so we use ChildOf to find the parent
    chunks_needing_mesh: Query<(Entity, &TerrainChunkData, &ChildOf), (Without<Mesh3d>, Without<TerrainChunkOf>)>,
    // Chunks that have data but NO ChildOf (orphaned chunks - shouldn't happen but let's check)
    orphan_chunks: Query<(Entity, &TerrainChunkData), (Without<Mesh3d>, Without<TerrainChunkOf>, Without<ChildOf>)>,
    // All existing chunks (to check if terrain has any)
    existing_chunks: Query<&TerrainChunkOf>,
) {
    // Debug: check for orphan chunks (have TerrainChunkData but no ChildOf)
    let orphan_count = orphan_chunks.iter().count();
    if orphan_count > 0 {
        console_warn!("Scene", "Found {} orphan chunks with TerrainChunkData but no ChildOf!", orphan_count);
        for (entity, chunk_data) in orphan_chunks.iter() {
            console_warn!("Scene", "  Orphan chunk ({}, {}) entity {:?}",
                chunk_data.chunk_x, chunk_data.chunk_z, entity);
        }
    }

    // Track which terrains have had chunks rehydrated (commands are deferred, so we can't
    // rely on existing_chunks query seeing the newly added TerrainChunkOf components)
    let mut terrains_with_rehydrated_chunks = std::collections::HashSet::new();

    // Debug: count chunks needing mesh
    let chunks_count = chunks_needing_mesh.iter().count();
    if chunks_count > 0 {
        console_info!("Scene", "Found {} chunks needing mesh rehydration", chunks_count);
    }

    // First, handle chunks that were loaded from the scene file (have data but no mesh)
    for (chunk_entity, chunk_data, child_of) in chunks_needing_mesh.iter() {
        // Use ChildOf to find the parent terrain
        let parent_entity = child_of.0;

        // Get the parent terrain's data
        let Ok((_, terrain_data)) = terrain_query.get(parent_entity) else {
            console_warn!("Scene", "Chunk ({}, {}) has ChildOf {:?} but parent has no TerrainData!",
                chunk_data.chunk_x, chunk_data.chunk_z, parent_entity);
            continue;
        };

        // Track that this terrain has chunks being rehydrated
        terrains_with_rehydrated_chunks.insert(parent_entity);

        // Calculate correct position from chunk grid coordinates
        let origin = terrain_data.chunk_world_origin(chunk_data.chunk_x, chunk_data.chunk_z);

        console_info!("Scene", "Rehydrating chunk ({}, {}) at position ({:.1}, {:.1}, {:.1})",
            chunk_data.chunk_x, chunk_data.chunk_z, origin.x, origin.y, origin.z);

        // Create mesh from the saved heightmap data
        let mesh = generate_chunk_mesh(terrain_data, chunk_data);
        let mesh_handle = meshes.add(mesh);

        // Create placeholder material (will be replaced by apply_terrain_material_system)
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.45, 0.73),
            perceptual_roughness: 0.5,
            ..default()
        });

        // Add transform, mesh, material, visibility, and the runtime TerrainChunkOf link
        // Note: This should overwrite any existing Transform from scene load
        // Note: RaytracingMesh3d is NOT added here - it's managed by sync_rendering_settings
        commands.entity(chunk_entity).insert((
            Transform::from_translation(origin),
            Mesh3d(mesh_handle),
            MeshMaterial3d(material),
            Visibility::default(),
            TerrainChunkOf(parent_entity),
            MaterialData {
                material_path: Some(DEFAULT_TERRAIN_MATERIAL.to_string()),
            },
        ));
    }

    // Debug: log rehydrated terrains
    if !terrains_with_rehydrated_chunks.is_empty() {
        console_info!("Scene", "Rehydrated chunks for {} terrain(s)", terrains_with_rehydrated_chunks.len());
    }

    // Second, handle terrains that have NO chunks at all (fallback: create flat terrain)
    for (terrain_entity, terrain_data) in terrain_query.iter() {
        // Skip if we just rehydrated chunks for this terrain (commands are deferred)
        if terrains_with_rehydrated_chunks.contains(&terrain_entity) {
            console_info!("Scene", "Skipping terrain {:?} - already rehydrated chunks", terrain_entity);
            continue;
        }

        // Check if this terrain already has chunks (from previous frames)
        let has_chunks = existing_chunks.iter().any(|chunk_of| chunk_of.0 == terrain_entity);
        if has_chunks {
            continue;
        }

        console_warn!("Scene", "Creating NEW flat terrain chunks for entity {:?} ({}x{} chunks) - no saved chunks found!",
            terrain_entity, terrain_data.chunks_x, terrain_data.chunks_z);

        // Create placeholder material
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.45, 0.73),
            perceptual_roughness: 0.5,
            ..default()
        });

        let initial_height = 0.2;

        // Spawn chunk entities as children
        for cz in 0..terrain_data.chunks_z {
            for cx in 0..terrain_data.chunks_x {
                let mut chunk_data = TerrainChunkData::new(
                    cx,
                    cz,
                    terrain_data.chunk_resolution,
                    initial_height,
                );
                chunk_data.dirty = false;

                let mesh = generate_chunk_mesh(terrain_data, &chunk_data);
                let mesh_handle = meshes.add(mesh);

                let origin = terrain_data.chunk_world_origin(cx, cz);

                // Note: RaytracingMesh3d is NOT added here - it's managed by sync_rendering_settings
                commands.spawn((
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(origin),
                    Visibility::default(),
                    EditorEntity {
                        name: format!("Chunk_{}_{}", cx, cz),
                        tag: String::new(),
                        visible: true,
                        locked: false,
                    },
                    SceneNode,
                    chunk_data,
                    TerrainChunkOf(terrain_entity),
                    ChildOf(terrain_entity),
                    MaterialData {
                        material_path: Some(DEFAULT_TERRAIN_MATERIAL.to_string()),
                    },
                ));
            }
        }

        console_info!("Scene", "Created {} terrain chunks",
            terrain_data.chunks_x * terrain_data.chunks_z);
    }
}

/// Legacy terrain material system — now a no-op.
/// Terrain chunks use MaterialData + apply_material_data for blueprint-based materials.
pub fn apply_terrain_materials() {
    // Material application is now handled by the MaterialData system
}

/// System to add RaytracingMesh3d to any Mesh3d entity that doesn't have it.
/// This catches meshes spawned by GLTF scenes and other sources.
/// Excludes editor gizmo meshes and the cloud dome (custom material incompatible
/// with Solari's StandardMaterial-only raytracing) from the acceleration structure.
#[cfg(feature = "solari")]
pub fn add_raytracing_to_meshes(
    mut commands: Commands,
    query: Query<(Entity, &Mesh3d), (Without<RaytracingMesh3d>, Without<GizmoMesh>, Without<CloudDomeMarker>)>,
) {
    for (entity, mesh3d) in query.iter() {
        commands.entity(entity).insert(RaytracingMesh3d(mesh3d.0.clone()));
    }
}

/// System to ensure all meshes have the required attributes for rendering.
/// Adds UV coordinates and tangents if missing. Tangents are needed for normal mapping.
/// Only processes newly added meshes to avoid per-frame overhead.
pub fn prepare_meshes_for_solari(
    mut meshes: ResMut<Assets<Mesh>>,
    new_meshes: Query<(Entity, &Mesh3d), Added<Mesh3d>>,
) {
    let new_count = new_meshes.iter().count();
    if new_count > 0 {
        console_info!("MeshPrep", "=== PREPARING {} NEW MESHES FOR RENDERING ===", new_count);
    }

    // Only process newly added meshes, not all meshes every frame
    for (entity, mesh3d) in new_meshes.iter() {
        // First check with immutable access if we need to modify anything
        let needs_uvs;
        let needs_tangents;
        let has_normals;
        let vertex_count_info;

        if let Some(mesh) = meshes.get(&mesh3d.0) {
            needs_uvs = !mesh.contains_attribute(Mesh::ATTRIBUTE_UV_0);
            needs_tangents = !mesh.contains_attribute(Mesh::ATTRIBUTE_TANGENT);
            has_normals = mesh.contains_attribute(Mesh::ATTRIBUTE_NORMAL);
            vertex_count_info = mesh.count_vertices();

            console_info!("MeshPrep", "Entity {:?}: vertices={} has_uvs={} has_tangents={} has_normals={}",
                entity, vertex_count_info, !needs_uvs, !needs_tangents, has_normals);
        } else {
            console_warn!("MeshPrep", "Entity {:?}: mesh asset not found!", entity);
            continue;
        }

        // Only get mutable access if we actually need to modify the mesh
        if needs_uvs || (needs_tangents && has_normals) {
            if let Some(mesh) = meshes.get_mut(&mesh3d.0) {
                // Add UV coordinates if missing
                if needs_uvs {
                    if let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                        let vertex_count = positions.len();
                        let uvs: Vec<[f32; 2]> = (0..vertex_count).map(|_| [0.0, 0.0]).collect();
                        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
                        console_info!("MeshPrep", "  Added UV_0 attribute ({} vertices)", vertex_count);
                    }
                }

                // Generate tangents if missing (requires normals and UVs)
                if needs_tangents && has_normals {
                    // Re-check UV after potentially adding them above
                    if mesh.contains_attribute(Mesh::ATTRIBUTE_UV_0) {
                        if let Err(e) = mesh.generate_tangents() {
                            // Tangent generation can fail for some mesh topologies, that's ok
                            console_warn!("MeshPrep", "  Failed to generate tangents: {:?}", e);
                        } else {
                            console_info!("MeshPrep", "  Generated tangents");
                        }
                    }
                }
            }
        }
    }
}

/// System to rebuild Bevy's Children component from ChildOf relationships.
/// This is needed after scene loading because Children isn't saved, only ChildOf is.
/// The hierarchy panel relies on Children to display parent-child relationships.
pub fn rebuild_children_from_child_of(
    mut commands: Commands,
    // Entities with ChildOf that might need their parent's Children updated
    child_of_query: Query<(Entity, &ChildOf), With<EditorEntity>>,
    // Parents that might need Children component
    parent_query: Query<Entity, (With<EditorEntity>, Without<Children>)>,
    // Parents that already have Children (to check if they're missing entries)
    parents_with_children: Query<(Entity, &Children), With<EditorEntity>>,
) {
    use std::collections::HashMap;

    // Build a map of parent -> children from ChildOf relationships
    let mut parent_children_map: HashMap<Entity, Vec<Entity>> = HashMap::new();
    for (child_entity, child_of) in child_of_query.iter() {
        parent_children_map
            .entry(child_of.0)
            .or_default()
            .push(child_entity);
    }

    if parent_children_map.is_empty() {
        return;
    }

    // For parents without Children component, we need to add it
    // But Bevy's Children is managed internally, so we use add_children
    for (parent_entity, children) in parent_children_map.iter() {
        // Check if parent exists and doesn't have Children
        if parent_query.get(*parent_entity).is_ok() {
            // Parent exists but has no Children - add children using Bevy's hierarchy
            for child in children.iter() {
                commands.entity(*parent_entity).add_children(&[*child]);
            }
            console_info!("Scene", "Rebuilt Children for parent {:?} with {} children", parent_entity, children.len());
        } else if let Ok((_, existing_children)) = parents_with_children.get(*parent_entity) {
            // Parent has Children but might be missing some entries
            let existing_set: std::collections::HashSet<_> = existing_children.iter().collect();
            let missing: Vec<_> = children.iter().filter(|c| !existing_set.contains(c)).copied().collect();
            if !missing.is_empty() {
                for child in missing.iter() {
                    commands.entity(*parent_entity).add_children(&[*child]);
                }
                console_info!("Scene", "Added {} missing children to parent {:?}", missing.len(), parent_entity);
            }
        }
    }
}

/// System to rehydrate 3D camera components after scene loading.
/// Adds Camera3d with Msaa::Off to entities that have CameraNodeData but no Camera3d.
/// Scene cameras are inactive by default - the camera preview system handles rendering from them.
pub fn rehydrate_cameras_3d(
    mut commands: Commands,
    query: Query<(Entity, &CameraNodeData), Without<Camera3d>>,
) {
    for (entity, _camera_data) in query.iter() {
        commands.entity(entity).insert((
            Camera3d::default(),
            Msaa::Off,
            Camera {
                is_active: false,
                ..default()
            },
            Name::new("Scene Camera"),
        ));
        console_info!("Scene", "Rehydrated Camera3D for entity {:?}", entity);
    }
}

/// System to rehydrate camera rig components after scene loading.
/// Adds Camera3d with Msaa::Off to entities that have CameraRigData but no Camera3d.
/// Scene cameras are inactive by default - the camera preview system handles rendering from them.
pub fn rehydrate_camera_rigs(
    mut commands: Commands,
    query: Query<(Entity, &CameraRigData), Without<Camera3d>>,
) {
    for (entity, _rig_data) in query.iter() {
        commands.entity(entity).insert((
            Camera3d::default(),
            Msaa::Off,
            Camera {
                is_active: false,
                ..default()
            },
            Name::new("Scene Camera Rig"),
        ));
        console_info!("Scene", "Rehydrated CameraRig for entity {:?}", entity);
    }
}

/// System to rehydrate 2D camera components after scene loading.
/// Adds Camera2d with Msaa::Off to entities that have Camera2DData but no Camera2d.
/// Scene cameras are inactive by default - the camera preview system handles rendering from them.
pub fn rehydrate_cameras_2d(
    mut commands: Commands,
    query: Query<(Entity, &Camera2DData), Without<Camera2d>>,
) {
    for (entity, _camera_data) in query.iter() {
        commands.entity(entity).insert((
            Camera2d,
            Msaa::Off,
            Camera {
                is_active: false,
                ..default()
            },
            Name::new("Scene Camera 2D"),
        ));
        console_info!("Scene", "Rehydrated Camera2D for entity {:?}", entity);
    }
}

/// System to assign NodeIcon to entities loaded from scenes.
/// Uses the ComponentRegistry to find the highest-priority matching component's icon.
pub fn assign_node_icons(world: &mut World) {
    use crate::component_system::ComponentRegistry;

    // Collect entities that need an icon
    let entities_without_icon: Vec<Entity> = world
        .query_filtered::<Entity, (With<EditorEntity>, Without<NodeIcon>)>()
        .iter(world)
        .collect();

    if entities_without_icon.is_empty() {
        return;
    }

    let Some(registry) = world.get_resource::<ComponentRegistry>() else {
        return;
    };

    // For each entity, find the highest-priority component and use its icon
    let mut assignments: Vec<(Entity, String)> = Vec::new();
    for entity in entities_without_icon {
        let present = registry.get_present_on(world, entity);
        // get_present_on returns unsorted; pick highest priority (lowest number)
        if let Some(best) = present.iter().min_by_key(|d| d.priority) {
            assignments.push((entity, best.icon.to_string()));
        }
    }

    for (entity, icon) in assignments {
        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
            entity_mut.insert(NodeIcon(icon));
        }
    }
}
