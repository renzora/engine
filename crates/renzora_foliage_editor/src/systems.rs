//! Foliage painting systems — hover, brush, scroll, gizmo.

use bevy::prelude::*;
use bevy::picking::mesh_picking::ray_cast::MeshRayCast;
use bevy::window::{CursorOptions, PrimaryWindow};

use renzora_terrain::data::{
    BrushShape, TerrainChunkData, TerrainData, compute_brush_falloff,
};
use renzora_terrain::foliage::{FoliageBrushType, FoliageDensityMap, FoliagePaintSettings, MAX_FOLIAGE_TYPES};

// ── Resources ──────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct FoliagePaintState {
    pub is_painting: bool,
    pub hover_position: Option<Vec3>,
    pub hover_uv: Option<Vec2>,
    pub active_chunk: Option<Entity>,
    /// Was painting last frame — used to detect paint-end and trigger rebuild.
    pub was_painting: bool,
    /// Chunks modified during the current paint stroke (rebuild deferred until release).
    pub dirty_chunks: Vec<Entity>,
}

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct FoliageToolState {
    pub active: bool,
}

// ── Systems ────────────────────────────────────────────────────────────────

/// Raycast against terrain to find brush position.
pub fn foliage_paint_hover_system(
    mouse: Res<ButtonInput<MouseButton>>,
    mut paint_state: ResMut<FoliagePaintState>,
    mut ray_cast: MeshRayCast,
    camera_query: Query<(&Camera, &GlobalTransform), With<renzora_editor_framework::EditorCamera>>,
    windows: Query<&Window>,
    viewport: Option<Res<renzora::core::viewport_types::ViewportState>>,
    chunk_query: Query<(Entity, &GlobalTransform), With<TerrainChunkData>>,
    terrain_query: Query<&TerrainData>,
) {
    // Only process when viewport is hovered (not over a panel/slider)
    let vp_hovered = viewport.as_ref().map_or(false, |v| v.hovered);
    if !vp_hovered {
        paint_state.hover_position = None;
        paint_state.hover_uv = None;
        paint_state.is_painting = false;
        return;
    }

    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else {
        paint_state.hover_position = None;
        paint_state.is_painting = false;
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else { return };
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor) else { return };

    // Find closest terrain chunk intersection
    let hits = ray_cast.cast_ray(ray, &default());
    let mut best: Option<(Entity, f32, Vec3)> = None;
    for (entity, hit) in hits.iter() {
        if chunk_query.contains(*entity) {
            match best {
                None => best = Some((*entity, hit.distance, hit.point)),
                Some((_, d, _)) if hit.distance < d => {
                    best = Some((*entity, hit.distance, hit.point));
                }
                _ => {}
            }
        }
    }

    if let Some((entity, _, point)) = best {
        paint_state.hover_position = Some(point);
        paint_state.active_chunk = Some(entity);

        // Compute UV from world position relative to chunk
        if let Ok((_, chunk_transform)) = chunk_query.get(entity) {
            if let Some(terrain_data) = terrain_query.iter().next() {
                let local = point - chunk_transform.translation();
                let uv_x = (local.x / terrain_data.chunk_size).clamp(0.0, 1.0);
                let uv_z = (local.z / terrain_data.chunk_size).clamp(0.0, 1.0);
                paint_state.hover_uv = Some(Vec2::new(uv_x, uv_z));
            }
        }
    } else {
        paint_state.hover_position = None;
        paint_state.hover_uv = None;
        paint_state.active_chunk = None;
    }

    paint_state.is_painting = mouse.pressed(MouseButton::Left)
        && paint_state.active_chunk.is_some();
}

