//! Foliage painting systems — hover, brush, scroll, gizmo.

use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};

use renzora_terrain::data::{
    compute_brush_falloff, BrushShape, TerrainChunkData, TerrainChunkOf, TerrainData,
};
use renzora_terrain::foliage::{
    FoliageBrushType, FoliageDensityMap, FoliagePaintSettings, FoliageRebuildCost,
    MAX_FOLIAGE_TYPES,
};

// ── Resources ──────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct FoliagePaintState {
    pub is_painting: bool,
    pub hover_position: Option<Vec3>,
    pub hover_uv: Option<Vec2>,
    pub active_chunk: Option<Entity>,
    /// Was painting last frame — used to detect paint-end and trigger rebuild.
    pub was_painting: bool,
    /// Chunks modified during the current paint stroke, rebuilt once more on
    /// release so the final mesh matches the last texels of the stroke exactly.
    pub dirty_chunks: Vec<Entity>,
    /// Seconds since the stroke's last live-preview rebuild.
    pub since_preview: f32,
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
    camera_query: Query<(&Camera, &GlobalTransform), With<renzora_editor_framework::EditorCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    viewport: Option<Res<renzora::core::viewport_types::ViewportState>>,
    chunk_query: Query<(Entity, &TerrainChunkData, &TerrainChunkOf)>,
    terrain_query: Query<(Entity, &TerrainData, &GlobalTransform)>,
) {
    // Only process when viewport is hovered (not over a panel/slider)
    let Some(vp) = viewport.as_ref().filter(|v| v.hovered) else {
        paint_state.hover_position = None;
        paint_state.hover_uv = None;
        paint_state.is_painting = false;
        return;
    };

    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else {
        paint_state.hover_position = None;
        paint_state.is_painting = false;
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };
    // Map the window cursor into the viewport's render-target pixels: subtract
    // the panel's screen offset, then scale for any Half / Quarter resolution.
    if vp.screen_size.x <= 0.0 || vp.screen_size.y <= 0.0 {
        return;
    }
    let render_pos = Vec2::new(
        (cursor.x - vp.screen_position.x) / vp.screen_size.x * vp.current_size.x as f32,
        (cursor.y - vp.screen_position.y) / vp.screen_size.y * vp.current_size.y as f32,
    );
    let Ok(ray) = camera.viewport_to_world(camera_transform, render_pos) else {
        return;
    };

    // Against the terrain's heights rather than its meshes (see
    // `renzora_terrain::raycast`): grass blades and paint-layer overlays float
    // just above the surface and are not part of it, so the brush reaches the
    // ground under them.
    let chunks: Vec<_> = chunk_query.iter().collect();
    let hit = renzora_terrain::raycast::raycast_terrains(ray, terrain_query.iter(), chunks);

    if let Some(hit) = hit {
        paint_state.hover_position = Some(hit.point);
        paint_state.active_chunk = Some(hit.chunk);
        paint_state.hover_uv = Some(hit.chunk_uv.clamp(Vec2::ZERO, Vec2::ONE));
    } else {
        paint_state.hover_position = None;
        paint_state.hover_uv = None;
        paint_state.active_chunk = None;
    }

    paint_state.is_painting =
        mouse.pressed(MouseButton::Left) && paint_state.active_chunk.is_some();
}

/// Apply brush strokes to the density map.
pub fn foliage_paint_system(
    mut paint_state: ResMut<FoliagePaintState>,
    settings: Res<FoliagePaintSettings>,
    time: Res<Time>,
    cost: Res<FoliageRebuildCost>,
    mut density_query: Query<(Entity, &mut FoliageDensityMap, &TerrainChunkData, &TerrainChunkOf)>,
) {
    if !paint_state.is_painting {
        return;
    }
    let Some(entity) = paint_state.active_chunk else {
        return;
    };
    let Some(uv) = paint_state.hover_uv else {
        return;
    };
    let Ok((_, active_map, active_chunk, active_of)) = density_query.get(entity) else {
        return;
    };
    let res = active_map.resolution;
    let (acx, acz, terrain) = (active_chunk.chunk_x as f32, active_chunk.chunk_z as f32, active_of.0);
    let dt = time.delta_secs();

    // Every chunk of this terrain the brush overlaps, not only the one under
    // its centre: neighbours share their edge texels, so the brush centre in
    // a neighbour's texels is the same point shifted by one chunk (`res - 1`
    // texels) per chunk of distance. A stroke along a seam used to stop dead at
    // it, half a brush of grass and half of nothing.
    let radius_texels = (settings.brush_radius * res as f32).max(1.0);
    let preview = {
        paint_state.since_preview += dt;
        // `was_painting` is still last frame's value here, so this is the
        // first frame of a stroke: preview at once rather than an interval
        // later, which on an expensive chunk is half a second of nothing.
        if !paint_state.was_painting {
            paint_state.since_preview = f32::MAX;
        }
        let due = paint_state.since_preview >= cost.live_interval();
        if due {
            paint_state.since_preview = 0.0;
        }
        due
    };
    for (chunk_entity, mut density_map, chunk, of) in &mut density_query {
        if of.0 != terrain || density_map.resolution != res {
            continue;
        }
        let span = (res - 1) as f32;
        let center = Vec2::new(
            (uv.x + acx - chunk.chunk_x as f32) * span,
            (uv.y + acz - chunk.chunk_z as f32) * span,
        );
        if center.x < -radius_texels || center.y < -radius_texels || center.x > span + radius_texels || center.y > span + radius_texels {
            continue;
        }
        if paint_chunk(&mut density_map, &settings, dt, center, radius_texels) {
            if !paint_state.dirty_chunks.contains(&chunk_entity) {
                paint_state.dirty_chunks.push(chunk_entity);
            }
            // Live preview, on a duty cycle paced by what a rebuild costs
            // (`FoliageRebuildCost`); `foliage_paint_finish_system` rebuilds
            // every touched chunk on release, so a stroke never ends on a
            // stale preview.
            if preview {
                density_map.dirty = true;
            }
        }
    }
}

