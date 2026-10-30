//! The Paint tool: the landscape weight brush and the panel's commands.
//!
//! The paint-hover system (in `systems.rs`) raycasts the terrain chunks and
//! stores a world-space `hover_position` + `active_entity` (the chunk hit).
//! Here the chunk leads to its terrain, the panel's selected row to a layer of
//! that terrain's landscape material (`TerrainLandscape`), and one dab per
//! frame goes into every chunk the brush reaches, seams included
//! (`renzora_terrain::landscape::weights::apply_weight_dab`).
//!
//! Undo: before the brush first changes a chunk during a stroke it is recorded
//! in `LandscapeStroke`, and `systems::terrain_stroke_end_system` turns those
//! into one undo entry when the button comes up.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use renzora::viewport_types::ViewportState;
use renzora_editor_framework::EditorSelection;
use renzora_terrain::data::{TerrainChunkData, TerrainChunkOf, TerrainData};
use renzora_terrain::landscape::weights::{apply_weight_dab, WeightTarget};
use renzora_terrain::landscape::{LandscapeStroke, TerrainChunkWeights, TerrainLandscape, WeightDab};
use renzora_terrain::paint::{
    PaintBrushType, SurfacePaintCommand, SurfacePaintSettings, SurfacePaintState, PAINT_RADIUS_RANGE,
};

use crate::brush_gizmo;

/// Apply the panel's queued commands. Layers come from the material, so the
/// old Add / Remove Layer commands have nothing to do; a material dropped on
/// the panel becomes the terrain's landscape material.
pub fn landscape_command_system(world: &mut World) {
    let pending: Vec<SurfacePaintCommand> = {
        let Some(mut state) = world.get_resource_mut::<SurfacePaintState>() else { return };
        if state.pending_commands.is_empty() {
            return;
        }
        state.pending_commands.drain(..).collect()
    };
    let Some(target) = paint_target(world) else { return };
    for cmd in pending {
        match cmd {
            SurfacePaintCommand::AssignMaterial { path, .. } => {
                if let Err(e) = renzora_terrain::landscape::assign_material(world, target, &path) {
                    warn!("[terrain] {e}");
                }
            }
            SurfacePaintCommand::ClearMaterial(_) => {
                let _ = renzora_terrain::landscape::assign_material(world, target, "");
            }
            SurfacePaintCommand::AddLayer | SurfacePaintCommand::RemoveLayer(_) => {
                renzora::console_log::console_info(
                    "Terrain",
                    "Layers come from the terrain's material: add a layer to its Layer Blend node in the Material Editor",
                );
            }
        }
    }
}

/// The terrain the Paint panel acts on: the selected one, else the first.
pub(crate) fn paint_target(world: &mut World) -> Option<Entity> {
    let selected = world.get_resource::<EditorSelection>().and_then(|s| s.get());
    if let Some(e) = selected.filter(|e| world.get::<TerrainData>(*e).is_some()) {
        return Some(e);
    }
    let mut q = world.query_filtered::<Entity, With<TerrainData>>();
    q.iter(world).next()
}