/// Apply brush strokes to the density map.
pub fn foliage_paint_system(
    mut paint_state: ResMut<FoliagePaintState>,
    settings: Res<FoliagePaintSettings>,
    time: Res<Time>,
    mut density_query: Query<&mut FoliageDensityMap>,
) {
    if !paint_state.is_painting {
        return;
    }
    let Some(entity) = paint_state.active_chunk else { return };
    let Some(uv) = paint_state.hover_uv else { return };
    let Ok(mut density_map) = density_query.get_mut(entity) else { return };

    // Track this chunk for deferred rebuild
    if !paint_state.dirty_chunks.contains(&entity) {
        paint_state.dirty_chunks.push(entity);
    }

    let res = density_map.resolution;
    let dt = time.delta_secs();
    let strength = settings.brush_strength * dt * 4.0;
    let type_idx = settings.active_type;
    if type_idx >= MAX_FOLIAGE_TYPES {
        return;
    }

    // Brush radius is in UV space (0..1), convert to texel space
    let radius_texels = (settings.brush_radius * res as f32).max(1.0);
    let center_x = uv.x * (res - 1) as f32;
    let center_z = uv.y * (res - 1) as f32;

    let min_x = ((center_x - radius_texels).floor() as i32).max(0) as u32;
    let max_x = ((center_x + radius_texels).ceil() as u32).min(res - 1);
    let min_z = ((center_z - radius_texels).floor() as i32).max(0) as u32;
    let max_z = ((center_z + radius_texels).ceil() as u32).min(res - 1);

    for tz in min_z..=max_z {
        for tx in min_x..=max_x {
            let dx = tx as f32 - center_x;
            let dz = tz as f32 - center_z;

            let dist = match settings.brush_shape {
                BrushShape::Circle => (dx * dx + dz * dz).sqrt(),
                BrushShape::Square => dx.abs().max(dz.abs()),
                BrushShape::Diamond => dx.abs() + dz.abs(),
            };

            let t = dist / radius_texels;
            if t >= 1.0 {
                continue;
            }

            let falloff = compute_brush_falloff(t, settings.brush_falloff, settings.falloff_type);
            let effect = (strength * falloff).min(1.0);

            let idx = (tz * res + tx) as usize;
            if idx >= density_map.density_weights.len() {
                continue;
            }

            match settings.brush_type {
                FoliageBrushType::Paint => {
                    let w = &mut density_map.density_weights[idx][type_idx];
                    *w = (*w + effect * (1.0 - *w)).min(1.0);
                }
                FoliageBrushType::Erase => {
                    let w = &mut density_map.density_weights[idx][type_idx];
                    *w = (*w - effect * *w).max(0.0);
                }
            }
        }
    }

    // Don't mark dirty during painting — defer rebuild until mouse released to avoid freezing.
    // Track this chunk for deferred rebuild.
    // (dirty flag will be set by foliage_paint_finish_system)
}

/// Scroll wheel resizes foliage brush (no modifier key needed).
pub fn foliage_paint_scroll_system(
    mut scroll_events: MessageReader<bevy::input::mouse::MouseWheel>,
    mut settings: ResMut<FoliagePaintSettings>,
    viewport: Option<Res<renzora::core::viewport_types::ViewportState>>,
) {
    let vp_hovered = viewport.as_ref().map_or(false, |v| v.hovered);
    if !vp_hovered {
        return;
    }
    for event in scroll_events.read() {
        let factor = if event.y > 0.0 { 1.1 } else { 0.9 };
        settings.brush_radius = (settings.brush_radius * factor).clamp(0.01, 0.5);
    }
}

/// Draw brush circle gizmo at hover position.
pub fn foliage_brush_gizmo_system(
    paint_state: Res<FoliagePaintState>,
    settings: Res<FoliagePaintSettings>,
    terrain_query: Query<&TerrainData>,
    mut gizmos: Gizmos,
) {
    let Some(pos) = paint_state.hover_position else { return };

    // Convert UV radius to world radius
    let world_radius = if let Some(terrain) = terrain_query.iter().next() {
        settings.brush_radius * terrain.chunk_size
    } else {
        settings.brush_radius * 64.0
    };

    let color = match settings.brush_type {
        FoliageBrushType::Paint => Color::srgba(0.3, 0.9, 0.3, 0.8),
        FoliageBrushType::Erase => Color::srgba(0.9, 0.4, 0.2, 0.8),
    };

    // Draw circle on XZ plane at hover position
    let segments = 48;
    let mut prev = Vec3::ZERO;
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let point = Vec3::new(
            pos.x + angle.cos() * world_radius,
            pos.y + 0.05, // slight offset above terrain to avoid z-fighting
            pos.z + angle.sin() * world_radius,
        );
        if i > 0 {
            gizmos.line(prev, point, color);
        }
        prev = point;
    }
}

/// Hide cursor while painting, show it when stopped. Also triggers deferred
/// mesh rebuild when a paint stroke ends (mouse released).
pub fn foliage_paint_finish_system(
    mut paint_state: ResMut<FoliagePaintState>,
    mut density_query: Query<&mut FoliageDensityMap>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let painting_now = paint_state.is_painting;
    let was_painting = paint_state.was_painting;

    // Cursor management
    if painting_now && !was_painting {
        // Just started painting — hide cursor
        if let Ok(mut cursor) = cursor_query.single_mut() {
            cursor.visible = false;
        }
    }
    if !painting_now && was_painting {
        // Just stopped painting — show cursor and trigger rebuild
        if let Ok(mut cursor) = cursor_query.single_mut() {
            cursor.visible = true;
        }

        // Mark all modified chunks as dirty so mesh rebuilds
        for entity in paint_state.dirty_chunks.drain(..) {
            if let Ok(mut density_map) = density_query.get_mut(entity) {
                density_map.dirty = true;
            }
        }
    }

    paint_state.was_painting = painting_now;
}
