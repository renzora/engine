//! Mesh sculpting module — deform arbitrary meshes using the terrain sculpt brush.
//!
//! Hooks into the existing `TerrainSculpt` tool mode and reuses `TerrainSettings`
//! for brush configuration. Uses `MeshRayCast` for accurate mesh hit detection.

mod data;
mod gizmo;
mod hover;
mod mesh_update;
mod sculpt;

pub use data::MeshSculptState;

use bevy::prelude::*;

use crate::core::AppState;

/// Plugin for mesh sculpting (vertex deformation on arbitrary meshes).
pub struct MeshSculptPlugin;

impl Plugin for MeshSculptPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MeshSculptState>().add_systems(
            Update,
            (
                hover::mesh_sculpt_hover_system,
                sculpt::mesh_sculpt_system,
                gizmo::mesh_sculpt_gizmo_system,
                mesh_update::mesh_sculpt_update_system,
            )
                .chain()
                .run_if(in_state(AppState::Editor)),
        );
    }
}
