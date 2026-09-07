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
