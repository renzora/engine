//! Element-level selection for Edit mode.
//!
//! Separate from the entity-level `EditorSelection` because Edit mode
//! selects pieces *inside* one entity's mesh (vertices, edges, faces),
//! not entities themselves.

use bevy::prelude::*;
use std::collections::HashSet;

use crate::edit_mesh::{EdgeId, FaceId, VertexId};

/// Which kind of mesh element is currently selectable. Blender's 1/2/3 keys.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectMode {
    #[default]
    Vertex,
    Edge,
    Face,
}

/// The currently edited entity plus element selection sets. Phase 1 keeps
/// all three element sets so switching SelectMode doesn't clear selection.
#[derive(Resource, Default, Debug)]
pub struct MeshSelection {
    pub target: Option<Entity>,
    pub mode: SelectMode,
    pub verts: HashSet<VertexId>,
    pub edges: HashSet<EdgeId>,
    pub faces: HashSet<FaceId>,
}

impl MeshSelection {
    pub fn clear(&mut self) {
        self.verts.clear();
        self.edges.clear();
        self.faces.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.verts.is_empty() && self.edges.is_empty() && self.faces.is_empty()
    }
}
