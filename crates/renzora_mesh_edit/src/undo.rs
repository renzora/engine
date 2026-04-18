//! Undo commands for mesh edit operations.
//!
//! Commands mutate the [`EditMesh`] component directly. They are valid only
//! while the entity is still in Edit mode — if the user exits Edit mode, the
//! `EditMesh` is removed and queued undo/redo for that entity becomes a
//! no-op. That's acceptable: vertex-level edits only make sense while edit
//! mode is active, and the bake-to-`Mesh` path captures the final result.

use bevy::prelude::*;
use renzora_undo::UndoCommand;

use crate::edit_mesh::EditMesh;
use crate::selection::MeshSelection;

/// Wholesale snapshot of an [`EditMesh`] before/after a topology-mutating
/// operation. More memory than per-element deltas but vastly simpler and
/// correct regardless of what the operator did. Used for extrude, delete,
/// inset, loop cut — everything that changes topology.
pub struct EditMeshSnapshotCmd {
    pub entity: Entity,
    pub label: &'static str,
    pub before: EditMesh,
    pub after: EditMesh,
    /// Selection snapshots so undo/redo also restores what was selected.
    pub before_sel: SelectionSnapshot,
    pub after_sel: SelectionSnapshot,
}

#[derive(Clone, Default)]
pub struct SelectionSnapshot {
    pub verts: std::collections::HashSet<crate::edit_mesh::VertexId>,
    pub edges: std::collections::HashSet<crate::edit_mesh::EdgeId>,
    pub faces: std::collections::HashSet<crate::edit_mesh::FaceId>,
}

impl SelectionSnapshot {
    pub fn from_selection(sel: &MeshSelection) -> Self {
        Self {
            verts: sel.verts.clone(),
            edges: sel.edges.clone(),
            faces: sel.faces.clone(),
        }
    }
    pub fn apply_to(&self, sel: &mut MeshSelection) {
        sel.verts = self.verts.clone();
        sel.edges = self.edges.clone();
        sel.faces = self.faces.clone();
    }
}

impl UndoCommand for EditMeshSnapshotCmd {
    fn label(&self) -> &str {
        self.label
    }
    fn execute(&mut self, world: &mut World) {
        let Ok(mut ent) = world.get_entity_mut(self.entity) else { return };
        if let Some(mut edit) = ent.get_mut::<EditMesh>() {
            *edit = self.after.clone();
            edit.dirty = true;
        }
        if let Some(mut sel) = world.get_resource_mut::<MeshSelection>() {
            self.after_sel.apply_to(&mut sel);
        }
    }
    fn undo(&mut self, world: &mut World) {
        let Ok(mut ent) = world.get_entity_mut(self.entity) else { return };
        if let Some(mut edit) = ent.get_mut::<EditMesh>() {
            *edit = self.before.clone();
            edit.dirty = true;
        }
        if let Some(mut sel) = world.get_resource_mut::<MeshSelection>() {
            self.before_sel.apply_to(&mut sel);
        }
    }
}

/// Move a set of vertices. Stores per-vertex (old, new) positions so undo
/// and redo are both one-shot writes.
pub struct VertexMoveCmd {
    pub entity: Entity,
    /// (vertex_index, old_position, new_position)
    pub deltas: Vec<(u32, Vec3, Vec3)>,
}

impl UndoCommand for VertexMoveCmd {
    fn label(&self) -> &str {
        "Move Vertices"
    }

    fn execute(&mut self, world: &mut World) {
        let Ok(mut ent) = world.get_entity_mut(self.entity) else { return };
        let Some(mut edit) = ent.get_mut::<EditMesh>() else { return };
        for (i, _, new) in &self.deltas {
            if let Some(v) = edit.vertices.get_mut(*i as usize) {
                v.position = *new;
            }
        }
        edit.dirty = true;
    }

    fn undo(&mut self, world: &mut World) {
        let Ok(mut ent) = world.get_entity_mut(self.entity) else { return };
        let Some(mut edit) = ent.get_mut::<EditMesh>() else { return };
        for (i, old, _) in &self.deltas {
            if let Some(v) = edit.vertices.get_mut(*i as usize) {
                v.position = *old;
            }
        }
        edit.dirty = true;
    }
}
