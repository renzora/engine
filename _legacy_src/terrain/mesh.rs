//! Terrain mesh generation

use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use std::collections::HashMap;

use crate::core::{EditorEntity, SceneNode};
use crate::component_system::MaterialData;
use super::{TerrainChunkData, TerrainChunkOf, TerrainData};

const DEFAULT_TERRAIN_MATERIAL: &str = "assets/materials/checkerboard_default.material_bp";

/// Generate a terrain mesh for a chunk
pub fn generate_chunk_mesh(
    terrain: &TerrainData,
    chunk: &TerrainChunkData,
) -> Mesh {
    let resolution = terrain.chunk_resolution;
    let spacing = terrain.vertex_spacing();
    let height_range = terrain.max_height - terrain.min_height;

    let vertex_count = (resolution * resolution) as usize;
    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = Vec::with_capacity(vertex_count);
    let mut uvs = Vec::with_capacity(vertex_count);

    // Generate vertices
    for z in 0..resolution {
        for x in 0..resolution {
            let height_normalized = chunk.get_height(x, z, resolution);
            let height = terrain.min_height + height_normalized * height_range;

            let pos = Vec3::new(
                x as f32 * spacing,
                height,
                z as f32 * spacing,
            );
            positions.push(pos);

            // UV coordinates (0-1 across chunk)
            let u = x as f32 / (resolution - 1) as f32;
            let v = z as f32 / (resolution - 1) as f32;
            uvs.push([u, v]);

            // Placeholder normal (will be calculated after)
            normals.push(Vec3::Y);
        }
    }

    // Calculate normals from neighboring heights
    for z in 0..resolution {
        for x in 0..resolution {
            let idx = (z * resolution + x) as usize;

            // Get heights of neighboring vertices (with edge clamping)
            let h_left = if x > 0 {
                chunk.get_height(x - 1, z, resolution)
            } else {
                chunk.get_height(x, z, resolution)
            };
            let h_right = if x < resolution - 1 {
                chunk.get_height(x + 1, z, resolution)
            } else {
                chunk.get_height(x, z, resolution)
            };
            let h_down = if z > 0 {
                chunk.get_height(x, z - 1, resolution)
            } else {
                chunk.get_height(x, z, resolution)
            };
            let h_up = if z < resolution - 1 {
                chunk.get_height(x, z + 1, resolution)
            } else {
                chunk.get_height(x, z, resolution)
            };

            // Convert to world heights for proper normal calculation
            let h_left_world = terrain.min_height + h_left * height_range;
            let h_right_world = terrain.min_height + h_right * height_range;
            let h_down_world = terrain.min_height + h_down * height_range;
            let h_up_world = terrain.min_height + h_up * height_range;

            // Calculate normal using central differences
            let dx = (h_right_world - h_left_world) / (2.0 * spacing);
            let dz = (h_up_world - h_down_world) / (2.0 * spacing);

            let normal = Vec3::new(-dx, 1.0, -dz).normalize();
            normals[idx] = normal;
        }
    }

    // Generate indices (two triangles per quad)
    let quad_count = ((resolution - 1) * (resolution - 1)) as usize;
    let mut indices = Vec::with_capacity(quad_count * 6);

    for z in 0..(resolution - 1) {
        for x in 0..(resolution - 1) {
            let top_left = z * resolution + x;
            let top_right = top_left + 1;
            let bottom_left = top_left + resolution;
            let bottom_right = bottom_left + 1;

            // First triangle (top-left, bottom-left, top-right)
            indices.push(top_left);
            indices.push(bottom_left);
            indices.push(top_right);

            // Second triangle (top-right, bottom-left, bottom-right)
            indices.push(top_right);
            indices.push(bottom_left);
            indices.push(bottom_right);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );

    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        positions.iter().map(|p| [p.x, p.y, p.z]).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        normals.iter().map(|n| [n.x, n.y, n.z]).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}

/// System to update terrain chunk meshes when dirty
pub fn terrain_chunk_mesh_update_system(
    mut meshes: ResMut<Assets<Mesh>>,
    terrain_query: Query<&TerrainData>,
    mut chunk_query: Query<(&mut TerrainChunkData, &TerrainChunkOf, &Mesh3d)>,
) {
    for (mut chunk, chunk_of, mesh_handle) in chunk_query.iter_mut() {
        if !chunk.dirty {
            continue;
        }

        let Ok(terrain) = terrain_query.get(chunk_of.0) else {
            continue;
        };

        // Generate new mesh
        let new_mesh = generate_chunk_mesh(terrain, &chunk);

        // Update the mesh asset
        if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
            *mesh = new_mesh;
        }

        chunk.dirty = false;
    }
}

