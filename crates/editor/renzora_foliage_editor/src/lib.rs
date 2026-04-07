//! Foliage Editor — painting foliage onto terrain with brush tools.

mod panel;
pub mod systems;

use bevy::prelude::*;
use renzora::editor::{ActiveTool, AppEditorExt};
use renzora_terrain::foliage::{FoliageDensityMap, FoliagePaintSettings};
use renzora_terrain::data::TerrainChunkData;

#[derive(Default)]
pub struct FoliageEditorPlugin;

impl Plugin for FoliageEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] FoliageEditorPlugin");
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
                    .run_if(|tool: Option<Res<ActiveTool>>| {
                        tool.map_or(false, |t| *t == ActiveTool::FoliagePaint)
                    }),
            );
    }
}

/// Auto-add FoliageDensityMap to terrain chunks that don't have one yet.
fn ensure_density_maps(
    mut commands: Commands,
    chunks_without: Query<Entity, (With<TerrainChunkData>, Without<FoliageDensityMap>)>,
) {
    for entity in chunks_without.iter() {
        commands.entity(entity).insert(FoliageDensityMap::new(64));
    }
}

renzora::add!(FoliageEditorPlugin);
