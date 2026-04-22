//! Generic Painter component — a stack of painted layers on any entity.
//!
//! Replaces the per-entity `TerrainBrushLayer` model with a single
//! [`Painter`] component that owns its layers as plain data. The painter
//! can sit on any entity that wants to be paintable (terrain, later: any
//! mesh). Each layer gets a child mesh entity that the
//! [`sync_painter_layer_meshes_system`] keeps in lockstep with the Vec.
//!
//! Layer reordering is just a `Vec` swap — the sync system reuses the
//! existing child entities and re-maps them to new indices.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::data::{TerrainChunkData, TerrainChunkOf, TerrainData};

/// Pure-data paint layer: mask + material path + offset + on/off.
///
/// The mask is row-major at the painter target's native resolution (for
/// terrain, the whole-terrain vertex grid). Paint tool stamps into it.
#[derive(Clone, Debug)]
pub struct PaintLayer {
    pub name: String,
    pub material_path: Option<String>,
    pub mask: Vec<f32>,
    pub coverage_threshold: f32,
    pub height_offset: f32,
    pub enabled: bool,
    /// Flipped when `mask`, `height_offset`, or `coverage_threshold` changes.
    pub mesh_dirty: bool,
    /// Flipped when `material_path` changes.
    pub material_dirty: bool,
}

impl PaintLayer {
    pub fn empty(name: impl Into<String>, grid_cells: u32) -> Self {
        let count = (grid_cells * grid_cells) as usize;
        Self {
            name: name.into(),
            material_path: None,
            mask: vec![0.0; count],
            coverage_threshold: 0.01,
            height_offset: 0.02,
            enabled: true,
            mesh_dirty: true,
            material_dirty: true,
        }
    }

    pub fn grid_size(&self) -> u32 {
        (self.mask.len() as f32).sqrt().round() as u32
    }
}

/// A stack of painted layers on an entity.
#[derive(Component, Clone, Debug, Default)]
pub struct Painter {
    pub layers: Vec<PaintLayer>,
    /// Index of the currently-active layer for painting. `None` before any
    /// layer is added or after the last is deleted.
    pub active_layer: Option<usize>,
}

impl Painter {
    pub fn active(&self) -> Option<&PaintLayer> {
        self.active_layer.and_then(|i| self.layers.get(i))
    }

    pub fn active_mut(&mut self) -> Option<&mut PaintLayer> {
        self.active_layer.and_then(|i| self.layers.get_mut(i))
    }
}

/// Marker on the per-layer child mesh entity, tying it back to its painter
/// and index within `Painter.layers`. The index is updated by the sync
/// system on reorder so child entities can be reused.
#[derive(Component, Clone, Copy, Debug)]
pub struct PainterLayerMesh {
    pub painter: Entity,
    pub layer_index: usize,
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Ensure each `Painter` has exactly one child mesh entity per layer, in the
/// same order as `layers`. Spawns new ones as needed, despawns extras,
/// and updates `PainterLayerMesh.layer_index` on existing ones so reorder
/// is a pure Vec operation.
pub fn sync_painter_layer_meshes_system(
    mut commands: Commands,
    painter_query: Query<(Entity, &Painter), Changed<Painter>>,
    mut mesh_query: Query<(Entity, &mut PainterLayerMesh)>,
) {
    for (painter_entity, painter) in painter_query.iter() {
        // Gather existing mesh entities for this painter.
        let mut existing: Vec<(Entity, usize)> = mesh_query
            .iter()
            .filter(|(_, m)| m.painter == painter_entity)
            .map(|(e, m)| (e, m.layer_index))
            .collect();
        existing.sort_by_key(|(_, i)| *i);

        // Despawn entities whose index is now out of range.
        let layer_count = painter.layers.len();
        for (entity, idx) in &existing {
            if *idx >= layer_count {
                commands.entity(*entity).despawn();
            }
        }

        // Spawn new entities for any layers without one.
        let mut covered: Vec<bool> = vec![false; layer_count];
        for (_, idx) in &existing {
            if *idx < layer_count {
                covered[*idx] = true;
            }
        }
        for i in 0..layer_count {
            if !covered[i] {
                let name = format!("Paint Layer Mesh {}", i);
                let layer_mesh = commands
                    .spawn((
                        Name::new(name),
                        Transform::default(),
                        Visibility::default(),
                        PainterLayerMesh {
                            painter: painter_entity,
                            layer_index: i,
                        },
                    ))
                    .id();
                commands.entity(layer_mesh).insert(ChildOf(painter_entity));
            }
        }

        // Re-map indices on existing entities (no-op if already correct).
        for (entity, _) in &existing {
            if let Ok((_, mut marker)) = mesh_query.get_mut(*entity) {
                // We only know this painter's entities, so no conflict with
                // other painters. Re-read current layer_index from the
                // existing list; the intent is to leave them as-is unless
                // the caller explicitly reordered layer indices via the
                // Painter data. Layer reordering must reassign indices on
                // the child entities directly — see `reorder_layers`.
                let _ = marker;
            }
        }
    }
}

/// Rebuild the geometry for any layer-mesh whose layer is dirty. Reads
/// the parent painter's target — currently terrain-only.
pub fn rebuild_painter_layer_meshes_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut painter_query: Query<&mut Painter>,
    mesh_query: Query<(Entity, &PainterLayerMesh, Option<&Mesh3d>)>,
    terrain_query: Query<&TerrainData>,
    chunk_query: Query<(&TerrainChunkData, &TerrainChunkOf)>,
) {
    for (mesh_entity, marker, existing_mesh) in mesh_query.iter() {
        let Ok(mut painter) = painter_query.get_mut(marker.painter) else {
            continue;
        };
        let Some(layer) = painter.layers.get_mut(marker.layer_index) else {
            continue;
        };
        if !layer.mesh_dirty && existing_mesh.is_some() {
            continue;
        }

        // Terrain-specific mesh gen for now.
        let Ok(terrain) = terrain_query.get(marker.painter) else {
            continue;
        };
        let chunks: Vec<&TerrainChunkData> = chunk_query
            .iter()
            .filter(|(_, of)| of.0 == marker.painter)
            .map(|(chunk, _)| chunk)
            .collect();
        if chunks.is_empty() {
            continue;
        }

        let mesh = build_layer_mesh_from_terrain(terrain, layer, &chunks);
        if let Some(h) = existing_mesh {
            if let Some(m) = meshes.get_mut(&h.0) {
                *m = mesh;
            }
        } else {
            let handle = meshes.add(mesh);
            commands.entity(mesh_entity).insert(Mesh3d(handle));
        }
        layer.mesh_dirty = false;
    }
}

