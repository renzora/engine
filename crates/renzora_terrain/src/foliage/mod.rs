//! Foliage system — density maps, the instanced grass pipeline, blade scatter,
//! and runtime systems.
//!
//! Merged from the former `renzora_foliage` crate.

pub mod blades;
pub mod data;
pub mod instance;
pub mod render;
pub mod scatter;
#[cfg(test)]
mod shader_test;
pub mod systems;

pub use blades::{scatter_foliage_chunk, MAX_BLADES_PER_CHUNK};
pub use data::{
    FoliageBatch as DensityFoliageBatch, FoliageBrushType, FoliageConfig, FoliageDensityMap,
    FoliagePaintSettings, FoliageType, MAX_FOLIAGE_TYPES,
};
pub use instance::{BladeSetId, GrassChunk, GrassInstance};
pub use scatter::{generate_foliage_instances, FoliageBatch, TerrainFoliageConfig};
pub use systems::FoliageRebuildCost;

use bevy::prelude::*;

#[derive(Default)]
pub struct FoliagePlugin;

impl Plugin for FoliagePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] FoliagePlugin");
        app.add_plugins(render::GrassRenderPlugin)
            .init_resource::<data::FoliageConfig>()
            .init_resource::<systems::FoliageRebuildCost>()
            .register_type::<data::FoliageDensityMap>()
            .register_type::<data::FoliageType>()
            .add_systems(
                Update,
                (
                    // Pinned into the `mesh_stale` hand-off window: composition
                    // sets the flag, the mesh rebuild consumes it — running in
                    // between is the only position guaranteed to observe it.
                    systems::foliage_follow_terrain_system
                        .after(crate::height_layers::compose_height_layers_system)
                        .before(crate::mesh::terrain_chunk_mesh_update_system),
                    systems::foliage_scatter_rebuild_system,
                ),
            );
    }
}

renzora::add!(FoliagePlugin);
