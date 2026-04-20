//! Terrain brush layer — a child entity of a terrain that carries its own
//! mesh + material for painted-region rendering.
//!
//! Replaces the splatmap approach: instead of blending up to 8 materials in
//! a single shader via per-texel weights, each brush layer is a separate
//! mesh entity with a standard [`MeshMaterial3d`] — so dropping any
//! `.material` file on it works out of the box.
//!
//! Data flow:
//! - `TerrainBrushLayer.mask` (per-texel coverage, same resolution as the
//!   chunk heightmap) is authored by the paint tool.
//! - `regenerate_brush_layer_mesh_system` rebuilds the mesh on dirty: it
//!   copies the base terrain surface and filters triangles to only those
//!   where all three corners have `mask > threshold`, offset by
//!   `height_offset` along the surface normal.
//! - `apply_brush_layer_material_system` keeps the `MeshMaterial3d` handle
//!   in sync with `material_path` (loads `.material` files via the asset
//!   server, falls back to `StandardMaterial::default` if unset).

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::data::{TerrainChunkData, TerrainChunkOf, TerrainData};

/// Marker linking a brush layer entity to the terrain it belongs to.
///
/// Separate from `ChildOf` so the layer can live anywhere in the scene
/// hierarchy if needed, but in practice they're spawned as children of the
/// terrain root for hierarchy visibility.
#[derive(Component, Clone, Copy, Debug)]
pub struct TerrainBrushLayerOf(pub Entity);

/// A painted overlay on a terrain, rendered as its own mesh.
///
/// `mask` is row-major at `resolution × resolution`. `height_offset` shifts
/// the layer mesh along world-Y — positive to sit above the surface (for
/// added materials), negative to carve under (for water beds, etc.).
#[derive(Component, Clone, Debug)]
pub struct TerrainBrushLayer {
    /// Display name, shown in the hierarchy + inspector.
    pub name: String,
    /// Asset-relative path to the `.material` this layer renders with.
    /// `None` = plain default material.
    pub material_path: Option<String>,
    /// Coverage mask, one f32 per heightmap vertex. Same resolution as the
    /// terrain's `chunk_resolution` times the chunk grid. Row-major.
    pub mask: Vec<f32>,
    /// Mask coverage threshold below which a vertex is treated as "not
    /// painted" during mesh generation. Small positive so very faint
    /// coverage doesn't spawn flickery stray triangles.
    pub coverage_threshold: f32,
    /// Vertical offset applied to layer-mesh vertices. Positive = above,
    /// negative = below (carve).
    pub height_offset: f32,
    /// On/off without deleting the mask.
    pub enabled: bool,
    /// Set when `mask`, `height_offset`, or `coverage_threshold` changes —
    /// triggers a mesh rebuild.
    pub dirty: bool,
    /// Set when `material_path` changes — triggers a material reload.
    pub material_dirty: bool,
}

impl TerrainBrushLayer {
    /// Allocate a layer sized for a terrain. `vertex_grid_size` is the
    /// terrain's `chunks_x * (chunk_resolution - 1) + 1`-style total vertex
    /// count along each axis.
    pub fn empty(name: impl Into<String>, vertex_grid_size: u32) -> Self {
        Self {
            name: name.into(),
            material_path: None,
            mask: vec![0.0; (vertex_grid_size * vertex_grid_size) as usize],
            coverage_threshold: 0.01,
            height_offset: 0.02,
            enabled: true,
            dirty: true,
            material_dirty: true,
        }
    }

    pub fn grid_size(&self) -> u32 {
        (self.mask.len() as f32).sqrt().round() as u32
    }

    /// Sample the mask at grid coords `(gx, gz)`.
    pub fn mask_at(&self, gx: u32, gz: u32, grid_size: u32) -> f32 {
        let idx = (gz * grid_size + gx) as usize;
        self.mask.get(idx).copied().unwrap_or(0.0)
    }

    pub fn set_mask_at(&mut self, gx: u32, gz: u32, grid_size: u32, value: f32) {
        let idx = (gz * grid_size + gx) as usize;
        if let Some(m) = self.mask.get_mut(idx) {
            *m = value.clamp(0.0, 1.0);
            self.dirty = true;
        }
    }
}

