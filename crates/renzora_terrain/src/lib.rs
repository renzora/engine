pub mod brush_layer;
pub mod data;
pub mod foliage;
pub mod height_layers;
pub mod heightmap_import;
pub mod material;
pub mod mesh;
pub mod paint;
pub mod painter;
pub mod sculpt;
pub mod splatmap_material;
pub mod splatmap_systems;
pub mod undo;

use bevy::prelude::*;

#[derive(Default)]
pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] TerrainPlugin");
        app.add_plugins(material::TerrainMaterialPlugin)
            .add_plugins(splatmap_material::TerrainSplatmapMaterialPlugin)
            .register_type::<data::TerrainData>()
            .register_type::<data::TerrainChunkData>()
            .register_type::<paint::PaintableSurfaceData>()
            .register_type::<painter::Painter>()
            .register_type::<painter::PaintLayer>()
            .register_type::<foliage::scatter::TerrainFoliageConfig>()
            .init_resource::<data::TerrainSettings>()
            .init_resource::<data::TerrainSculptState>()
            .init_resource::<data::StampBrushData>()
            .init_resource::<paint::SurfacePaintSettings>()
            .init_resource::<paint::SurfacePaintState>()
            .init_resource::<undo::TerrainStrokeSnapshot>()
            .init_resource::<splatmap_systems::TerrainLayerTextures>()
            .init_resource::<height_layers::HeightLayerStack>()
            .init_resource::<brush_layer::TerrainBrushLayerRegistry>()
            .init_resource::<painter::PainterRegistry>()
            .add_systems(
                Update,
                (
                    (mesh::rehydrate_terrain_chunks, mesh::backfill_missing_chunks),
                    height_layers::ensure_composed_buffer_system,
                    paint::mark_new_surfaces_dirty_system,
                    paint::derive_splatmap_weights_system
                        .after(paint::mark_new_surfaces_dirty_system),
                    height_layers::compose_height_layers_system
                        .after(height_layers::ensure_composed_buffer_system)
                        .after(paint::derive_splatmap_weights_system),
                    mesh::terrain_chunk_mesh_update_system
                        .after(height_layers::compose_height_layers_system),
                    mesh::flush_stale_colliders_system
                        .after(mesh::terrain_chunk_mesh_update_system),
                    mesh::terrain_data_changed_system,
                    splatmap_systems::splatmap_upload_system
                        .after(paint::derive_splatmap_weights_system),
                    splatmap_systems::terrain_layer_texture_system,
                    brush_layer::regenerate_brush_layer_mesh_system
                        .after(mesh::terrain_chunk_mesh_update_system),
                    brush_layer::apply_brush_layer_material_system,
                    brush_layer::sync_brush_layer_registry_system,
                    painter::ensure_painter_system,
                    painter::painter_follow_terrain_system
                        .after(height_layers::compose_height_layers_system)
                        .before(mesh::terrain_chunk_mesh_update_system),
                    painter::resize_painter_masks_system,
                    painter::sync_painter_layer_meshes_system,
                    painter::rebuild_painter_layer_meshes_system
                        .after(painter::sync_painter_layer_meshes_system)
                        .after(painter::resize_painter_masks_system)
                        .after(mesh::terrain_chunk_mesh_update_system),
                    painter::apply_painter_layer_materials_system
                        .after(painter::sync_painter_layer_meshes_system),
                    painter::sync_painter_registry_system,
                ),
            );
    }
}

renzora::add!(TerrainPlugin);
