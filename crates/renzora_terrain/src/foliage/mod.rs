//! Foliage system — density maps, grass material, mesh generation, and runtime systems.
//!
//! Merged from the former `renzora_foliage` crate.

pub mod data;
pub mod material;
pub mod mesh_gen;
pub mod scatter;
pub mod systems;

pub use data::{
    FoliageBatch as DensityFoliageBatch, FoliageBrushType, FoliageConfig, FoliageDensityMap,
    FoliagePaintSettings, FoliageType, MAX_FOLIAGE_TYPES,
};
pub use material::{GrassMaterial, GrassUniforms};
pub use scatter::{generate_foliage_instances, FoliageBatch, TerrainFoliageConfig};

use bevy::pbr::MaterialPlugin;
use bevy::prelude::*;

#[derive(Default)]
pub struct FoliagePlugin;

impl Plugin for FoliagePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] FoliagePlugin");
        bevy::asset::embedded_asset!(app, "grass.wgsl");
        app.add_plugins(MaterialPlugin::<material::GrassMaterial>::default())
            .init_resource::<data::FoliageConfig>()
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
                    systems::foliage_mesh_rebuild_system,
                    systems::foliage_uniform_update_system,
                ),
            );
    }
}

renzora::add!(FoliagePlugin);
