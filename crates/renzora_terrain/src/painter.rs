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
use serde::{Deserialize, Serialize};

use crate::data::{TerrainChunkData, TerrainChunkOf, TerrainData};

/// Pure-data paint layer: mask + material path + offset + on/off.
///
/// The mask is row-major at the painter target's native resolution (for
/// terrain, the whole-terrain vertex grid). Paint tool stamps into it.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct PaintLayer {
    pub name: String,
    pub material_path: Option<String>,
    pub mask: Vec<f32>,
    pub coverage_threshold: f32,
    pub height_offset: f32,
    pub enabled: bool,
    /// Flipped when `mask`, `height_offset`, or `coverage_threshold` changes.
    /// Not serialized: a scene-loaded layer has no child mesh yet, and the
    /// rebuild system regenerates any layer-mesh without a `Mesh3d` regardless
    /// of this flag — so `false` after load is correct.
    #[serde(skip)]
    #[reflect(ignore)]
    pub mesh_dirty: bool,
    /// Flipped when `material_path` changes. Same load story as `mesh_dirty`.
    #[serde(skip)]
    #[reflect(ignore)]
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
///
/// Reflect-serialized so painted masks survive scene save/load (the per-layer
/// child mesh entities are marked `HideInHierarchy` and deliberately excluded
/// from the scene — they're derived data, respawned by the sync system when
/// the deserialized `Painter` shows up `Added`).
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
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

/// Paint masks oversample the terrain vertex grid: this many mask cells per
/// vertex step per axis. The brush outline quantizes to mask cells, so at 1×
/// a large brush's edge visibly stair-steps at sculpt-vertex granularity
/// (~0.5 m on a default tile); 2× halves the step and the alpha feather does
/// the rest. Raising this quadruples mask memory and overlay vertex count.
pub const PAINTER_OVERSAMPLE: u32 = 2;

