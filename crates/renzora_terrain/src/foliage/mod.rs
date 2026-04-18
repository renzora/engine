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
pub use scatter::{TerrainFoliageConfig, FoliageBatch, generate_foliage_instances};

use bevy::prelude::*;
use bevy::pbr::MaterialPlugin;

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
                    systems::foliage_follow_terrain_system,
                    systems::foliage_mesh_rebuild_system,
                    systems::foliage_uniform_update_system,
                ),
            );
    }
}
