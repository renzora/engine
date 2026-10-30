#![allow(dead_code)] // WIP file — many helpers staged for future panel layouts.

//! Terrain sculpting & painting systems — hover detection, brush application, gizmo rendering.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::ViewportState;
use renzora::core::EditorCamera;
use renzora_terrain::data::*;
use renzora_editor_framework::EditorSelection;
use renzora_terrain::landscape::{LandscapeStroke, TerrainChunkWeights, TerrainLandscape};
use renzora_terrain::paint::{self, SurfacePaintState};
use renzora_terrain::raycast::TerrainRayHit;
use renzora_terrain::sculpt;
use renzora_terrain::undo::TerrainUndoEntry;

use crate::brush_gizmo;

// ── Viewport ray ─────────────────────────────────────────────────────────────

/// The world-space ray under the cursor, or `None` if the cursor isn't over the
/// viewport (or there's nothing to cast from).
///
/// Every terrain tool needs this and they all need it identically, so it lives
/// here rather than being copied per tool. The one subtle step is the last:
/// the cursor arrives in *window* pixels, but the viewport is a panel that may
/// be rendering at Half or Quarter resolution, so the position has to be
/// rescaled into the render target's own pixels before `viewport_to_world`. Get
/// that wrong and the brush drifts further from the cursor the further you are
/// from the panel's top-left corner — which is exactly the bug that keeps coming
/// back when someone re-derives this inline.
pub fn viewport_cursor_ray(
    viewport: &ViewportState,
    window_query: &Query<&Window, With<PrimaryWindow>>,
    camera_query: &Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
) -> Option<Ray3d> {
    if !viewport.hovered {
        return None;
    }
    let window = window_query.single().ok()?;
    let cursor_pos = window.cursor_position()?;

    let vp_pos = viewport.screen_position;
    let vp_size = viewport.screen_size;
    if cursor_pos.x < vp_pos.x
        || cursor_pos.y < vp_pos.y
        || cursor_pos.x > vp_pos.x + vp_size.x
        || cursor_pos.y > vp_pos.y + vp_size.y
    {
        return None;
    }
    if vp_size.x <= 0.0 || vp_size.y <= 0.0 {
        return None;
    }

    let (camera, cam_transform) = camera_query.iter().next()?;
    let local_pos = Vec2::new(
        (cursor_pos.x - vp_pos.x) / vp_size.x * viewport.current_size.x as f32,
        (cursor_pos.y - vp_pos.y) / vp_size.y * viewport.current_size.y as f32,
    );
    camera.viewport_to_world(cam_transform, local_pos).ok()
}

// ── Height sampling ──────────────────────────────────────────────────────────

