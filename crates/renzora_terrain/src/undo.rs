//! Terrain undo/redo — snapshot-based heightmap and layer-mask undo stack.

use bevy::prelude::*;

/// A single terrain-edit snapshot: chunk heightmaps and paint-layer masks. Used
/// as the `before`/`after` payload of the terrain `renzora_undo::SnapshotCmd`.
#[derive(Clone)]
pub struct TerrainUndoEntry {
    /// (chunk_x, chunk_z, base_heights) snapshots
    pub chunk_snapshots: Vec<(u32, u32, Vec<f32>)>,
    /// Per-entity snapshot of every layer's coverage mask.
    pub layer_mask_snapshots: Vec<(Entity, Vec<Vec<f32>>)>,
}

/// Resource: holds the "before" snapshot while a stroke is in progress.
#[derive(Resource, Default)]
pub struct TerrainStrokeSnapshot {
    pub active: bool,
    pub chunk_snapshots: Vec<(u32, u32, Vec<f32>)>,
    pub layer_mask_snapshots: Vec<(Entity, Vec<Vec<f32>>)>,
}