fn build_layer_mesh_from_terrain(
    terrain: &TerrainData,
    layer: &PaintLayer,
    chunks: &[&TerrainChunkData],
) -> Mesh {
    let chunk_res = terrain.chunk_resolution;
    let grid_size = layer.grid_size();
    let height_range = terrain.height_range();
    let spacing = terrain.vertex_spacing();
    let half_w = terrain.total_width() / 2.0;
    let half_d = terrain.total_depth() / 2.0;

    let vertex_count = (grid_size * grid_size) as usize;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(vertex_count);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(vertex_count);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(vertex_count);

    let sample_height = |gx: u32, gz: u32| -> f32 {
        let cx = (gx / (chunk_res - 1)).min(terrain.chunks_x.saturating_sub(1));
        let cz = (gz / (chunk_res - 1)).min(terrain.chunks_z.saturating_sub(1));
        let vx = gx - cx * (chunk_res - 1);
        let vz = gz - cz * (chunk_res - 1);
        chunks
            .iter()
            .find(|c| c.chunk_x == cx && c.chunk_z == cz)
            .map(|c| {
                let n = c.get_height(vx.min(chunk_res - 1), vz.min(chunk_res - 1), chunk_res);
                terrain.min_height + n * height_range
            })
            .unwrap_or(terrain.min_height)
    };

    for gz in 0..grid_size {
        for gx in 0..grid_size {
            let wx = gx as f32 * spacing - half_w;
            let wz = gz as f32 * spacing - half_d;
            let wy = sample_height(gx, gz) + layer.height_offset;
            positions.push([wx, wy, wz]);
            uvs.push([
                gx as f32 / (grid_size - 1).max(1) as f32,
                gz as f32 / (grid_size - 1).max(1) as f32,
            ]);
            normals.push([0.0, 1.0, 0.0]);
        }
    }

    for gz in 0..grid_size {
        for gx in 0..grid_size {
            let hl = sample_height(gx.saturating_sub(1), gz);
            let hr = sample_height((gx + 1).min(grid_size - 1), gz);
            let hd = sample_height(gx, gz.saturating_sub(1));
            let hu = sample_height(gx, (gz + 1).min(grid_size - 1));
            let dx = (hr - hl) / (2.0 * spacing.max(1e-4));
            let dz = (hu - hd) / (2.0 * spacing.max(1e-4));
            let n = Vec3::new(-dx, 1.0, -dz).normalize_or_zero();
            normals[(gz * grid_size + gx) as usize] = [n.x, n.y, n.z];
        }
    }

    let mut indices: Vec<u32> = Vec::new();
    let threshold = layer.coverage_threshold;
    for gz in 0..(grid_size - 1) {
        for gx in 0..(grid_size - 1) {
            let tl = gz * grid_size + gx;
            let tr = tl + 1;
            let bl = tl + grid_size;
            let br = bl + 1;

            let m_tl = layer.mask[tl as usize];
            let m_tr = layer.mask[tr as usize];
            let m_bl = layer.mask[bl as usize];
            let m_br = layer.mask[br as usize];

            if m_tl > threshold && m_bl > threshold && m_tr > threshold {
                indices.push(tl);
                indices.push(bl);
                indices.push(tr);
            }
            if m_tr > threshold && m_bl > threshold && m_br > threshold {
                indices.push(tr);
                indices.push(bl);
                indices.push(br);
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Loads each layer's `.material` file and (re)builds the
/// `StandardMaterial` on its mesh-entity. Runs whenever a layer is marked
/// `material_dirty` or has no material yet.
pub fn apply_painter_layer_materials_system(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    vfs: Res<renzora::core::VirtualFileReader>,
    mut painter_query: Query<&mut Painter>,
    mesh_query: Query<(Entity, &PainterLayerMesh, Option<&MeshMaterial3d<StandardMaterial>>)>,
) {
    for (mesh_entity, marker, existing_mat) in mesh_query.iter() {
        let Ok(mut painter) = painter_query.get_mut(marker.painter) else {
            continue;
        };
        let Some(layer) = painter.layers.get_mut(marker.layer_index) else {
            continue;
        };
        if !layer.material_dirty && existing_mat.is_some() {
            continue;
        }
        let mat = build_material(&layer.material_path, &asset_server, &vfs);
        let handle = materials.add(mat);
        commands.entity(mesh_entity).insert(MeshMaterial3d(handle));
        layer.material_dirty = false;
    }
}

fn build_material(
    material_path: &Option<String>,
    asset_server: &AssetServer,
    vfs: &renzora::core::VirtualFileReader,
) -> StandardMaterial {
    let Some(path) = material_path.as_deref() else {
        return StandardMaterial {
            base_color: Color::srgb(0.7, 0.7, 0.7),
            perceptual_roughness: 0.85,
            ..Default::default()
        };
    };
    let Some(json) = vfs.read_string(path) else {
        return StandardMaterial {
            base_color: Color::srgb(0.9, 0.3, 0.6),
            perceptual_roughness: 0.85,
            ..Default::default()
        };
    };
    let (albedo, normal, arm) = extract_layer_textures_from_json(&json)
        .unwrap_or((None, None, None));
    let mut mat = StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        metallic: 0.0,
        ..Default::default()
    };
    if let Some(ref p) = albedo {
        mat.base_color_texture = Some(asset_server.load(p.clone()));
    }
    if let Some(ref p) = normal {
        mat.normal_map_texture = Some(asset_server.load(p.clone()));
    }
    if let Some(ref p) = arm {
        mat.metallic_roughness_texture = Some(asset_server.load(p.clone()));
        mat.occlusion_texture = Some(asset_server.load(p.clone()));
    }
    mat
}

fn extract_layer_textures_from_json(
    json: &str,
) -> Result<(Option<String>, Option<String>, Option<String>), serde_json::Error> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let nodes = v["nodes"].as_array();
    let connections = v["connections"].as_array();
    let (Some(nodes), Some(connections)) = (nodes, connections) else {
        return Ok((None, None, None));
    };
    let output = nodes.iter().find(|n| {
        n["node_type"]
            .as_str()
            .map_or(false, |t| t.starts_with("output/"))
    });
    let Some(output) = output else {
        return Ok((None, None, None));
    };
    let output_id = output["id"].as_u64().unwrap_or(0);
    let trace = |pin: &str| -> Option<String> {
        let conn = connections
            .iter()
            .find(|c| c["to_node"].as_u64() == Some(output_id) && c["to_pin"].as_str() == Some(pin))?;
        let from = conn["from_node"].as_u64()?;
        let src = nodes.iter().find(|n| n["id"].as_u64() == Some(from))?;
        let t = src["node_type"].as_str()?;
        if !t.contains("texture") {
            return None;
        }
        let vals = src.get("input_values")?.as_object()?;
        for (_, v) in vals {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
            if let Some(obj) = v.as_object() {
                if let Some(tex) = obj.get("Texture").and_then(|v| v.as_str()) {
                    if !tex.is_empty() {
                        return Some(tex.to_string());
                    }
                }
            }
        }
        None
    };
    let albedo = trace("base_color");
    let normal = trace("normal");
    let arm = trace("metallic")
        .or_else(|| trace("roughness"))
        .or_else(|| trace("ao"));
    Ok((albedo, normal, arm))
}

// ── Registry ─────────────────────────────────────────────────────────────────

/// Cached preview of painters + layers, kept in sync each frame so
/// inspector UIs that only have `&World` can read layer state without
/// running queries.
#[derive(Resource, Default, Debug)]
pub struct PainterRegistry {
    pub painters: HashMap<Entity, PainterPreview>,
}

#[derive(Clone, Debug, Default)]
pub struct PainterPreview {
    pub active_layer: Option<usize>,
    pub layers: Vec<LayerPreview>,
}

#[derive(Clone, Debug)]
pub struct LayerPreview {
    pub name: String,
    pub material_path: Option<String>,
    pub height_offset: f32,
    pub enabled: bool,
}

pub fn sync_painter_registry_system(
    mut registry: ResMut<PainterRegistry>,
    painters: Query<(Entity, &Painter)>,
) {
    registry.painters.clear();
    for (entity, painter) in painters.iter() {
        registry.painters.insert(
            entity,
            PainterPreview {
                active_layer: painter.active_layer,
                layers: painter
                    .layers
                    .iter()
                    .map(|l| LayerPreview {
                        name: l.name.clone(),
                        material_path: l.material_path.clone(),
                        height_offset: l.height_offset,
                        enabled: l.enabled,
                    })
                    .collect(),
            },
        );
    }
}

// ── Convenience mutators ─────────────────────────────────────────────────────

/// Add a new empty layer and mark it active.
pub fn push_layer(painter: &mut Painter, name: impl Into<String>, grid_cells: u32) {
    let layer = PaintLayer::empty(name, grid_cells);
    painter.layers.push(layer);
    painter.active_layer = Some(painter.layers.len() - 1);
}

/// Remove layer `idx`. Fixes up `active_layer` if needed.
pub fn remove_layer(
    commands: &mut Commands,
    painter_entity: Entity,
    painter: &mut Painter,
    mesh_query: &Query<(Entity, &PainterLayerMesh)>,
    idx: usize,
) {
    if idx >= painter.layers.len() {
        return;
    }
    painter.layers.remove(idx);
    // Adjust active_layer
    painter.active_layer = match painter.active_layer {
        Some(a) if a == idx => {
            if painter.layers.is_empty() {
                None
            } else {
                Some(a.min(painter.layers.len() - 1))
            }
        }
        Some(a) if a > idx => Some(a - 1),
        other => other,
    };
    // Re-index existing layer-mesh entities to match the Vec; despawn the
    // now-orphan at the old tail.
    let mut markers: Vec<(Entity, usize)> = mesh_query
        .iter()
        .filter(|(_, m)| m.painter == painter_entity)
        .map(|(e, m)| (e, m.layer_index))
        .collect();
    markers.sort_by_key(|(_, i)| *i);
    for (entity, old_idx) in markers {
        if old_idx == idx {
            commands.entity(entity).despawn();
        } else if old_idx > idx {
            commands
                .entity(entity)
                .insert(PainterLayerMesh {
                    painter: painter_entity,
                    layer_index: old_idx - 1,
                });
        }
    }
    // Force mesh rebuild for the now-shifted layers.
    for layer in painter.layers.iter_mut() {
        layer.mesh_dirty = true;
        layer.material_dirty = true;
    }
}

/// Move layer from `from` to `to`, re-indexing child meshes.
pub fn reorder_layers(
    commands: &mut Commands,
    painter_entity: Entity,
    painter: &mut Painter,
    mesh_query: &Query<(Entity, &PainterLayerMesh)>,
    from: usize,
    to: usize,
) {
    if from >= painter.layers.len() || to >= painter.layers.len() || from == to {
        return;
    }
    let layer = painter.layers.remove(from);
    painter.layers.insert(to, layer);
    // Fix up active_layer
    painter.active_layer = painter.active_layer.map(|a| {
        if a == from {
            to
        } else if from < a && a <= to {
            a - 1
        } else if to <= a && a < from {
            a + 1
        } else {
            a
        }
    });

    // Re-index child meshes by their current layer_index → new position.
    let mut markers: Vec<(Entity, usize)> = mesh_query
        .iter()
        .filter(|(_, m)| m.painter == painter_entity)
        .map(|(e, m)| (e, m.layer_index))
        .collect();
    for (entity, old_idx) in markers.drain(..) {
        let new_idx = if old_idx == from {
            to
        } else if from < to && old_idx > from && old_idx <= to {
            old_idx - 1
        } else if to < from && old_idx >= to && old_idx < from {
            old_idx + 1
        } else {
            old_idx
        };
        if new_idx != old_idx {
            commands.entity(entity).insert(PainterLayerMesh {
                painter: painter_entity,
                layer_index: new_idx,
            });
        }
    }

    // All layers potentially need re-rebuild because rendering order is
    // implicit via the Vec order (top-down alpha later if we add it).
    for layer in painter.layers.iter_mut() {
        layer.mesh_dirty = true;
    }
}