/// Mask resolution for a painter sitting on `terrain`, along the larger
/// axis. Masks are square (`grid_size²` cells) even on non-square terrains
/// so `PaintLayer::grid_size`'s sqrt stays exact; the extra cells past the
/// short axis simply never match a triangle.
pub fn painter_grid_size(terrain: &TerrainData) -> u32 {
    terrain.chunks_x.max(terrain.chunks_z) * (terrain.chunk_resolution - 1) * PAINTER_OVERSAMPLE
        + 1
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Every terrain is paintable: attach an empty `Painter` to any terrain that
/// lacks one. Covers fresh spawns AND scenes saved before painting existed —
/// which is why this is a system rather than only a `spawn_terrain` insert.
pub fn ensure_painter_system(
    mut commands: Commands,
    terrains: Query<Entity, (With<TerrainData>, Without<Painter>)>,
) {
    for entity in terrains.iter() {
        commands.entity(entity).insert(Painter::default());
    }
}

/// Re-fit layer masks when the terrain grid changes (Add Neighbor, resolution
/// edits). Nearest-neighbour resample keeps painted coverage roughly in place;
/// `mesh_dirty` makes the rebuild system pick the change up.
pub fn resize_painter_masks_system(
    mut painters: Query<(&TerrainData, &mut Painter), Changed<TerrainData>>,
) {
    for (terrain, mut painter) in painters.iter_mut() {
        let expected = painter_grid_size(terrain);
        // Deref-read first so an unchanged painter isn't flagged Changed.
        if painter.layers.iter().all(|l| l.grid_size() == expected) {
            continue;
        }
        for layer in painter.layers.iter_mut() {
            let old = layer.grid_size();
            if old == expected {
                continue;
            }
            layer.mask = resample_mask(&layer.mask, old, expected);
            layer.mesh_dirty = true;
        }
    }
}

fn resample_mask(old: &[f32], old_size: u32, new_size: u32) -> Vec<f32> {
    let count = (new_size * new_size) as usize;
    if old_size == 0 || old.is_empty() {
        return vec![0.0; count];
    }
    let mut out = Vec::with_capacity(count);
    let scale = |v: u32| -> u32 {
        ((v as f32 / (new_size - 1).max(1) as f32) * (old_size - 1) as f32).round() as u32
    };
    for gz in 0..new_size {
        let sz = scale(gz);
        for gx in 0..new_size {
            let sx = scale(gx);
            out.push(old[(sz * old_size + sx) as usize]);
        }
    }
    out
}

/// Sculpting moves the surface the overlays sit on; re-flag their meshes so
/// they follow the new heights. Keyed on the chunks' `mesh_stale` hand-off
/// flag and ordered inside the compose→mesh-rebuild window (same contract as
/// `foliage_follow_terrain_system`).
pub fn painter_follow_terrain_system(
    chunk_query: Query<(&TerrainChunkData, &TerrainChunkOf)>,
    mut painter_query: Query<&mut Painter>,
) {
    for (chunk, of) in chunk_query.iter() {
        if !chunk.mesh_stale {
            continue;
        }
        let Ok(mut painter) = painter_query.get_mut(of.0) else {
            continue;
        };
        // Deref-read guard so an already-flagged painter isn't re-flagged
        // (which would mark it Changed every frame of a stroke for nothing).
        if painter.layers.iter().all(|l| l.mesh_dirty) {
            continue;
        }
        for layer in painter.layers.iter_mut() {
            layer.mesh_dirty = true;
        }
    }
}

/// Ensure each `Painter` has exactly one child mesh entity per layer, in the
/// same order as `layers`. Spawns new ones as needed, despawns extras,
/// and updates `PainterLayerMesh.layer_index` on existing ones so reorder
/// is a pure Vec operation.
pub fn sync_painter_layer_meshes_system(
    mut commands: Commands,
    painter_query: Query<(Entity, &Painter), Changed<Painter>>,
    mesh_query: Query<(Entity, &PainterLayerMesh)>,
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
        let layer_visibility = |i: usize| {
            if painter.layers.get(i).is_none_or(|l| l.enabled) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            }
        };
        for (i, is_covered) in covered.iter().enumerate() {
            if !is_covered {
                let name = format!("Paint Layer Mesh {}", i);
                let layer_mesh = commands
                    .spawn((
                        Name::new(name),
                        Transform::default(),
                        layer_visibility(i),
                        // Derived data: the mask lives on the serialized
                        // `Painter`; these meshes must not be saved into the
                        // scene (they'd load back as zombie children with no
                        // marker) nor clutter the hierarchy panel.
                        renzora::core::HideInHierarchy,
                        PainterLayerMesh {
                            painter: painter_entity,
                            layer_index: i,
                        },
                    ))
                    .id();
                commands.entity(layer_mesh).insert(ChildOf(painter_entity));
            }
        }
        // Existing entities keep their indices — reordering reassigns them
        // directly on the child entities (see `reorder_layers`) — but their
        // visibility follows the layer's `enabled` flag.
        for (entity, idx) in &existing {
            if *idx < layer_count {
                commands.entity(*entity).insert(layer_visibility(*idx));
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
        // Read the dirty flag through Deref — taking `&mut` here would flag
        // the Painter changed every frame and keep the `Changed<Painter>`
        // sync system permanently hot.
        let needs_rebuild = painter
            .layers
            .get(marker.layer_index)
            .is_some_and(|l| l.mesh_dirty || existing_mesh.is_none());
        if !needs_rebuild {
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

        let Some(layer) = painter.layers.get_mut(marker.layer_index) else {
            continue;
        };
        let mesh = build_layer_mesh_from_terrain(terrain, layer, &chunks, marker.layer_index);
        if let Some(h) = existing_mesh {
            if let Some(mut m) = meshes.get_mut(&h.0) {
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
    layer_index: usize,
) -> Mesh {
    // Stacked alpha-blended layers at an identical offset z-fight; each layer
    // index rides a hair higher so the newest paint reliably wins.
    let index_lift = layer_index as f32 * 0.01;
    let chunk_res = terrain.chunk_resolution;
    let grid_size = layer.grid_size();
    let height_range = terrain.height_range();
    let spacing = terrain.vertex_spacing();
    let half_w = terrain.total_width() / 2.0;
    let half_d = terrain.total_depth() / 2.0;
    // World size of one mask cell — derived from the layer's own grid so the
    // builder stays correct for any oversample factor (and for old masks the
    // resize system hasn't caught up with yet).
    let max_axis_w = terrain.chunks_x.max(terrain.chunks_z) as f32 * terrain.chunk_size;
    let cell = max_axis_w / (grid_size.saturating_sub(1)).max(1) as f32;

    let vertex_count = (grid_size * grid_size) as usize;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(vertex_count);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(vertex_count);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(vertex_count);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(vertex_count);

    // Coverage feathers out through vertex alpha instead of a hard triangle
    // cutoff — the band above the threshold fades 0→1 so brush edges read as
    // soft falloff, not vertex-grid staircases.
    let threshold = layer.coverage_threshold;
    let feather = 0.25f32;
    let vertex_alpha = |m: f32| ((m - threshold) / feather).clamp(0.0, 1.0);

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
    // Bilinear height at fractional terrain-vertex coordinates — mask cells
    // sit between sculpt vertices when oversampled, so the overlay has to
    // interpolate the surface exactly like the GPU rasterizes the chunk
    // triangles, or it would dip below the terrain between vertices.
    let sample_height_f = |vx_f: f32, vz_f: f32| -> f32 {
        let x0 = vx_f.floor().max(0.0) as u32;
        let z0 = vz_f.floor().max(0.0) as u32;
        let tx = (vx_f - x0 as f32).clamp(0.0, 1.0);
        let tz = (vz_f - z0 as f32).clamp(0.0, 1.0);
        let h00 = sample_height(x0, z0);
        let h10 = sample_height(x0 + 1, z0);
        let h01 = sample_height(x0, z0 + 1);
        let h11 = sample_height(x0 + 1, z0 + 1);
        (h00 * (1.0 - tx) + h10 * tx) * (1.0 - tz) + (h01 * (1.0 - tx) + h11 * tx) * tz
    };
    // Mask cell → fractional terrain-vertex coordinate.
    let vert_per_cell = cell / spacing.max(1e-6);

    for gz in 0..grid_size {
        for gx in 0..grid_size {
            let wx = gx as f32 * cell - half_w;
            let wz = gz as f32 * cell - half_d;
            let vx_f = gx as f32 * vert_per_cell;
            let vz_f = gz as f32 * vert_per_cell;
            let wy = sample_height_f(vx_f, vz_f) + layer.height_offset + index_lift;
            positions.push([wx, wy, wz]);
            uvs.push([
                gx as f32 / (grid_size - 1).max(1) as f32,
                gz as f32 / (grid_size - 1).max(1) as f32,
            ]);
            normals.push([0.0, 1.0, 0.0]);
            let m = layer.mask[(gz * grid_size + gx) as usize];
            colors.push([1.0, 1.0, 1.0, vertex_alpha(m)]);
        }
    }

    for gz in 0..grid_size {
        for gx in 0..grid_size {
            let vx_f = gx as f32 * vert_per_cell;
            let vz_f = gz as f32 * vert_per_cell;
            let hl = sample_height_f(vx_f - vert_per_cell, vz_f);
            let hr = sample_height_f(vx_f + vert_per_cell, vz_f);
            let hd = sample_height_f(vx_f, vz_f - vert_per_cell);
            let hu = sample_height_f(vx_f, vz_f + vert_per_cell);
            let dx = (hr - hl) / (2.0 * cell.max(1e-4));
            let dz = (hu - hd) / (2.0 * cell.max(1e-4));
            let n = Vec3::new(-dx, 1.0, -dz).normalize_or_zero();
            normals[(gz * grid_size + gx) as usize] = [n.x, n.y, n.z];
        }
    }

    let mut indices: Vec<u32> = Vec::new();
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

            // ANY covered corner emits the triangle — the uncovered corners
            // carry alpha 0, so the edge fades instead of stair-stepping.
            if m_tl > threshold || m_bl > threshold || m_tr > threshold {
                indices.push(tl);
                indices.push(bl);
                indices.push(tr);
            }
            if m_tr > threshold || m_bl > threshold || m_br > threshold {
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
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
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
    mesh_query: Query<(
        Entity,
        &PainterLayerMesh,
        Option<&MeshMaterial3d<StandardMaterial>>,
    )>,
) {
    for (mesh_entity, marker, existing_mat) in mesh_query.iter() {
        let Ok(mut painter) = painter_query.get_mut(marker.painter) else {
            continue;
        };
        // Deref-read first — see the matching note in the rebuild system.
        let needs_rebuild = painter
            .layers
            .get(marker.layer_index)
            .is_some_and(|l| l.material_dirty || existing_mat.is_none());
        if !needs_rebuild {
            continue;
        }
        let Some(layer) = painter.layers.get_mut(marker.layer_index) else {
            continue;
        };
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
    // Alpha-blended: the layer mesh carries per-vertex alpha that feathers
    // coverage edges (see `build_layer_mesh_from_terrain`); an opaque
    // material would put the hard staircase right back.
    //
    // Layers start empty — the user drops a `.material` on the layer to give
    // it its real look. Until then, every layer defaults to plain grass green
    // so strokes are visible against the checkerboard.
    let Some(path) = material_path.as_deref() else {
        return StandardMaterial {
            base_color: Color::srgb(0.36, 0.55, 0.30),
            perceptual_roughness: 0.85,
            alpha_mode: AlphaMode::Blend,
            ..Default::default()
        };
    };
    let Some(json) = vfs.read_string(path) else {
        return StandardMaterial {
            base_color: Color::srgb(0.9, 0.3, 0.6),
            perceptual_roughness: 0.85,
            alpha_mode: AlphaMode::Blend,
            ..Default::default()
        };
    };
    let (albedo, normal, arm) =
        extract_layer_textures_from_json(&json).unwrap_or((None, None, None));
    let mut mat = StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        metallic: 0.0,
        alpha_mode: AlphaMode::Blend,
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
            .is_some_and(|t| t.starts_with("output/"))
    });
    let Some(output) = output else {
        return Ok((None, None, None));
    };
    let output_id = output["id"].as_u64().unwrap_or(0);
    let trace = |pin: &str| -> Option<String> {
        let conn = connections.iter().find(|c| {
            c["to_node"].as_u64() == Some(output_id) && c["to_pin"].as_str() == Some(pin)
        })?;
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
            commands.entity(entity).insert(PainterLayerMesh {
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
