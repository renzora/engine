//! Terrain undo/redo: snapshot payloads for sculpt and paint strokes.

use bevy::prelude::*;

use crate::landscape::weights::Texel;

/// A single terrain-edit snapshot: chunk heightmaps and the layer weights of
/// the chunks a paint stroke touched. Used as the `before`/`after` payload of
/// the terrain `renzora_undo::SnapshotCmd`.
#[derive(Clone, Default)]
pub struct TerrainUndoEntry {
    /// (chunk_x, chunk_z, base_heights) snapshots
    pub chunk_snapshots: Vec<(u32, u32, Vec<f32>)>,
    /// `(chunk entity, resolution, texels)` for each chunk whose weights the
    /// stroke changed. Empty texels are a chunk that had never been painted.
    pub weight_snapshots: Vec<(Entity, u32, Vec<Texel>)>,
}

/// Resource: holds the "before" snapshot while a stroke is in progress.
#[derive(Resource, Default)]
pub struct TerrainStrokeSnapshot {
    pub active: bool,
    pub chunk_snapshots: Vec<(u32, u32, Vec<f32>)>,
}
