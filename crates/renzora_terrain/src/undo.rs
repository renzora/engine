//! Terrain undo/redo — snapshot-based heightmap and splatmap undo stack.

use bevy::prelude::*;

/// A single undo entry capturing chunk heightmaps and splatmap weights before a stroke.
#[derive(Clone)]
pub struct TerrainUndoEntry {
    /// (chunk_x, chunk_z, heights) snapshots
    pub chunk_snapshots: Vec<(u32, u32, Vec<f32>)>,
    /// (entity, splatmap_weights) snapshots
    pub splatmap_snapshots: Vec<(Entity, Vec<[f32; 8]>)>,
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
    pub splatmap_snapshots: Vec<(Entity, Vec<[f32; 8]>)>,
}
