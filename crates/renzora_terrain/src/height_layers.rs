//! Non-destructive height edit layers for terrain.
//!
//! The sculpt buffer (`TerrainChunkData::base_heights`) is the authoritative
//! user input. Each [`HeightEditLayer`] contributes a sparse per-chunk delta
//! that is composited on top to produce the rendered heights
//! (`TerrainChunkData::heights`). When the user sculpts, base changes and we
//! recompose; when a layer changes (e.g. a spline brush moves), we mark the
//! affected chunks dirty and recompose.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::data::{TerrainChunkData, TerrainData, TerrainChunkOf};
use crate::paint::{MAX_LAYERS, PaintableSurfaceData};

/// A non-destructive delta applied on top of the base sculpt.
///
/// Sparse at the chunk level: only chunks the layer touches store a grid.
/// Within a stored chunk the delta grid is dense (one f32 per texel) because
/// brush footprints are locally dense and it keeps composition index-free.
#[derive(Clone, Debug)]
pub struct HeightEditLayer {
    pub name: String,
    pub enabled: bool,
    /// (chunk_x, chunk_z) -> dense delta grid of length `resolution * resolution`.
    pub chunk_deltas: HashMap<(u32, u32), Vec<f32>>,
    /// Set when any chunk_deltas entry changed so the composition system
    /// knows to mark touched chunks dirty.
    pub dirty: bool,
}

impl HeightEditLayer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            enabled: true,
            chunk_deltas: HashMap::new(),
            dirty: false,
        }
    }

    /// Replace a chunk's delta grid. Marks the layer dirty.
    pub fn set_chunk_delta(&mut self, chunk_x: u32, chunk_z: u32, delta: Vec<f32>) {
        self.chunk_deltas.insert((chunk_x, chunk_z), delta);
        self.dirty = true;
    }

    /// Remove a chunk's delta grid (it becomes a no-op). Marks the layer dirty.
    pub fn clear_chunk_delta(&mut self, chunk_x: u32, chunk_z: u32) {
        if self.chunk_deltas.remove(&(chunk_x, chunk_z)).is_some() {
            self.dirty = true;
        }
    }
}

/// Ordered stack of height edit layers. Composition applies them in order.
#[derive(Resource, Default, Debug)]
pub struct HeightLayerStack {
    pub layers: Vec<HeightEditLayer>,
}

impl HeightLayerStack {
    /// Mark every chunk that any dirty layer touches, then clear the layer
    /// dirty flags. Returns a set of (chunk_x, chunk_z) coords to recompose.
    fn drain_dirty_chunks(&mut self) -> Vec<(u32, u32)> {
        let mut touched: Vec<(u32, u32)> = Vec::new();
        for layer in &mut self.layers {
            if !layer.dirty {
                continue;
            }
            for key in layer.chunk_deltas.keys() {
                if !touched.contains(key) {
                    touched.push(*key);
                }
            }
            layer.dirty = false;
        }
        touched
    }
}

