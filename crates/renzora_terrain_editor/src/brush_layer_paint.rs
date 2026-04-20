//! Paint tool → `Painter.layers[active].mask` writer.
//!
//! The paint-hover system (in `systems.rs`) raycasts the terrain chunks
//! and stores a world-space `hover_position` + `active_entity` (the chunk
//! hit). We walk from that chunk up to its terrain root, grab the
//! `Painter` component, and stamp a disc into the active layer's mask.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use renzora::viewport_types::ViewportState;
use renzora_terrain::data::{TerrainChunkOf, TerrainData};
use renzora_terrain::painter::{PaintLayer, Painter};
use renzora_terrain::paint::{PaintBrushType, SurfacePaintSettings, SurfacePaintState};

pub fn brush_layer_paint_system(
    paint_state: Res<SurfacePaintState>,
    paint_settings: Res<SurfacePaintSettings>,
    chunk_query: Query<&TerrainChunkOf>,
    mut painter_query: Query<(&mut Painter, &TerrainData, &GlobalTransform)>,
    time: Res<Time>,
    mut gizmos: Gizmos,
) {
    // Brush cursor ring is drawn whenever we have a hover + the hit chunk's
    // terrain has a Painter.
    if paint_state.brush_visible {
        if let (Some(hover_pos), Some(chunk_entity)) =
            (paint_state.hover_position, paint_state.active_entity)
        {
            if let Ok(of) = chunk_query.get(chunk_entity) {
                if let Ok((_, terrain, _)) = painter_query.get(of.0) {
                    let world_radius = paint_settings.brush_radius * terrain.chunk_size;
                    draw_cursor_ring(&mut gizmos, hover_pos, world_radius, &paint_settings);
                }
            }
        }
    }

    if !paint_state.is_painting {
        return;
    }
    let Some(hover_pos) = paint_state.hover_position else {
        return;
    };
    let Some(chunk_entity) = paint_state.active_entity else {
        return;
    };
    let Ok(of) = chunk_query.get(chunk_entity) else {
        return;
    };
    let Ok((mut painter, terrain, terrain_gt)) = painter_query.get_mut(of.0) else {
        return;
    };
    let Some(active_idx) = painter.active_layer else {
        return;
    };
    let spacing = terrain.vertex_spacing();
    let half_w = terrain.total_width() / 2.0;
    let half_d = terrain.total_depth() / 2.0;
    let world_radius = paint_settings.brush_radius * terrain.chunk_size;
    let strength = (paint_settings.brush_strength * time.delta_secs() * 4.0).clamp(0.0, 1.0);

    let local = hover_pos - terrain_gt.translation();
    let grid_x = (local.x + half_w) / spacing;
    let grid_z = (local.z + half_d) / spacing;
    let grid_radius = (world_radius / spacing).max(0.5);

    let Some(layer) = painter.layers.get_mut(active_idx) else {
        return;
    };
    stamp_into_mask(
        layer,
        grid_x,
        grid_z,
        grid_radius,
        strength,
        paint_settings.brush_type,
    );
}

fn stamp_into_mask(
    layer: &mut PaintLayer,
    grid_x: f32,
    grid_z: f32,
    grid_radius: f32,
    strength: f32,
    brush: PaintBrushType,
) {
    let grid_size = layer.grid_size();
    if grid_size < 2 {
        return;
    }
    let min_gx = (grid_x - grid_radius).floor().max(0.0) as i32;
    let max_gx = (grid_x + grid_radius)
        .ceil()
        .min((grid_size - 1) as f32) as i32;
    let min_gz = (grid_z - grid_radius).floor().max(0.0) as i32;
    let max_gz = (grid_z + grid_radius)
        .ceil()
        .min((grid_size - 1) as f32) as i32;

    for gz in min_gz..=max_gz {
        for gx in min_gx..=max_gx {
            let dx = gx as f32 - grid_x;
            let dz = gz as f32 - grid_z;
            let dist = (dx * dx + dz * dz).sqrt();
            if dist > grid_radius {
                continue;
            }
            let t = (dist / grid_radius).clamp(0.0, 1.0);
            let falloff = (1.0 + (t * std::f32::consts::PI).cos()) * 0.5;
            let contribution = (strength * falloff).clamp(0.0, 1.0);

            let idx = (gz as u32 * grid_size + gx as u32) as usize;
            if idx >= layer.mask.len() {
                continue;
            }
            let current = layer.mask[idx];
            layer.mask[idx] = match brush {
                PaintBrushType::Paint => current.max(contribution),
                PaintBrushType::Erase => (current - contribution).max(0.0),
                PaintBrushType::Fill => 1.0,
                PaintBrushType::Smooth => {
                    let mut sum = 0.0f32;
                    let mut count = 0.0f32;
                    for nz in (gz - 1).max(0)..=(gz + 1).min(grid_size as i32 - 1) {
                        for nx in (gx - 1).max(0)..=(gx + 1).min(grid_size as i32 - 1) {
                            let nidx = (nz as u32 * grid_size + nx as u32) as usize;
                            sum += layer.mask[nidx];
                            count += 1.0;
                        }
                    }
                    let avg = sum / count.max(1.0);
                    current + (avg - current) * contribution
                }
            };
        }
    }
    layer.mesh_dirty = true;
}

fn draw_cursor_ring(
    gizmos: &mut Gizmos,
    center: Vec3,
    world_radius: f32,
    settings: &SurfacePaintSettings,
) {
    let color = match settings.brush_type {
        PaintBrushType::Paint => Color::srgba(0.2, 0.7, 0.9, 0.9),
        PaintBrushType::Erase => Color::srgba(0.9, 0.3, 0.2, 0.9),
        PaintBrushType::Smooth => Color::srgba(0.3, 0.6, 0.9, 0.9),
        PaintBrushType::Fill => Color::srgba(0.9, 0.8, 0.2, 0.9),
    };
    const SEGMENTS: u32 = 48;
    for i in 0..SEGMENTS {
        let a0 = (i as f32 / SEGMENTS as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / SEGMENTS as f32) * std::f32::consts::TAU;
        let p0 = Vec3::new(
            center.x + a0.cos() * world_radius,
            center.y + 0.1,
            center.z + a0.sin() * world_radius,
        );
        let p1 = Vec3::new(
            center.x + a1.cos() * world_radius,
            center.y + 0.1,
            center.z + a1.sin() * world_radius,
        );
        gizmos.line(p0, p1, color);
    }
}

pub fn brush_layer_scroll_system(
    viewport: Res<ViewportState>,
    mut settings: ResMut<SurfacePaintSettings>,
    mut scroll_events: MessageReader<MouseWheel>,
) {
    if !viewport.hovered {
        return;
    }
    for ev in scroll_events.read() {
        let factor = if ev.y > 0.0 { 1.1 } else { 0.9 };
        settings.brush_radius = (settings.brush_radius * factor).clamp(0.01, 0.5);
    }
}