/// Bilinear resampling of heightmap from one resolution to another.
/// Preserves sculpted detail when resolution changes.
fn resample_heights(old_heights: &[f32], old_res: u32, new_res: u32) -> Vec<f32> {
    let new_size = (new_res * new_res) as usize;
    let mut out = vec![0.2f32; new_size];

    if old_heights.is_empty() || old_res < 2 || new_res < 2 {
        return out;
    }

    for nz in 0..new_res {
        for nx in 0..new_res {
            let fx = nx as f32 / (new_res - 1) as f32 * (old_res - 1) as f32;
            let fz = nz as f32 / (new_res - 1) as f32 * (old_res - 1) as f32;

            let ox0 = (fx.floor() as u32).min(old_res - 1);
            let oz0 = (fz.floor() as u32).min(old_res - 1);
            let ox1 = (ox0 + 1).min(old_res - 1);
            let oz1 = (oz0 + 1).min(old_res - 1);

            let tx = fx.fract();
            let tz = fz.fract();

            let get = |x: u32, z: u32| -> f32 {
                old_heights.get((z * old_res + x) as usize).copied().unwrap_or(0.2)
            };

            let h = get(ox0, oz0) * (1.0 - tx) * (1.0 - tz)
                + get(ox1, oz0) * tx * (1.0 - tz)
                + get(ox0, oz1) * (1.0 - tx) * tz
                + get(ox1, oz1) * tx * tz;

            out[(nz * new_res + nx) as usize] = h;
        }
    }

    out
}

/// System that fully rebuilds terrain chunks whenever TerrainData is modified.
///
/// Handles all inspector changes: min/max height, chunk size, count, and resolution.
/// Heights are bilinearly resampled when resolution changes to preserve sculpting.
pub fn terrain_data_changed_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut std_materials: ResMut<Assets<StandardMaterial>>,
    changed_terrain: Query<(Entity, &TerrainData), Changed<TerrainData>>,
    added: Query<Entity, Added<TerrainData>>,
    chunk_query: Query<(Entity, &TerrainChunkOf, &TerrainChunkData)>,
) {
    for (terrain_entity, terrain_data) in changed_terrain.iter() {
        // Skip the very first insertion — chunks are spawned by add_terrain / spawn functions
        if added.contains(terrain_entity) {
            continue;
        }

        // Collect existing chunks for this terrain
        let existing: Vec<(Entity, u32, u32, Vec<f32>)> = chunk_query
            .iter()
            .filter(|(_, of, _)| of.0 == terrain_entity)
            .map(|(e, _, data)| (e, data.chunk_x, data.chunk_z, data.heights.clone()))
            .collect();

        if existing.is_empty() {
            continue;
        }

        // Detect old resolution from existing chunk heightmap size
        let old_res = {
            let len = existing[0].3.len();
            (len as f32).sqrt().round() as u32
        };

        // Build lookup: (cx, cz) → old heights
        let old_map: HashMap<(u32, u32), Vec<f32>> = existing
            .iter()
            .map(|(_, cx, cz, h)| ((*cx, *cz), h.clone()))
            .collect();

        // Despawn all old chunk entities
        for (chunk_entity, _, _, _) in &existing {
            commands.entity(*chunk_entity).despawn();
        }

        // Create a fresh material for rebuilt chunks (blueprint system overrides on next frame)
        let material = std_materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.7, 0.7),
            perceptual_roughness: 0.9,
            ..default()
        });

        let new_res = terrain_data.chunk_resolution;

        // Spawn new chunks with resampled or fresh heights
        for cz in 0..terrain_data.chunks_z {
            for cx in 0..terrain_data.chunks_x {
                let heights = if let Some(old_h) = old_map.get(&(cx, cz)) {
                    if old_res != new_res {
                        resample_heights(old_h, old_res, new_res)
                    } else {
                        old_h.clone()
                    }
                } else {
                    // Brand-new chunk (terrain grew) — initialize flat
                    vec![0.2f32; (new_res * new_res) as usize]
                };

                let chunk_data = TerrainChunkData {
                    chunk_x: cx,
                    chunk_z: cz,
                    heights,
                    dirty: false, // mesh is generated immediately below
                };

                let mesh = generate_chunk_mesh(terrain_data, &chunk_data);
                let mesh_handle = meshes.add(mesh);
                let origin = terrain_data.chunk_world_origin(cx, cz);

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
    }
}