/// Composition system: rebuilds `chunk.heights` from
/// `base_heights + Σ enabled HeightEditLayer deltas + Σ (splat_weight × carve_depth)`
/// for every chunk marked dirty. Also flags chunks touched by any dirty
/// `HeightEditLayer`.
///
/// Runs **after** `derive_splatmap_weights_system` (so the splat weights are
/// up-to-date) and **before** the mesh rebuild system.
pub fn compose_height_layers_system(
    mut layer_stack: ResMut<HeightLayerStack>,
    terrain_query: Query<&TerrainData>,
    mut chunk_query: Query<(&mut TerrainChunkData, Option<&TerrainChunkOf>, Option<&PaintableSurfaceData>)>,
) {
    let layer_dirty_chunks = layer_stack.drain_dirty_chunks();
    if !layer_dirty_chunks.is_empty() {
        for (mut chunk, _, _) in chunk_query.iter_mut() {
            if layer_dirty_chunks.contains(&(chunk.chunk_x, chunk.chunk_z)) {
                chunk.dirty = true;
            }
        }
    }

    for (mut chunk, chunk_of, surface) in chunk_query.iter_mut() {
        if !chunk.dirty {
            continue;
        }
        let chunk = chunk.as_mut();
        let base_len = chunk.base_heights.len();
        if chunk.heights.len() != base_len {
            chunk.heights.resize(base_len, 0.0);
        }
        let (base, composed) = (&chunk.base_heights, &mut chunk.heights);
        composed.copy_from_slice(base);

        let cx = chunk.chunk_x;
        let cz = chunk.chunk_z;
        for layer in &layer_stack.layers {
            if !layer.enabled {
                continue;
            }
            let Some(delta) = layer.chunk_deltas.get(&(cx, cz)) else {
                continue;
            };
            if delta.len() != chunk.heights.len() {
                continue;
            }
            for (i, h) in chunk.heights.iter_mut().enumerate() {
                *h = (*h + delta[i]).clamp(0.0, 1.0);
            }
        }

        // Per-layer carve: sample derived splatmap weights at each heightmap
        // texel and apply Σ(splat_weight × carve_depth). Only visible coverage
        // contributes, so hidden layers don't double-up.
        if let (Some(of), Some(surface)) = (chunk_of, surface) {
            let Ok(terrain) = terrain_query.get(of.0) else {
                continue;
            };
            let splat_res = surface.splatmap_resolution;
            let heights_res = terrain.chunk_resolution;
            if surface.splatmap_weights.len() != (splat_res * splat_res) as usize {
                continue;
            }
            let mut carve = [0.0f32; MAX_LAYERS];
            let mut has_carve = false;
            for (i, layer) in surface.layers.iter().enumerate().take(MAX_LAYERS) {
                if layer.enabled && layer.carve_depth != 0.0 {
                    carve[i] = layer.carve_depth;
                    has_carve = true;
                }
            }
            if !has_carve {
                continue;
            }
            for vz in 0..heights_res {
                for vx in 0..heights_res {
                    let u = vx as f32 / (heights_res - 1).max(1) as f32;
                    let v = vz as f32 / (heights_res - 1).max(1) as f32;
                    let w = sample_splatmap_bilinear(&surface.splatmap_weights, splat_res, u, v);
                    let mut delta = 0.0f32;
                    for i in 0..MAX_LAYERS {
                        delta += w[i] * carve[i];
                    }
                    let idx = (vz * heights_res + vx) as usize;
                    if idx < chunk.heights.len() {
                        chunk.heights[idx] = (chunk.heights[idx] + delta).clamp(0.0, 1.0);
                    }
                }
            }
        }
    }
}

/// Bilinear sample of the derived splatmap at UV `(u, v)` in `[0, 1]`.
fn sample_splatmap_bilinear(weights: &[[f32; MAX_LAYERS]], res: u32, u: f32, v: f32) -> [f32; MAX_LAYERS] {
    let res_f = res as f32;
    let fx = (u * (res_f - 1.0)).clamp(0.0, res_f - 1.0);
    let fy = (v * (res_f - 1.0)).clamp(0.0, res_f - 1.0);
    let x0 = fx.floor() as u32;
    let y0 = fy.floor() as u32;
    let x1 = (x0 + 1).min(res - 1);
    let y1 = (y0 + 1).min(res - 1);
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;

    let idx = |x: u32, y: u32| -> [f32; MAX_LAYERS] {
        weights[(y * res + x) as usize]
    };
    let a = idx(x0, y0);
    let b = idx(x1, y0);
    let c = idx(x0, y1);
    let d = idx(x1, y1);
    let mut out = [0.0f32; MAX_LAYERS];
    for i in 0..MAX_LAYERS {
        let top = a[i] * (1.0 - tx) + b[i] * tx;
        let bot = c[i] * (1.0 - tx) + d[i] * tx;
        out[i] = top * (1.0 - ty) + bot * ty;
    }
    out
}

/// Ensure the composed buffer is populated after scene load.
///
/// Serde skips the composed `heights` field, so freshly-loaded chunks have an
/// empty composed buffer. This system initialises it from `base_heights` and
/// flags the chunk dirty so the composition/mesh systems pick it up.
pub fn ensure_composed_buffer_system(mut chunks: Query<&mut TerrainChunkData, Added<TerrainChunkData>>) {
    for mut chunk in chunks.iter_mut() {
        chunk.ensure_composed_buffer();
    }
}
