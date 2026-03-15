pub mod data;
pub mod material;
pub mod mesh;
pub mod sculpt;
pub mod paint;

use bevy::prelude::*;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] TerrainPlugin");
        app.add_plugins(material::TerrainMaterialPlugin)
            .register_type::<data::TerrainData>()
            .register_type::<data::TerrainChunkData>()
            .register_type::<paint::PaintableSurfaceData>()
            .init_resource::<data::TerrainSettings>()
            .init_resource::<data::TerrainToolState>()
            .init_resource::<data::TerrainSculptState>()
            .init_resource::<paint::SurfacePaintSettings>()
            .init_resource::<paint::SurfacePaintState>()
            .add_systems(
                Update,
                (
                    mesh::rehydrate_terrain_chunks,
                    mesh::terrain_chunk_mesh_update_system,
                    mesh::terrain_data_changed_system,
                ),
            );
    }
}
