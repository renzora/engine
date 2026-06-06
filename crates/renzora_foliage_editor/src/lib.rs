//! Foliage Editor — painting foliage onto terrain with brush tools.

mod native;
pub mod systems;

use bevy::prelude::*;
use renzora_editor::ActiveTool;
use renzora_terrain::data::TerrainChunkData;
use renzora_terrain::foliage::{FoliageDensityMap, FoliagePaintSettings};

#[derive(Default)]
pub struct FoliageEditorPlugin;

impl Plugin for FoliageEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] FoliageEditorPlugin");
        // Native (bevy_ui/ember) port of the egui foliage panel; its registered
        // content overrides the egui panel body for id "foliage_painting".
        app.add_plugins(native::NativeFoliage);
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
                        tool.is_some_and(|t| *t == ActiveTool::FoliagePaint)
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

renzora::add!(FoliageEditorPlugin, Editor);
