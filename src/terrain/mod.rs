//! Unreal-style partitioned terrain system
//!
//! The terrain is divided into a grid of chunks, each with its own mesh.
//! This enables efficient LOD, culling, and streaming for large terrains.

mod data;
mod mesh;
mod sculpt;

pub use data::*;
pub use mesh::*;
pub use sculpt::*;

use bevy::prelude::*;

use crate::core::AppState;

/// Plugin for the terrain system
pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<TerrainData>()
            .register_type::<TerrainChunkData>()
            .init_resource::<TerrainSettings>()
            .init_resource::<TerrainSculptState>()
            .add_systems(
                Update,
                (
                    terrain_tool_shortcut_system,
                    terrain_brush_scroll_system,
                    terrain_sculpt_hover_system,
                    terrain_sculpt_system,
                    terrain_brush_cursor_system,
                    terrain_chunk_mesh_update_system,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            );
    }
}