/// The brush cursor, and one dab per frame while the button is held.
#[allow(clippy::too_many_arguments)]
pub fn landscape_paint_system(
    paint_state: Res<SurfacePaintState>,
    paint_settings: Res<SurfacePaintSettings>,
    mut stroke: ResMut<LandscapeStroke>,
    chunk_of: Query<&TerrainChunkOf>,
    terrains: Query<(&TerrainData, &GlobalTransform, Option<&TerrainLandscape>)>,
    mut chunks: Query<(Entity, &TerrainChunkData, &TerrainChunkOf, &mut TerrainChunkWeights)>,
    time: Res<Time>,
    mut gizmos: Gizmos,
) {
    let (Some(hover), Some(chunk_entity)) = (paint_state.hover_position, paint_state.active_entity) else {
        return;
    };
    let Ok(of) = chunk_of.get(chunk_entity) else { return };
    let root = of.0;
    let Ok((terrain, transform, landscape)) = terrains.get(root) else { return };

    if paint_state.brush_visible {
        let ground: Vec<&TerrainChunkData> =
            chunks.iter().filter(|(_, _, o, _)| o.0 == root).map(|(_, c, _, _)| c).collect();
        brush_gizmo::draw_brush_cursor(
            &mut gizmos,
            hover,
            paint_settings.brush_radius,
            paint_settings.brush_shape,
            paint_settings.brush_falloff,
            paint_color(paint_settings.brush_type),
            terrain,
            transform.translation(),
            &ground,
        );
    }

    if !paint_state.is_painting {
        return;
    }
    let Some(landscape) = landscape.filter(|l| !l.layers.is_empty()) else { return };
    let Some(layer) = landscape.layers.get(paint_settings.active_layer.min(landscape.layers.len() - 1)) else {
        return;
    };
    stroke.active = true;

    let half = Vec2::new(terrain.total_width(), terrain.total_depth()) / 2.0;
    let center = hover.xz() - transform.translation().xz() + half;
    let dab = WeightDab {
        mode: paint_settings.brush_type,
        center,
        radius: paint_settings.brush_radius.clamp(PAINT_RADIUS_RANGE.0, PAINT_RADIUS_RANGE.1),
        // An airbrush: Strength is how much of the way a held brush goes in a
        // tenth of a second.
        amount: (paint_settings.brush_strength * time.delta_secs() * 10.0).clamp(0.0, 1.0),
        target: paint_settings.target_weight.clamp(0.0, 1.0),
        falloff: paint_settings.brush_falloff,
        falloff_type: paint_settings.falloff_type,
        shape: paint_settings.brush_shape,
        channel: layer.channel,
        base: landscape.base_channel(),
        slope_mask: paint_settings.slope_mask,
        height_mask: paint_settings.height_mask,
        root_y: transform.translation().y,
    };

    // Only the chunks the brush can reach, each recorded for undo the first
    // time this stroke touches it.
    let reach = dab.radius + terrain.chunk_size;
    let resolution = renzora_terrain::landscape::weights::weight_resolution(terrain);
    let mut targets: Vec<WeightTarget> = Vec::new();
    for (entity, chunk, o, weights) in chunks.iter_mut() {
        if o.0 != root {
            continue;
        }
        let chunk_centre =
            Vec2::new((chunk.chunk_x as f32 + 0.5) * terrain.chunk_size, (chunk.chunk_z as f32 + 0.5) * terrain.chunk_size);
        if chunk_centre.distance(center) > reach {
            continue;
        }
        let weights = weights.into_inner();
        // Decoded before it is recorded, or undo would hold "never painted"
        // for a chunk whose paint simply had not been read yet.
        weights.ensure_decoded(resolution);
        stroke.record(entity, weights);
        targets.push(WeightTarget { chunk, weights });
    }
    apply_weight_dab(terrain, &mut targets, &dab);
}

/// One colour per paint brush, so the cursor says which one is in hand: the
/// same job [`crate::systems::brush_color`] does for the sculpt brushes.
fn paint_color(brush_type: PaintBrushType) -> Color {
    match brush_type {
        PaintBrushType::Paint => Color::srgba(0.2, 0.7, 0.9, 0.9),
        PaintBrushType::Erase => Color::srgba(0.9, 0.3, 0.2, 0.9),
        PaintBrushType::Smooth => Color::srgba(0.3, 0.6, 0.9, 0.9),
        PaintBrushType::Fill => Color::srgba(0.9, 0.8, 0.2, 0.9),
    }
}

/// Scroll over the viewport resizes the paint brush, in metres.
pub fn landscape_brush_scroll_system(
    viewport: Res<ViewportState>,
    mut settings: ResMut<SurfacePaintSettings>,
    mut scroll_events: MessageReader<MouseWheel>,
) {
    if !viewport.hovered {
        scroll_events.clear();
        return;
    }
    for ev in scroll_events.read() {
        let factor = if ev.y > 0.0 { 1.1 } else { 0.9 };
        settings.brush_radius = (settings.brush_radius * factor).clamp(PAINT_RADIUS_RANGE.0, PAINT_RADIUS_RANGE.1);
    }
}
