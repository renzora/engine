//! Foliage Editor — painting foliage onto terrain with brush tools.

pub mod panel;
pub mod systems;

use bevy::prelude::*;
use renzora_editor_framework::ActiveTool;
use renzora_terrain::data::{TerrainChunkData, TerrainData};
use renzora_terrain::foliage::{FoliageDensityMap, FoliagePaintSettings};

#[derive(Default)]
pub struct FoliageEditorPlugin;

impl Plugin for FoliageEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] FoliageEditorPlugin");
        // The foliage tools, as a **Foliage** section on the terrain's inspector.
        // They used to be a dock panel *and* a palette on the viewport shelf,
        // driving one set of resources from two places; the shelf copy is gone
        // and the panel became the section. See `panel::FoliagePanel`.
        app.add_plugins(panel::FoliagePanel);
        app.init_resource::<FoliagePaintSettings>()
            .init_resource::<systems::FoliagePaintState>()
            .add_systems(
                Update,
                (
                    ensure_density_maps,
                    systems::foliage_paint_hover_system,
                    systems::foliage_paint_system,
                    systems::foliage_paint_scroll_system,
                    systems::foliage_brush_gizmo_system,
                    systems::foliage_paint_finish_system,
                )
                    .chain()
                    // Ordered ahead of the rescatter so a stroke's texels reach
                    // the blades in the frame they were painted. Unordered, the
                    // two systems land either way round and the preview lags a
                    // whole frame behind the cursor about half the time — which
                    // reads as the brush being spongy rather than as a fixed
                    // delay, and is the more annoying of the two.
                    .before(renzora_terrain::foliage::systems::foliage_scatter_rebuild_system)
                    .run_if(|tool: Option<Res<ActiveTool>>| {
                        tool.is_some_and(|t| *t == ActiveTool::FoliagePaint)
                    }),
            );
    }
}

/// Auto-add FoliageDensityMap to terrain chunks that don't have one yet.
///
/// Sized from the terrain's chunk size, not a constant — the mask has to hold a
/// fixed number of texels per *metre* for the smallest brush to paint a disc
/// rather than a square. Chunks in a scene saved before that keep the mask they
/// were saved with; only new ones get the finer one.
fn ensure_density_maps(
    mut commands: Commands,
    chunks_without: Query<Entity, (With<TerrainChunkData>, Without<FoliageDensityMap>)>,
    terrain: Query<&TerrainData>,
) {
    let Some(chunk_size) = terrain.iter().next().map(|t| t.chunk_size) else {
        return;
    };
    for entity in chunks_without.iter() {
        commands
            .entity(entity)
            .insert(FoliageDensityMap::for_chunk(chunk_size));
    }
}

renzora::add!(FoliageEditorPlugin, Editor);

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_terrain::data::{TerrainChunkOf, TerrainData};
    use renzora_terrain::foliage::{
        DensityFoliageBatch as FoliageBatch, FoliageConfig, FoliageRebuildCost, GrassChunk,
    };
    use std::time::Duration;

    /// A stroke held over a chunk, end to end: the mask is created, the brush
    /// writes it, and the rescatter turns it into blades. Everything after the
    /// hover ray, which needs a window and a camera.
    #[test]
    fn a_held_stroke_scatters_grass() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .init_resource::<FoliagePaintSettings>()
            .init_resource::<systems::FoliagePaintState>()
            .init_resource::<FoliageConfig>()
            .init_resource::<FoliageRebuildCost>()
            .add_systems(
                Update,
                (
                    ensure_density_maps,
                    systems::foliage_paint_system,
                    systems::foliage_paint_finish_system,
                    renzora_terrain::foliage::systems::foliage_scatter_rebuild_system,
                )
                    .chain(),
            );

        let terrain = TerrainData {
            chunks_x: 8,
            chunks_z: 8,
            chunk_size: 64.0,
            chunk_resolution: 129,
            ..default()
        };
        let res = terrain.chunk_resolution;
        let root = app
            .world_mut()
            .spawn((terrain, GlobalTransform::default()))
            .id();
        let chunk = app
            .world_mut()
            .spawn((
                TerrainChunkData::new(3, 3, res, 0.2),
                TerrainChunkOf(root),
                GlobalTransform::from_translation(Vec3::new(-64.0, 0.0, -64.0)),
            ))
            .id();

        // First frame only seeds the mask.
        app.update();
        assert!(app.world().get::<FoliageDensityMap>(chunk).is_some());

        {
            let mut state = app.world_mut().resource_mut::<systems::FoliagePaintState>();
            state.is_painting = true;
            state.active_chunk = Some(chunk);
            state.hover_uv = Some(Vec2::splat(0.5));
        }
        for _ in 0..10 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(16));
            app.update();
        }
        app.world_mut()
            .resource_mut::<systems::FoliagePaintState>()
            .is_painting = false;
        app.update();
        app.update();

        let map = app.world().get::<FoliageDensityMap>(chunk).unwrap();
        let painted = map.density_weights.iter().filter(|w| w[0] > 0.0).count();
        assert!(painted > 0, "the stroke wrote nothing into the mask");

        let blades: usize = app
            .world_mut()
            .query::<(&FoliageBatch, &GrassChunk)>()
            .iter(app.world())
            .filter(|(b, _)| b.chunk_entity == chunk)
            .map(|(_, g)| g.len())
            .sum();
        assert!(blades > 0, "a painted mask scattered no blades");
    }
}