/// Build a mesh for a brush layer by copying the terrain surface and only
/// emitting triangles where all three corners have mask coverage above
/// `threshold`.
///
/// The mesh lives in terrain-local space (matching the terrain root's
/// transform), so the brush-layer entity should spawn at the terrain's
/// transform with identity local transform.
pub fn build_brush_layer_mesh(
    terrain: &TerrainData,
    layer: &TerrainBrushLayer,
    chunks: &[&TerrainChunkData],
) -> Mesh {
    let chunk_res = terrain.chunk_resolution;
    let grid_size = layer.grid_size();
    let height_range = terrain.height_range();
    let spacing = terrain.vertex_spacing();
    let half_w = terrain.total_width() / 2.0;
    let half_d = terrain.total_depth() / 2.0;

    // Build a dense grid of vertices across the whole terrain, sampling
    // heights from whichever chunk owns each cell. Missing chunks sample
    // as the terrain's min-height floor.
    let vertex_count = (grid_size * grid_size) as usize;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(vertex_count);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(vertex_count);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(vertex_count);

    let sample_height = |gx: u32, gz: u32| -> f32 {
        // Each chunk owns a (chunk_res - 1) × (chunk_res - 1) cell-span but
        // shares its border vertices with neighbours. Figure out the chunk
        // and the in-chunk vertex coord.
        let cx = (gx / (chunk_res - 1)).min(terrain.chunks_x.saturating_sub(1));
        let cz = (gz / (chunk_res - 1)).min(terrain.chunks_z.saturating_sub(1));
        let vx = gx - cx * (chunk_res - 1);
        let vz = gz - cz * (chunk_res - 1);
        chunks
            .iter()
            .find(|c| c.chunk_x == cx && c.chunk_z == cz)
            .map(|c| {
                let normalized = c.get_height(vx.min(chunk_res - 1), vz.min(chunk_res - 1), chunk_res);
                terrain.min_height + normalized * height_range
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

    // Central-difference normals using sampled heights.
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

    // Emit triangles where all 3 corners have mask coverage above threshold.
    // This keeps the boundary crisp; soft-edge blending can come later via
    // a per-vertex alpha channel or a shader effect.
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

/// Regenerate the mesh for any brush layer that's dirty. Picks up the
/// layer's parent terrain via `TerrainBrushLayerOf` and gathers its chunks.
pub fn regenerate_brush_layer_mesh_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut layer_query: Query<(Entity, &mut TerrainBrushLayer, &TerrainBrushLayerOf, Option<&Mesh3d>)>,
    terrain_query: Query<&TerrainData>,
    chunk_query: Query<(&TerrainChunkData, &TerrainChunkOf)>,
) {
    for (entity, mut layer, of, mesh_handle) in layer_query.iter_mut() {
        if !layer.dirty && mesh_handle.is_some() {
            continue;
        }
        let Ok(terrain) = terrain_query.get(of.0) else {
            continue;
        };

        let chunks: Vec<&TerrainChunkData> = chunk_query
            .iter()
            .filter(|(_, chunk_of)| chunk_of.0 == of.0)
            .map(|(chunk, _)| chunk)
            .collect();

        if chunks.is_empty() {
            continue;
        }

        let mesh = build_brush_layer_mesh(terrain, &layer, &chunks);
        if let Some(mh) = mesh_handle {
            if let Some(m) = meshes.get_mut(&mh.0) {
                *m = mesh;
            }
        } else {
            let handle = meshes.add(mesh);
            commands.entity(entity).insert(Mesh3d(handle));
        }
        layer.dirty = false;
    }
}

/// System: load the `.material` path (a future enhancement would wire this
/// through the shader graph pipeline; for now we just create a default
/// `StandardMaterial` so the mesh is visible and easy to iterate on).
///
/// TODO: integrate with the shader graph `.material` resolver when the
/// brush-layer model stabilises. For now, dropping a material stores its
/// path but renders with a placeholder tint so you can see the layer.
pub fn apply_brush_layer_material_system(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    vfs: Res<renzora::core::VirtualFileReader>,
    mut layer_query: Query<(Entity, &mut TerrainBrushLayer, Option<&MeshMaterial3d<StandardMaterial>>)>,
) {
    for (entity, mut layer, existing_mat) in layer_query.iter_mut() {
        if !layer.material_dirty && existing_mat.is_some() {
            continue;
        }
        let mat = build_material_for_layer(&layer, &asset_server, &vfs);
        let handle = materials.add(mat);
        commands.entity(entity).insert(MeshMaterial3d(handle));
        layer.material_dirty = false;
    }
}

fn build_material_for_layer(
    layer: &TerrainBrushLayer,
    asset_server: &AssetServer,
    vfs: &renzora::core::VirtualFileReader,
) -> StandardMaterial {
    // No material path yet — placeholder gray.
    let Some(path) = layer.material_path.as_deref() else {
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

    let (albedo, normal, arm) = extract_layer_textures_from_json(&json).unwrap_or((None, None, None));

    let mut mat = StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        metallic: 0.0,
        ..Default::default()
    };
    if let Some(ref path) = albedo {
        mat.base_color_texture = Some(asset_server.load(path.clone()));
    }
    if let Some(ref path) = normal {
        mat.normal_map_texture = Some(asset_server.load(path.clone()));
    }
    if let Some(ref path) = arm {
        mat.metallic_roughness_texture = Some(asset_server.load(path.clone()));
        mat.occlusion_texture = Some(asset_server.load(path.clone()));
    }
    mat
}

/// Parse a `.material` graph JSON and extract the texture paths wired to
/// `base_color`, `normal`, and one of `metallic`/`roughness`/`ao`
/// (packed ARM). Mirrors the splatmap-era parser but scoped to this module.
fn extract_layer_textures_from_json(
    json: &str,
) -> Result<(Option<String>, Option<String>, Option<String>), serde_json::Error> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let nodes = v["nodes"].as_array();
    let connections = v["connections"].as_array();

    let (Some(nodes), Some(connections)) = (nodes, connections) else {
        return Ok((None, None, None));
    };

    let output_node = nodes.iter().find(|n| {
        n["node_type"]
            .as_str()
            .map_or(false, |t| t.starts_with("output/"))
    });
    let Some(output_node) = output_node else {
        return Ok((None, None, None));
    };
    let output_id = output_node["id"].as_u64().unwrap_or(0);

    let trace_texture = |pin_name: &str| -> Option<String> {
        let conn = connections.iter().find(|c| {
            c["to_node"].as_u64() == Some(output_id) && c["to_pin"].as_str() == Some(pin_name)
        })?;
        let from_node_id = conn["from_node"].as_u64()?;
        let source = nodes.iter().find(|n| n["id"].as_u64() == Some(from_node_id))?;
        let node_type = source["node_type"].as_str()?;
        if !node_type.contains("texture") {
            return None;
        }
        let input_vals = source.get("input_values")?.as_object()?;
        for (_key, val) in input_vals {
            if let Some(s) = val.as_str() {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
            if let Some(obj) = val.as_object() {
                if let Some(tex) = obj.get("Texture").and_then(|v| v.as_str()) {
                    if !tex.is_empty() {
                        return Some(tex.to_string());
                    }
                }
            }
        }
        None
    };

    let albedo = trace_texture("base_color");
    let normal = trace_texture("normal");
    let arm = trace_texture("metallic")
        .or_else(|| trace_texture("roughness"))
        .or_else(|| trace_texture("ao"));

    Ok((albedo, normal, arm))
}

// ── Registry / preview cache ─────────────────────────────────────────────────

/// Lightweight cache of brush-layer entities per terrain, kept in sync by
/// [`sync_brush_layer_registry_system`] so `&World` inspector UIs can list
/// layers without running a Bevy query (which needs `&mut World`).
#[derive(Resource, Default, Debug)]
pub struct TerrainBrushLayerRegistry {
    pub layers_by_terrain: HashMap<Entity, Vec<BrushLayerPreview>>,
}

#[derive(Clone, Debug)]
pub struct BrushLayerPreview {
    pub entity: Entity,
    pub name: String,
    pub material_path: Option<String>,
    pub height_offset: f32,
    pub enabled: bool,
}

pub fn sync_brush_layer_registry_system(
    mut registry: ResMut<TerrainBrushLayerRegistry>,
    layers: Query<(Entity, &TerrainBrushLayer, &TerrainBrushLayerOf)>,
) {
    registry.layers_by_terrain.clear();
    for (entity, layer, of) in layers.iter() {
        registry
            .layers_by_terrain
            .entry(of.0)
            .or_default()
            .push(BrushLayerPreview {
                entity,
                name: layer.name.clone(),
                material_path: layer.material_path.clone(),
                height_offset: layer.height_offset,
                enabled: layer.enabled,
            });
    }
}
