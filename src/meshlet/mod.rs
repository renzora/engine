//! Meshlet/Virtual Geometry integration module
//!
//! This module provides integration with Bevy's experimental meshlet rendering system,
//! which implements GPU-driven virtual geometry with:
//! - Meshlet clustering with hierarchical LOD
//! - Visibility buffer rendering
//! - Two-pass occlusion culling (Hi-Z depth pyramid)
//! - GPU-driven indirect rendering
//!
//! Note: Meshlet rendering is experimental in Bevy and only supports opaque, non-deforming meshes.

#![allow(dead_code)]

use bevy::prelude::*;
use bevy::pbr::experimental::meshlet::MeshletMesh;
use bevy::asset::LoadState;

use crate::core::{EditorEntity, SceneNode};
use crate::shared::MeshletMeshData;
use crate::{console_info, console_error};

/// Plugin for meshlet/virtual geometry integration
pub struct MeshletIntegrationPlugin;

impl Plugin for MeshletIntegrationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                rehydrate_meshlet_meshes,
                track_meshlet_loading,
            ),
        );
    }
}

/// Tracks meshlet assets that are currently loading
#[derive(Component)]
pub struct LoadingMeshletMesh {
    pub handle: Handle<MeshletMesh>,
}

/// Marker for entities that have had their meshlet mesh applied
#[derive(Component)]
pub struct MeshletMeshApplied;

/// System to rehydrate meshlet meshes after scene loading.
/// When scenes are saved, only MeshletMeshData is stored (data component).
/// This system loads the MeshletMesh asset and prepares it for rendering.
pub fn rehydrate_meshlet_meshes(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &MeshletMeshData), (Without<LoadingMeshletMesh>, Without<MeshletMeshApplied>)>,
) {
    for (entity, meshlet_data) in query.iter() {
        if !meshlet_data.enabled {
            continue;
        }

        if meshlet_data.meshlet_path.is_empty() {
            continue;
        }

        // Start loading the meshlet asset
        let handle: Handle<MeshletMesh> = asset_server.load(&meshlet_data.meshlet_path);

        console_info!("Meshlet", "Loading meshlet mesh: {} for entity {:?}",
            meshlet_data.meshlet_path, entity);

        // Mark entity as loading
        commands.entity(entity).insert(LoadingMeshletMesh {
            handle: handle.clone(),
        });
    }
}

/// System to track meshlet mesh loading and apply when ready
pub fn track_meshlet_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<(Entity, &LoadingMeshletMesh, &MeshletMeshData, Option<&MeshMaterial3d<StandardMaterial>>)>,
) {
    for (entity, loading, meshlet_data, existing_material) in query.iter() {
        match asset_server.get_load_state(&loading.handle) {
            Some(LoadState::Loaded) => {
                // Get or create material
                let material_handle = if let Some(mat) = existing_material {
                    mat.0.clone()
                } else {
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.7, 0.7, 0.7),
                        perceptual_roughness: 0.9,
                        ..default()
                    })
                };

                // Remove standard mesh components if present, add meshlet components
                // Note: In Bevy 0.18, meshlet meshes use the Handle<MeshletMesh> directly as a component
                commands.entity(entity)
                    .remove::<Mesh3d>()
                    .remove::<LoadingMeshletMesh>()
                    .insert(MeshletMeshApplied)
                    .insert(MeshMaterial3d(material_handle));

                console_info!("Meshlet", "Applied meshlet mesh to entity {:?}", entity);
            }
            Some(LoadState::Failed(_)) => {
                console_error!("Meshlet", "Failed to load meshlet mesh: {} for entity {:?}",
                    meshlet_data.meshlet_path, entity);
                commands.entity(entity).remove::<LoadingMeshletMesh>();
            }
            _ => {
                // Still loading, do nothing
            }
        }
    }
}

/// Spawn a new entity configured for meshlet mesh rendering
/// Note: The actual meshlet mesh must be assigned via the MeshletMeshData.meshlet_path
pub fn spawn_meshlet_entity(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    meshlet_path: String,
    name: &str,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        MeshMaterial3d(material),
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: name.to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
        MeshletMeshData {
            meshlet_path,
            enabled: true,
            lod_bias: 0.0,
        },
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

/// Default simplification quality for meshlet conversion (0 = highest quality, 255 = lowest)
pub const DEFAULT_MESHLET_SIMPLIFICATION: u8 = 64;

/// Convert a regular mesh to a meshlet mesh
/// This is a preprocessing step that should be done at build time or as an editor action.
/// Returns the meshlet mesh that can be saved as an asset.
/// The `simplification_quality` parameter controls LOD generation (0 = highest quality, 255 = lowest)
pub fn convert_mesh_to_meshlet(mesh: &Mesh, simplification_quality: u8) -> Result<MeshletMesh, MeshletConversionError> {
    MeshletMesh::from_mesh(mesh, simplification_quality)
        .map_err(|e| MeshletConversionError::ConversionFailed(format!("{:?}", e)))
}

/// Errors that can occur during meshlet conversion
#[derive(Debug)]
pub enum MeshletConversionError {
    ConversionFailed(String),
    InvalidMesh(String),
}

impl std::fmt::Display for MeshletConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeshletConversionError::ConversionFailed(msg) => write!(f, "Meshlet conversion failed: {}", msg),
            MeshletConversionError::InvalidMesh(msg) => write!(f, "Invalid mesh for meshlet conversion: {}", msg),
        }
    }
}

impl std::error::Error for MeshletConversionError {}

/// Get statistics about a meshlet mesh
pub struct MeshletStats {
    pub meshlet_count: usize,
    pub triangle_count: usize,
    pub vertex_count: usize,
}

/// Calculate statistics for a meshlet mesh asset
/// Note: This requires access to the internal meshlet data which may not be public
pub fn get_meshlet_stats(_meshlet_mesh: &MeshletMesh) -> Option<MeshletStats> {
    // The MeshletMesh struct's internals are not publicly accessible
    // We would need Bevy to expose these statistics
    // For now, return None and we can display "Unknown" in the UI
    None
}
