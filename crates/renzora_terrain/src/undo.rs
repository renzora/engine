//! Terrain undo/redo — snapshot-based heightmap and layer-mask undo stack.

use bevy::prelude::*;

/// A single undo entry capturing chunk heightmaps and paint-layer masks
/// before a stroke.
#[derive(Clone)]
pub struct TerrainUndoEntry {
    /// (chunk_x, chunk_z, base_heights) snapshots
    pub chunk_snapshots: Vec<(u32, u32, Vec<f32>)>,
    /// Per-entity snapshot of every layer's coverage mask.
    pub layer_mask_snapshots: Vec<(Entity, Vec<Vec<f32>>)>,
}

/// Resource: terrain undo/redo stacks.
#[derive(Resource)]
pub struct TerrainUndoStack {
    pub undo: Vec<TerrainUndoEntry>,
    pub redo: Vec<TerrainUndoEntry>,
    pub max_entries: usize,
}

impl Default for TerrainUndoStack {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            max_entries: 50,
        }
    }
}

impl TerrainUndoStack {
    /// Push a new undo entry, clearing the redo stack.
    pub fn push_undo(&mut self, entry: TerrainUndoEntry) {
        self.undo.push(entry);
        self.redo.clear();
        if self.undo.len() > self.max_entries {
            self.undo.remove(0);
        }
    }
}

/// Resource: holds the "before" snapshot while a stroke is in progress.
#[derive(Resource, Default)]
pub struct TerrainStrokeSnapshot {
    pub active: bool,
    pub chunk_snapshots: Vec<(u32, u32, Vec<f32>)>,
    pub layer_mask_snapshots: Vec<(Entity, Vec<Vec<f32>>)>,
}
