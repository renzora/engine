//! Terrain entity spawning

use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};
use crate::shared::MaterialData;
use crate::terrain::{TerrainData, TerrainChunkData, TerrainChunkOf, generate_chunk_mesh};
use crate::component_system::components::terrain::NeedsTerrainMaterial;
use super::{Category, EntityTemplate};

/// Path to the default checkerboard material blueprint (relative to engine root)
const DEFAULT_MATERIAL_PATH: &str = "assets/materials/checkerboard_default.material_bp";

pub static TEMPLATES: &[EntityTemplate] = &[
    EntityTemplate { name: "Terrain (4x4)", category: Category::Terrain, spawn: spawn_terrain_4x4 },
    EntityTemplate { name: "Terrain (8x8)", category: Category::Terrain, spawn: spawn_terrain_8x8 },
    EntityTemplate { name: "Terrain (16x16)", category: Category::Terrain, spawn: spawn_terrain_16x16 },
];

fn create_terrain_material(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.45, 0.25), // Grass green-brown
        perceptual_roughness: 0.95,
        ..default()
    })
}

/// Spawn a 4x4 chunk terrain (256m x 256m with default settings)
pub fn spawn_terrain_4x4(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_terrain_with_size(commands, meshes, materials, parent, 4, 4)
}

/// Spawn an 8x8 chunk terrain (512m x 512m with default settings)
pub fn spawn_terrain_8x8(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_terrain_with_size(commands, meshes, materials, parent, 8, 8)
}

/// Spawn a 16x16 chunk terrain (1024m x 1024m with default settings)
pub fn spawn_terrain_16x16(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_terrain_with_size(commands, meshes, materials, parent, 16, 16)
}

/// Spawn terrain with specified chunk counts
fn spawn_terrain_with_size(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
    chunks_x: u32,
    chunks_z: u32,
) -> Entity {
    let terrain_data = TerrainData {
        chunks_x,
        chunks_z,
        chunk_size: 64.0,
        chunk_resolution: 33, // 32x32 quads per chunk (lower res for performance)
        max_height: 50.0,
        min_height: -10.0,
    };

    let material = create_terrain_material(materials);
    let initial_height = 0.2; // Start at 20% height (slight elevation)

    // Spawn terrain root entity (Y=-2.0 so terrain surface sits on grid)
    let mut terrain_cmd = commands.spawn((
        Transform::from_xyz(0.0, -2.0, 0.0),
        Visibility::default(),
        EditorEntity {
            name: format!("Terrain_{}x{}", chunks_x, chunks_z),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
        terrain_data.clone(),
    ));

    if let Some(parent_entity) = parent {
        terrain_cmd.insert(ChildOf(parent_entity));
    }

    let terrain_entity = terrain_cmd.id();

    // Spawn chunk entities
    for cz in 0..terrain_data.chunks_z {
        for cx in 0..terrain_data.chunks_x {
            let mut chunk_data = TerrainChunkData::new(
                cx,
                cz,
                terrain_data.chunk_resolution,
                initial_height,
            );
            chunk_data.dirty = false; // We're generating the mesh now

            // Generate initial mesh
            let mesh = generate_chunk_mesh(&terrain_data, &chunk_data);
            let mesh_handle = meshes.add(mesh);

            // Calculate chunk position
            let origin = terrain_data.chunk_world_origin(cx, cz);

            // Note: RaytracingMesh3d is managed by sync_rendering_settings based on Solari state
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
                NeedsTerrainMaterial,
                MaterialData {
                    material_path: Some(DEFAULT_MATERIAL_PATH.to_string()),
                },
            ));
        }
    }

    terrain_entity
}
