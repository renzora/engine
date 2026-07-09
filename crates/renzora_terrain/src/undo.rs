//! Terrain undo/redo — snapshot-based heightmap and paint-layer undo payloads.

use bevy::prelude::*;

use crate::painter::PaintLayer;

/// A single terrain-edit snapshot: chunk heightmaps and the full painter layer
/// stack. Used as the `before`/`after` payload of the terrain
/// `renzora_undo::SnapshotCmd`.
///
/// Layers are cloned whole (not just their masks) so undoing a stroke that
/// *created* a layer removes it, and redo can bring it back with its name and
/// material intact.
#[derive(Clone)]
pub struct TerrainUndoEntry {
    /// (chunk_x, chunk_z, base_heights) snapshots
    pub chunk_snapshots: Vec<(u32, u32, Vec<f32>)>,
    /// Per-painter-entity snapshot of the complete layer stack.
    pub layer_snapshots: Vec<(Entity, Vec<PaintLayer>)>,
}

/// Resource: holds the "before" snapshot while a stroke is in progress.
#[derive(Resource, Default)]
pub struct TerrainStrokeSnapshot {
    pub active: bool,
    pub chunk_snapshots: Vec<(u32, u32, Vec<f32>)>,
    pub layer_snapshots: Vec<(Entity, Vec<PaintLayer>)>,
}
