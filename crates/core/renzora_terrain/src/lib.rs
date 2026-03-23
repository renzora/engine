pub mod data;
pub mod material;
pub mod mesh;
pub mod sculpt;
pub mod paint;
pub mod splatmap_material;
pub mod splatmap_systems;
pub mod undo;
pub mod heightmap_import;
pub mod foliage;

use bevy::prelude::*;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] TerrainPlugin");
        app.add_plugins(material::TerrainMaterialPlugin)
            .add_plugins(splatmap_material::TerrainSplatmapMaterialPlugin)
            .register_type::<data::TerrainData>()
            .register_type::<data::TerrainChunkData>()
            .register_type::<paint::PaintableSurfaceData>()
            .register_type::<foliage::TerrainFoliageConfig>()
            .init_resource::<data::TerrainSettings>()
            .init_resource::<data::TerrainToolState>()
            .init_resource::<data::TerrainSculptState>()
            .init_resource::<paint::SurfacePaintSettings>()
            .init_resource::<paint::SurfacePaintState>()
            .init_resource::<undo::TerrainUndoStack>()
            .init_resource::<undo::TerrainStrokeSnapshot>()
            .init_resource::<splatmap_systems::TerrainLayerTextures>()
            .add_systems(
                Update,
                (
                    mesh::rehydrate_terrain_chunks,
                    mesh::terrain_chunk_mesh_update_system,
                    mesh::terrain_data_changed_system,
                    splatmap_systems::splatmap_upload_system,
                    splatmap_systems::terrain_layer_texture_system,
                ),
            );
    }
}