/// Get terrain height at terrain-local coordinates (0..total_width, 0..total_depth).
#[allow(dead_code)]
fn get_terrain_height_at(
    local_x: f32,
    local_z: f32,
    terrain: &TerrainData,
    chunk_query: &Query<(&TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    terrain_entity: Entity,
) -> Option<f32> {
    let cx = (local_x / terrain.chunk_size).floor() as i32;
    let cz = (local_z / terrain.chunk_size).floor() as i32;

    if cx < 0 || cz < 0 || cx >= terrain.chunks_x as i32 || cz >= terrain.chunks_z as i32 {
        return None;
    }
    let cx = cx as u32;
    let cz = cz as u32;

    for (chunk, chunk_of, _) in chunk_query.iter() {
        if chunk_of.0 != terrain_entity || chunk.chunk_x != cx || chunk.chunk_z != cz {
            continue;
        }

        let in_x = local_x - cx as f32 * terrain.chunk_size;
        let in_z = local_z - cz as f32 * terrain.chunk_size;
        let spacing = terrain.vertex_spacing();
        let fx = in_x / spacing;
        let fz = in_z / spacing;

        let vx0 = (fx.floor() as u32).min(terrain.chunk_resolution - 1);
        let vz0 = (fz.floor() as u32).min(terrain.chunk_resolution - 1);
        let vx1 = (vx0 + 1).min(terrain.chunk_resolution - 1);
        let vz1 = (vz0 + 1).min(terrain.chunk_resolution - 1);
        let tx = fx - fx.floor();
        let tz = fz - fz.floor();

        let h00 = chunk.get_height(vx0, vz0, terrain.chunk_resolution);
        let h10 = chunk.get_height(vx1, vz0, terrain.chunk_resolution);
        let h01 = chunk.get_height(vx0, vz1, terrain.chunk_resolution);
        let h11 = chunk.get_height(vx1, vz1, terrain.chunk_resolution);

        let h0 = h00 * (1.0 - tx) + h10 * tx;
        let h1 = h01 * (1.0 - tx) + h11 * tx;
        let height_normalized = h0 * (1.0 - tz) + h1 * tz;

        return Some(terrain.min_height + height_normalized * terrain.height_range());
    }

    None
}

// ── Hover system ─────────────────────────────────────────────────────────────

/// The nearest terrain under `ray`, for the hover systems' queries.
fn raycast_terrains(
    ray: Ray3d,
    terrains: &Query<(Entity, &TerrainData, &GlobalTransform)>,
    chunks: &Query<(Entity, &TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
) -> Option<TerrainRayHit> {
    let chunks: Vec<_> = chunks.iter().map(|(e, chunk, of, _)| (e, chunk, of)).collect();
    renzora_terrain::raycast::raycast_terrains(ray, terrains.iter(), chunks)
}

/// Detect brush position on terrain via mesh raycast (accurate on sculpted surfaces).
#[allow(clippy::too_many_arguments)]
pub fn terrain_sculpt_hover_system(
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    terrain_query: Query<(Entity, &TerrainData, &GlobalTransform)>,
    chunk_query: Query<(Entity, &TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    mut sculpt_state: ResMut<TerrainSculptState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    _settings: Res<TerrainSettings>,
) {
    sculpt_state.brush_visible = false;

    let Some(ray) = viewport_cursor_ray(&viewport, &window_query, &camera_query) else {
        sculpt_state.hover_position = None;
        // Only drop the active terrain when the cursor has actually left the
        // viewport; a momentarily missing ray inside it shouldn't end the stroke.
        if !viewport.hovered {
            sculpt_state.active_terrain = None;
        }
        return;
    };

    // Against the heights, not the meshes (see `renzora_terrain::raycast`).
    // Paint-layer overlays and grass sit just above the surface and are not
    // part of it, so the brush reaches the ground under them.
    let hit = raycast_terrains(ray, &terrain_query, &chunk_query);

    if let Some(TerrainRayHit { point: hit_pos, terrain: terrain_entity, .. }) = hit {
        sculpt_state.hover_position = Some(hit_pos);
        sculpt_state.active_terrain = Some(terrain_entity);
        sculpt_state.brush_visible = true;

        if mouse_button.just_pressed(MouseButton::Left) {
            sculpt_state.is_sculpting = true;

            // Capture flatten start height
            if let Some((_, terrain_data, terrain_transform)) =
                terrain_query.iter().find(|(e, _, _)| *e == terrain_entity)
            {
                let local_y = hit_pos.y - terrain_transform.translation().y;
                let normalized = (local_y - terrain_data.min_height) / terrain_data.height_range();
                sculpt_state.flatten_start_height = Some(normalized.clamp(0.0, 1.0));
            }
        }
    } else {
        sculpt_state.hover_position = None;
        sculpt_state.active_terrain = None;
    }

    if mouse_button.just_released(MouseButton::Left) {
        sculpt_state.is_sculpting = false;
        sculpt_state.flatten_start_height = None;
    }
}

// ── Pick from ground ─────────────────────────────────────────────────────────

/// Set Height's "Pick from ground": while armed, the next click on the terrain
/// sets the target height to the ground under the cursor instead of sculpting.
#[derive(Resource, Default)]
pub struct HeightPick {
    pub armed: bool,
}

/// Turn an armed pick's click into a target height, and swallow the stroke it
/// would otherwise start.
///
/// Runs after the hover system, which has already sampled the ground at the
/// click (`flatten_start_height`, the same normalised height Flatten levels
/// to) and before the sculpt system, which would otherwise apply a first dab.
/// Escape disarms it.
pub fn height_pick_system(
    mut pick: ResMut<HeightPick>,
    mut sculpt_state: ResMut<TerrainSculptState>,
    mut settings: ResMut<TerrainSettings>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if !pick.armed {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        pick.armed = false;
        return;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if let Some(h) = sculpt_state.flatten_start_height {
        settings.target_height = h;
        sculpt_state.is_sculpting = false;
        pick.armed = false;
    }
}

// ── Sculpt system ────────────────────────────────────────────────────────────

/// Apply sculpting brush and draw brush gizmo.
#[allow(clippy::too_many_arguments)]
pub fn terrain_sculpt_system(
    sculpt_state: Res<TerrainSculptState>,
    settings: Res<TerrainSettings>,
    stamp_data: Res<StampBrushData>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    terrain_query: Query<(&TerrainData, &GlobalTransform)>,
    mut chunk_query: Query<(&mut TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    mut gizmos: Gizmos,
) {
    let is_stamp = settings.brush_type == TerrainBrushType::Stamp;

    // Draw brush gizmo
    if sculpt_state.brush_visible {
        if let Some(hover_pos) = sculpt_state.hover_position {
            if let Some(terrain_entity) = sculpt_state.active_terrain {
                if let Ok((terrain_data, terrain_transform)) = terrain_query.get(terrain_entity) {
                    // Borrowed once for the whole cursor: the two rings sample
                    // ~96 points, and walking the query per point is the
                    // difference between a free gizmo and a visible cost on a
                    // large grid. Dropped before the apply loop below takes the
                    // same chunks mutably.
                    let chunks: Vec<&TerrainChunkData> = chunk_query
                        .iter()
                        .filter(|(_, of, _)| of.0 == terrain_entity)
                        .map(|(chunk, _, _)| chunk)
                        .collect();
                    if is_stamp && stamp_data.is_loaded() {
                        draw_stamp_gizmo(
                            &mut gizmos,
                            hover_pos,
                            &settings,
                            &stamp_data,
                            terrain_data,
                            terrain_transform.translation(),
                            &chunks,
                        );
                    } else {
                        draw_brush_gizmo(
                            &mut gizmos,
                            hover_pos,
                            &settings,
                            terrain_data,
                            terrain_transform.translation(),
                            &chunks,
                        );
                    }
                }
            }
        }
    }

    // Stamp brush: apply once on click, not continuously
    let should_apply = if is_stamp {
        mouse_button.just_pressed(MouseButton::Left) && sculpt_state.hover_position.is_some()
    } else {
        sculpt_state.is_sculpting
    };

    if !should_apply {
        return;
    }
    let Some(hover_pos) = sculpt_state.hover_position else {
        return;
    };
    let Some(terrain_entity) = sculpt_state.active_terrain else {
        return;
    };
    let Ok((terrain_data, terrain_transform)) = terrain_query.get(terrain_entity) else {
        return;
    };

    let terrain_pos = terrain_transform.translation();
    let half_w = terrain_data.total_width() / 2.0;
    let half_d = terrain_data.total_depth() / 2.0;

    let local_x = hover_pos.x - terrain_pos.x + half_w;
    let local_z = hover_pos.z - terrain_pos.z + half_d;
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let dt = time.delta_secs();

    // Erosion works on the heights across chunk seams (a gully runs on into
    // the next chunk), so it takes the terrain's chunks together rather than
    // one at a time. Raindrops per frame scale with strength and frame time,
    // so a stroke erodes at the same rate at any frame rate.
    if matches!(settings.brush_type, TerrainBrushType::Erosion | TerrainBrushType::Hydro) {
        let mut chunks: Vec<&mut TerrainChunkData> = chunk_query
            .iter_mut()
            .filter(|(_, of, _)| of.0 == terrain_entity)
            .map(|(c, _, _)| c.into_inner())
            .collect();
        let area = Some((Vec2::new(local_x, local_z), settings.brush_radius));
        let seed = (time.elapsed_secs() * 1000.0) as u32;
        let (droplets, thermal) = match settings.brush_type {
            TerrainBrushType::Hydro => ((settings.brush_strength * dt * 6000.0) as u32 + 1, 0),
            _ => (0, (settings.brush_strength * dt * 60.0).ceil() as u32),
        };
        renzora_terrain::erosion::erode(terrain_data, &mut chunks, area, droplets, thermal, 34.0, seed);
        return;
    }

    // The terrain's chunks together: Smooth, Flatten and the other area
    // brushes read across chunk seams. The `dirty` flag a brush sets on the
    // chunks it actually moved is what drives the rebuild.
    let mut chunks: Vec<&mut TerrainChunkData> = chunk_query
        .iter_mut()
        .filter(|(_, of, _)| of.0 == terrain_entity)
        .map(|(c, _, _)| c.into_inner())
        .collect();
    if is_stamp {
        for chunk in chunks.iter_mut() {
            sculpt::apply_stamp(chunk, terrain_data, &settings, &stamp_data, local_x, local_z);
        }
    } else {
        sculpt::apply_brush_dab(terrain_data, &mut chunks, &settings, &sculpt_state, local_x, local_z, dt, shift);
    }
}

// ── Brush scroll system ──────────────────────────────────────────────────────

/// Scroll wheel resizes terrain brush.
pub fn terrain_brush_scroll_system(
    viewport: Res<ViewportState>,
    mut settings: ResMut<TerrainSettings>,
    mut scroll_events: MessageReader<MouseWheel>,
) {
    if !viewport.hovered {
        return;
    }

    for ev in scroll_events.read() {
        let factor = if ev.y > 0.0 { 1.1 } else { 0.9 };
        settings.brush_radius = (settings.brush_radius * factor).clamp(1.0, 200.0);
    }
}

// ── Brush gizmo ──────────────────────────────────────────────────────────────

/// The sculpt brush cursor. The ring itself is [`crate::brush_gizmo`], shared
/// with the paint tool — only the colour is this tool's own.
#[allow(clippy::too_many_arguments)]
fn draw_brush_gizmo(
    gizmos: &mut Gizmos,
    hover_pos: Vec3,
    settings: &TerrainSettings,
    terrain: &TerrainData,
    terrain_pos: Vec3,
    chunks: &[&TerrainChunkData],
) {
    brush_gizmo::draw_brush_cursor(
        gizmos,
        hover_pos,
        settings.brush_radius,
        settings.brush_shape,
        settings.falloff,
        brush_color(settings.brush_type),
        terrain,
        terrain_pos,
        chunks,
    );
}

/// Draw a stamp preview gizmo — shows the stamp shape as a raised wireframe grid
/// at the terrain's actual vertex density so the preview matches reality.
fn draw_stamp_gizmo(
    gizmos: &mut Gizmos,
    hover_pos: Vec3,
    settings: &TerrainSettings,
    stamp: &StampBrushData,
    terrain: &TerrainData,
    terrain_pos: Vec3,
    chunks: &[&TerrainChunkData],
) {
    let color = brush_color(settings.brush_type);
    let radius = settings.brush_radius;
    let cos_r = settings.stamp_rotation.cos();
    let sin_r = settings.stamp_rotation.sin();
    let height_scale =
        settings.stamp_height_scale * settings.brush_strength * terrain.height_range();

    // Match terrain vertex spacing so preview = actual result
    let spacing = terrain.vertex_spacing();
    let grid_res = ((radius * 2.0 / spacing).ceil() as u32).clamp(4, 64);

    // Helper: get world position for a stamp grid point
    let stamp_point = |gx: u32, gz: u32| -> Vec3 {
        let u = gx as f32 / grid_res as f32;
        let v = gz as f32 / grid_res as f32;
        let lx = (u * 2.0 - 1.0) * radius;
        let lz = (v * 2.0 - 1.0) * radius;
        let rx = lx * cos_r - lz * sin_r;
        let rz = lx * sin_r + lz * cos_r;

        let wx = hover_pos.x + rx;
        let wz = hover_pos.z + rz;

        let stamp_h = stamp.sample(u, v) * height_scale;

        let terrain_y = brush_gizmo::surface_height(wx, wz, terrain, terrain_pos, chunks)
            .unwrap_or(hover_pos.y);

        Vec3::new(wx, terrain_y + stamp_h + 0.1, wz)
    };

    // Draw grid lines along X
    for gz in 0..=grid_res {
        for gx in 0..grid_res {
            let a = stamp_point(gx, gz);
            let b = stamp_point(gx + 1, gz);
            gizmos.line(a, b, color);
        }
    }
    // Draw grid lines along Z
    for gx in 0..=grid_res {
        for gz in 0..grid_res {
            let a = stamp_point(gx, gz);
            let b = stamp_point(gx, gz + 1);
            gizmos.line(a, b, color);
        }
    }

    // Outer boundary ring
    brush_gizmo::draw_ring(
        gizmos,
        hover_pos,
        radius,
        settings.brush_shape,
        terrain,
        terrain_pos,
        chunks,
        color.with_alpha(0.5),
    );
}

fn brush_color(brush_type: TerrainBrushType) -> Color {
    match brush_type {
        TerrainBrushType::Raise => Color::srgba(0.2, 0.8, 0.2, 0.9),
        TerrainBrushType::Lower => Color::srgba(0.8, 0.4, 0.2, 0.9),
        TerrainBrushType::Sculpt => Color::srgba(0.3, 0.7, 0.3, 0.9),
        TerrainBrushType::Erase => Color::srgba(0.8, 0.2, 0.2, 0.9),
        TerrainBrushType::Smooth => Color::srgba(0.2, 0.5, 0.9, 0.9),
        TerrainBrushType::Flatten => Color::srgba(0.9, 0.9, 0.2, 0.9),
        TerrainBrushType::SetHeight => Color::srgba(0.9, 0.7, 0.1, 0.9),
        TerrainBrushType::Ramp => Color::srgba(0.9, 0.6, 0.2, 0.9),
        TerrainBrushType::Erosion => Color::srgba(0.6, 0.35, 0.1, 0.9),
        TerrainBrushType::Hydro => Color::srgba(0.1, 0.5, 0.9, 0.9),
        TerrainBrushType::Noise => Color::srgba(0.7, 0.5, 0.8, 0.9),
        TerrainBrushType::Retop => Color::srgba(0.4, 0.8, 0.4, 0.9),
        TerrainBrushType::Terrace => Color::srgba(0.8, 0.7, 0.3, 0.9),
        TerrainBrushType::Pinch => Color::srgba(0.9, 0.3, 0.7, 0.9),
        TerrainBrushType::Relax => Color::srgba(0.3, 0.8, 0.8, 0.9),
        TerrainBrushType::Cliff => Color::srgba(0.5, 0.4, 0.3, 0.9),
        TerrainBrushType::Stamp => Color::srgba(0.9, 0.6, 0.9, 0.9),
    }
}

// ── Paint systems ─────────────────────────────────────────────────────────

/// Hover detection for paint mode — mesh raycast for accurate hit on sculpted terrain.
pub fn terrain_paint_hover_system(
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    terrain_query: Query<(Entity, &TerrainData, &GlobalTransform)>,
    chunk_query: Query<(Entity, &TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    mut paint_state: ResMut<SurfacePaintState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    paint_state.brush_visible = false;

    let Some(ray) = viewport_cursor_ray(&viewport, &window_query, &camera_query) else {
        paint_state.hover_position = None;
        if !viewport.hovered {
            paint_state.active_entity = None;
        }
        return;
    };

    // Against the heights, as the sculpt hover does; painted overlays sit
    // above the surface and must not block repainting.
    let hit = raycast_terrains(ray, &terrain_query, &chunk_query);

    if let Some(TerrainRayHit { point: hit_pos, chunk: chunk_entity, chunk_uv: uv, .. }) = hit {
        paint_state.hover_position = Some(hit_pos);
        paint_state.hover_uv = Some(uv);
        paint_state.active_entity = Some(chunk_entity);
        paint_state.brush_visible = true;

        if mouse_button.just_pressed(MouseButton::Left) {
            paint_state.is_painting = true;
        }
    } else {
        paint_state.hover_position = None;
        paint_state.hover_uv = None;
        paint_state.active_entity = None;
    }

    if mouse_button.just_released(MouseButton::Left) {
        paint_state.is_painting = false;
    }
}

/// Keep `SurfacePaintState.layers_preview` mirroring the target terrain's
/// landscape layers, so the panel's layer list shows the material's layers.
/// Prefers the selected terrain, falling back to the first one (the same
/// targeting as `landscape_paint::paint_target`). Every row carries the
/// terrain's material path, which the panel shows on the selected row.
pub fn sync_layer_preview_system(
    mut paint_state: ResMut<paint::SurfacePaintState>,
    selection: Res<EditorSelection>,
    landscapes: Query<(&TerrainLandscape, Option<&renzora_terrain::landscape::TerrainMaterial>)>,
) {
    let found = selection
        .get()
        .and_then(|e| landscapes.get(e).ok())
        .or_else(|| landscapes.iter().next());
    let Some((landscape, material)) = found else {
        // Last terrain gone: empty the list rather than showing stale rows.
        if paint_state.layer_count != 0 || !paint_state.layers_preview.is_empty() {
            paint_state.layers_preview.clear();
            paint_state.layer_count = 0;
        }
        return;
    };
    let path = material.map(|m| m.path.clone()).filter(|p| !p.is_empty());
    let new_preview: Vec<paint::LayerPreview> = landscape
        .layers
        .iter()
        .map(|l| paint::LayerPreview {
            name: l.name.clone(),
            material_source: path.clone(),
            carve_depth: 0.0,
            enabled: true,
        })
        .collect();
    if new_preview.len() != paint_state.layers_preview.len()
        || new_preview
            .iter()
            .zip(paint_state.layers_preview.iter())
            .any(|(a, b)| a.name != b.name || a.material_source != b.material_source)
    {
        paint_state.layer_count = new_preview.len();
        paint_state.layers_preview = new_preview;
    }
}

// ── Undo/Redo systems ─────────────────────────────────────────────────────

use renzora_terrain::undo::TerrainStrokeSnapshot;

/// Snapshot chunk heightmaps when a sculpt stroke begins, and open the
/// weight stroke when a paint stroke does. Paint records its own "before"
/// lazily, chunk by chunk as the brush first touches each
/// (`LandscapeStroke::record`), so a paint stroke copies no heights at all.
pub fn terrain_stroke_begin_system(
    sculpt_state: Res<TerrainSculptState>,
    paint_state: Res<SurfacePaintState>,
    mut snapshot: ResMut<TerrainStrokeSnapshot>,
    mut weights: ResMut<LandscapeStroke>,
    chunk_query: Query<(Entity, &TerrainChunkData)>,
) {
    let sculpting = sculpt_state.is_sculpting;
    let painting = paint_state.is_painting;

    if (sculpting || painting) && !snapshot.active {
        snapshot.active = true;
        snapshot.chunk_snapshots.clear();
        weights.before.clear();
        if sculpting {
            for (_, chunk) in chunk_query.iter() {
                snapshot.chunk_snapshots.push((chunk.chunk_x, chunk.chunk_z, chunk.base_heights.clone()));
            }
        }
    }
}

/// Record a terrain stroke onto the central undo stack when a sculpt/paint
/// stroke ends. Exclusive (`&mut World`) so it can call `renzora_undo::record`.
/// Terrain is scene content, so the entry lands on the `Scene` stack and shows
/// in the History panel alongside every other scene edit — replacing the old
/// private `TerrainUndoStack` + hard-coded Ctrl+Z handler.
pub fn terrain_stroke_end_system(world: &mut World) {
    let sculpting = world.resource::<TerrainSculptState>().is_sculpting;
    let painting = world.resource::<SurfacePaintState>().is_painting;
    let active = world.resource::<TerrainStrokeSnapshot>().active;
    if sculpting || painting || !active {
        return;
    }

    // Take the "before" captured during the stroke and end the stroke. The
    // weight stroke's end is what lets the changed chunks be saved again.
    let mut before = {
        let mut snapshot = world.resource_mut::<TerrainStrokeSnapshot>();
        snapshot.active = false;
        TerrainUndoEntry {
            chunk_snapshots: std::mem::take(&mut snapshot.chunk_snapshots),
            weight_snapshots: Vec::new(),
        }
    };
    {
        let mut stroke = world.resource_mut::<LandscapeStroke>();
        stroke.active = false;
        before.weight_snapshots =
            stroke.before.drain().map(|(entity, (res, texels))| (entity, res, texels)).collect();
    }

    // The "after" of exactly what the "before" holds: heights only for a
    // sculpt stroke, weights only for the chunks the brush touched.
    let mut after = TerrainUndoEntry::default();
    if !before.chunk_snapshots.is_empty() {
        let mut cq = world.query::<&TerrainChunkData>();
        for chunk in cq.iter(world) {
            after.chunk_snapshots.push((chunk.chunk_x, chunk.chunk_z, chunk.base_heights.clone()));
        }
    }
    for (entity, _, _) in &before.weight_snapshots {
        if let Some(w) = world.get::<TerrainChunkWeights>(*entity) {
            after.weight_snapshots.push((*entity, w.resolution, w.texels.clone()));
        }
    }

    // Only record if the stroke actually changed something: a click that
    // painted nothing (all outside a mask, say) makes no undo step.
    let heights_changed = after.chunk_snapshots.iter().any(|(cx, cz, h)| {
        before
            .chunk_snapshots
            .iter()
            .find(|(bx, bz, _)| bx == cx && bz == cz)
            .map(|(_, _, bh)| bh != h)
            .unwrap_or(true)
    });
    let weights_changed = after.weight_snapshots.iter().any(|(e, _, texels)| {
        before
            .weight_snapshots
            .iter()
            .find(|(be, _, _)| be == e)
            .is_none_or(|(_, _, bt)| bt != texels)
    });
    if !heights_changed && !weights_changed {
        return;
    }

    // Named for what the stroke was, so the History panel and an undo reply
    // say "Sculpt: Flatten" rather than a row of identical "Terrain" entries.
    let label = if heights_changed {
        let brush = world.resource::<TerrainSettings>().brush_type;
        format!("Sculpt: {}", brush.display_name())
    } else {
        "Paint Terrain".to_string()
    };
    renzora_undo::record(
        world,
        renzora_undo::UndoContext::Scene,
        Box::new(renzora_undo::SnapshotCmd {
            label,
            before,
            after,
            restore: restore_terrain,
            // Each stroke is already one entry (sealed below); no cross-stroke merge.
            merge_key: None,
        }),
    );
    // Seal so the next stroke is a separate undo step (mouse release also seals
    // via renzora_undo's gesture seal, but be explicit).
    renzora_undo::seal(world, &renzora_undo::UndoContext::Scene);
}

/// Restore a captured terrain snapshot into the world — the `restore` fn for the
/// terrain `SnapshotCmd`. Writes chunk heightmaps and layer weights back and
/// flags them dirty so the meshes rebuild and the weights reach the GPU. Undo
/// passes the "before" blob, redo passes "after".
fn restore_terrain(world: &mut World, entry: &TerrainUndoEntry) {
    {
        let mut cq = world.query::<&mut TerrainChunkData>();
        for mut chunk in cq.iter_mut(world) {
            if let Some((_, _, heights)) = entry
                .chunk_snapshots
                .iter()
                .find(|(cx, cz, _)| *cx == chunk.chunk_x && *cz == chunk.chunk_z)
            {
                chunk.base_heights = heights.clone();
                chunk.dirty = true;
            }
        }
    }
    for (entity, resolution, texels) in &entry.weight_snapshots {
        if let Some(mut weights) = world.get_mut::<TerrainChunkWeights>(*entity) {
            // Never-painted chunks come back as a full grid of the base
            // channel, so the GPU copy is overwritten rather than left as is.
            let res = (*resolution).max(weights.resolution).max(1);
            let restored = if texels.is_empty() {
                vec![renzora_terrain::landscape::weights::solid(0); (res * res) as usize]
            } else {
                texels.clone()
            };
            weights.set_texels(if texels.is_empty() { res } else { *resolution }, restored);
        }
    }
}