/// One frame of the brush on one chunk's density map, its centre at `center`
/// in that map's texels. Returns whether the brush reached the map.
fn paint_chunk(
    density_map: &mut FoliageDensityMap,
    settings: &FoliagePaintSettings,
    dt: f32,
    center: Vec2,
    radius_texels: f32,
) -> bool {
    let res = density_map.resolution;

    // Paced against `FULL_COVERAGE_WEIGHT` in `mesh_gen`: coverage is now
    // proportional to the painted weight with nothing propping it up from below,
    // so the brush has to actually reach that weight within the handful of frames
    // a moving cursor spends over any one texel. At 4x it reached ~0.03 and a
    // normal stroke laid down a tenth of the grass it asked for.
    let strength = settings.brush_strength * dt * 12.0;
    let type_idx = settings.active_type;
    if type_idx >= MAX_FOLIAGE_TYPES {
        return false;
    }

    // Grow / Trim write the chunk's height channel instead of a type's density
    // weight. The channel is unallocated until the first such stroke, so this is
    // where it gets sized — see `FoliageDensityMap::height_scale`.
    let brush = settings.brush_type;
    let paints_height = brush.paints_height();
    if paints_height {
        density_map.ensure_height_scale();
    }
    let target = brush.target(settings.brush_height);

    let (center_x, center_z) = (center.x, center.y);
    if center_x + radius_texels < 0.0 || center_z + radius_texels < 0.0 {
        return false;
    }

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

            // Every mode is the same stroke — an exponential approach toward the
            // mode's target at the falloff-weighted rate — so only the cell it
            // moves differs. Grow and Trim are one-directional on purpose: a
            // brush that also drags an already-taller texel *back* toward its
            // target would trim while you were trying to grow wherever the two
            // strokes overlapped.
            let cell = if paints_height {
                let Some(cell) = density_map.height_scale.get_mut(idx) else {
                    continue;
                };
                let growing = matches!(brush, FoliageBrushType::Grow);
                if (growing && *cell >= target) || (!growing && *cell <= target) {
                    continue;
                }
                cell
            } else {
                let Some(weights) = density_map.density_weights.get_mut(idx) else {
                    continue;
                };
                &mut weights[type_idx]
            };
            *cell += effect * (target - *cell);
        }
    }
    min_x <= max_x && min_z <= max_z
}

/// Scroll wheel resizes foliage brush (no modifier key needed).
pub fn foliage_paint_scroll_system(
    mut scroll_events: MessageReader<bevy::input::mouse::MouseWheel>,
    mut settings: ResMut<FoliagePaintSettings>,
    viewport: Option<Res<renzora::core::viewport_types::ViewportState>>,
) {
    let vp_hovered = viewport.as_ref().is_some_and(|v| v.hovered);
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
    let Some(pos) = paint_state.hover_position else {
        return;
    };

    // Convert UV radius to world radius
    let world_radius = if let Some(terrain) = terrain_query.iter().next() {
        settings.brush_radius * terrain.chunk_size
    } else {
        settings.brush_radius * 64.0
    };

    let color = match settings.brush_type {
        FoliageBrushType::Paint => Color::srgba(0.3, 0.9, 0.3, 0.8),
        FoliageBrushType::Erase => Color::srgba(0.9, 0.4, 0.2, 0.8),
        // The height brushes are a different channel, not a different amount of
        // the same one, so they get their own hue rather than a shade of green —
        // otherwise Grow over unpainted ground looks like it should be laying
        // down grass and silently does nothing.
        FoliageBrushType::Grow => Color::srgba(0.4, 0.7, 1.0, 0.8),
        FoliageBrushType::Trim => Color::srgba(0.8, 0.6, 1.0, 0.8),
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

/// Hide cursor while painting, show it when stopped. Also triggers the final
/// mesh rebuild when a paint stroke ends (mouse released) — the stroke previews
/// as it's drawn, but the last texels painted since the previous preview tick
/// have not been baked yet, and a chunk left behind mid-stroke is still showing
/// the preview it had when the cursor moved off it.
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
