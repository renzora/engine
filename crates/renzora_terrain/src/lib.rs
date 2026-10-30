pub mod clusters;
pub mod data;
pub mod erosion;
pub mod foliage;
pub mod generate;
pub mod grid;
pub mod height_layers;
pub mod heightmap_import;
pub mod landscape;
pub mod lod;
pub mod material;
pub mod mesh;
pub mod paint;
pub mod painter;
pub mod raycast;
pub mod sculpt;
pub mod shadows;
pub mod starter;
pub mod surface;
pub mod streaming;
pub mod undo;

use bevy::prelude::*;

#[derive(Default)]
pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] TerrainPlugin");
        app.add_plugins(material::TerrainMaterialPlugin)
            .add_plugins(surface::TerrainSurfacePlugin)
            // The terrain's landscape material and painted layer weights.
            .add_plugins(landscape::LandscapePlugin)
            // Coarser chunk meshes at a distance. The rebuild reads the same
            // `mesh_stale` flag as the full mesh rebuild, so it runs first.
            .add_systems(
                Update,
                (
                    lod::rebuild_lod_meshes
                        .after(height_layers::compose_height_layers_system)
                        .before(mesh::terrain_chunk_mesh_update_system),
                    (lod::drop_orphaned_lods, lod::ensure_chunk_lods, lod::sync_lod_materials)
                        .chain()
                        .after(mesh::terrain_chunk_mesh_update_system),
                    // A sun left on Bevy's 150 m cascades reaches across the
                    // terrain instead.
                    shadows::fit_sun_cascades_to_terrain,
                    // Before physics reads the request, so a chunk's collider
                    // is built as a heightfield and never as a trimesh.
                    mesh::sync_chunk_heightfields.before(renzora::physics::PhysicsSet::Prepare),
                ),
            )
            .register_type::<data::TerrainData>()
            .register_type::<clusters::TerrainClusters>()
            .register_type::<data::TerrainChunkData>()
            .register_type::<paint::PaintableSurfaceData>()
            // The old overlay paint, registered so scenes saved with it load;
            // `landscape::migrate` converts it (see `painter`).
            .register_type::<painter::Painter>()
            .register_type::<painter::PaintLayer>()
            .register_type::<foliage::scatter::TerrainFoliageConfig>()
            .init_resource::<data::TerrainSettings>()
            .init_resource::<data::TerrainSculptState>()
            .init_resource::<data::StampBrushData>()
            .init_resource::<generate::TerrainGenSettings>()
            .init_resource::<paint::SurfacePaintSettings>()
            .init_resource::<paint::SurfacePaintState>()
            .init_resource::<undo::TerrainStrokeSnapshot>()
            .init_resource::<height_layers::HeightLayerStack>()
            .init_resource::<streaming::StreamFocus>()
            .add_systems(
                Update,
                (
                    // Residency decisions land before rehydrate so a chunk
                    // streamed back in this frame rebuilds this frame, and a
                    // starter chunk shaped this frame builds this frame.
                    (
                        streaming::stream_terrain_chunks,
                        starter::shape_pending_chunks,
                        mesh::rehydrate_terrain_chunks,
                        mesh::backfill_missing_chunks,
                        starter::report_terrain_progress,
                    )
                        .chain(),
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
                    // After composition, which is what flags a far chunk's
                    // stand-in as stale when its ground is edited.
                    streaming::sync_stream_proxies
                        .after(streaming::stream_terrain_chunks)
                        .after(height_layers::compose_height_layers_system),
                ),
            );
    }
}

renzora::add!(TerrainPlugin);
