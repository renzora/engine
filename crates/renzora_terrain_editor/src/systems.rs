#![allow(dead_code)] // WIP file — many helpers staged for future panel layouts.

//! Terrain sculpting & painting systems — hover detection, brush application, gizmo rendering.

use bevy::input::mouse::MouseWheel;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::ViewportState;
use renzora::core::EditorCamera;
use renzora_terrain::data::*;
use renzora_editor_framework::EditorSelection;
use renzora_terrain::paint::{self, SurfacePaintState};
use renzora_terrain::painter::{PaintLayer, Painter};
use renzora_terrain::sculpt;
use renzora_terrain::undo::TerrainUndoEntry;

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

/// Sample terrain height at a world position (for brush gizmo).
fn sample_brush_height(
    world_x: f32,
    world_z: f32,
    terrain: &TerrainData,
    terrain_pos: Vec3,
    chunk_query: &Query<(&mut TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    terrain_entity: Entity,
) -> Option<f32> {
    let half_w = terrain.total_width() / 2.0;
    let half_d = terrain.total_depth() / 2.0;
    let local_x = world_x - terrain_pos.x + half_w;
    let local_z = world_z - terrain_pos.z + half_d;

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

        return Some(
            terrain.min_height + height_normalized * terrain.height_range() + terrain_pos.y,
        );
    }

    None
}

// ── Hover system ─────────────────────────────────────────────────────────────

/// Detect brush position on terrain via mesh raycast (accurate on sculpted surfaces).
pub fn terrain_sculpt_hover_system(
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    terrain_query: Query<(Entity, &TerrainData, &GlobalTransform)>,
    chunk_query: Query<(Entity, &TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    mut sculpt_state: ResMut<TerrainSculptState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    _settings: Res<TerrainSettings>,
    mut mesh_ray_cast: MeshRayCast,
) {
    sculpt_state.brush_visible = false;

    if !viewport.hovered {
        sculpt_state.hover_position = None;
        sculpt_state.active_terrain = None;
        return;
    }

    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        sculpt_state.hover_position = None;
        return;
    };

    // Check viewport bounds
    let vp_pos = viewport.screen_position;
    let vp_size = viewport.screen_size;
    if cursor_pos.x < vp_pos.x
        || cursor_pos.y < vp_pos.y
        || cursor_pos.x > vp_pos.x + vp_size.x
        || cursor_pos.y > vp_pos.y + vp_size.y
    {
        sculpt_state.hover_position = None;
        return;
    }

    let Some((camera, cam_transform)) = camera_query.iter().next() else {
        sculpt_state.hover_position = None;
        return;
    };

    // Viewport-local cursor mapped into render-target pixels (the target may be
    // smaller than the panel at Half / Quarter resolution).
    if vp_size.x <= 0.0 || vp_size.y <= 0.0 {
        sculpt_state.hover_position = None;
        return;
    }
    let local_pos = Vec2::new(
        (cursor_pos.x - vp_pos.x) / vp_size.x * viewport.current_size.x as f32,
        (cursor_pos.y - vp_pos.y) / vp_size.y * viewport.current_size.y as f32,
    );
    let Ok(ray) = camera.viewport_to_world(cam_transform, local_pos) else {
        sculpt_state.hover_position = None;
        return;
    };

    // Mesh raycast — hits actual sculpted geometry. Filtered to terrain
    // chunks only: the default settings early-exit on the nearest mesh, and
    // paint-layer overlays / grass sit just above the surface, so an
    // unfiltered ray dies on them and the brush goes dead over painted areas.
    let settings = MeshRayCastSettings {
        filter: &|entity| chunk_query.contains(entity),
        ..default()
    };
    let hits = mesh_ray_cast.cast_ray(ray, &settings);

    // Find closest hit on a terrain chunk entity
    let mut closest_hit: Option<(Vec3, Entity, f32)> = None;

    for (hit_entity, hit) in hits.iter() {
        // Check if this entity is a terrain chunk
        if let Some((_, _, chunk_of, _)) = chunk_query.iter().find(|(e, _, _, _)| *e == *hit_entity)
        {
            let terrain_entity = chunk_of.0;
            let dist = hit.distance;
            if closest_hit.is_none() || dist < closest_hit.as_ref().unwrap().2 {
                closest_hit = Some((hit.point, terrain_entity, dist));
            }
        }
    }

    if let Some((hit_pos, terrain_entity, _)) = closest_hit {
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

// ── Sculpt system ────────────────────────────────────────────────────────────

/// Apply sculpting brush and draw brush gizmo.
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
                    if is_stamp && stamp_data.is_loaded() {
                        draw_stamp_gizmo(
                            &mut gizmos,
                            hover_pos,
                            &settings,
                            &stamp_data,
                            terrain_data,
                            terrain_transform.translation(),
                            &chunk_query,
                            terrain_entity,
                        );
                    } else {
                        draw_brush_gizmo(
                            &mut gizmos,
                            hover_pos,
                            &settings,
                            terrain_data,
                            terrain_transform.translation(),
                            &chunk_query,
                            terrain_entity,
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

    for (mut chunk, chunk_of, _) in chunk_query.iter_mut() {
        if chunk_of.0 != terrain_entity {
            continue;
        }
        if is_stamp {
            sculpt::apply_stamp(
                &mut chunk,
                terrain_data,
                &settings,
                &stamp_data,
                local_x,
                local_z,
            );
        } else {
            sculpt::apply_brush(
                &mut chunk,
                terrain_data,
                &settings,
                &sculpt_state,
                local_x,
                local_z,
                dt,
                shift,
            );
        }
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

fn draw_brush_gizmo(
    gizmos: &mut Gizmos,
    hover_pos: Vec3,
    settings: &TerrainSettings,
    terrain: &TerrainData,
    terrain_pos: Vec3,
    chunk_query: &Query<(&mut TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    terrain_entity: Entity,
) {
    let color = brush_color(settings.brush_type);
    let radius = settings.brush_radius;
    let segments = 48usize;

    // Outer ring
    let _outer = sample_ring(
        gizmos,
        hover_pos,
        radius,
        segments,
        settings,
        terrain,
        terrain_pos,
        chunk_query,
        terrain_entity,
        color,
    );

    // Inner falloff ring
    if settings.falloff < 0.99 {
        let inner_radius = radius * (1.0 - settings.falloff);
        let inner_color = color.with_alpha(0.4);
        sample_ring(
            gizmos,
            hover_pos,
            inner_radius,
            segments,
            settings,
            terrain,
            terrain_pos,
            chunk_query,
            terrain_entity,
            inner_color,
        );
    }
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
    chunk_query: &Query<(&mut TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    terrain_entity: Entity,
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

        let terrain_y =
            sample_brush_height(wx, wz, terrain, terrain_pos, chunk_query, terrain_entity)
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
    let boundary_color = color.with_alpha(0.5);
    let segments = 48;
    sample_ring(
        gizmos,
        hover_pos,
        radius,
        segments,
        settings,
        terrain,
        terrain_pos,
        chunk_query,
        terrain_entity,
        boundary_color,
    );
}

fn sample_ring(
    gizmos: &mut Gizmos,
    center: Vec3,
    radius: f32,
    segments: usize,
    settings: &TerrainSettings,
    terrain: &TerrainData,
    terrain_pos: Vec3,
    chunk_query: &Query<(&mut TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    terrain_entity: Entity,
    color: Color,
) -> Vec<Vec3> {
    let mut points = Vec::with_capacity(segments);

    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let (sin_a, cos_a) = angle.sin_cos();

        let (dx, dz) = match settings.brush_shape {
            BrushShape::Circle => (cos_a * radius, sin_a * radius),
            BrushShape::Square => {
                let t = angle / std::f32::consts::FRAC_PI_2;
                let side = (t.floor() as i32) % 4;
                let frac = t.fract();
                match side {
                    0 => (radius, (frac * 2.0 - 1.0) * radius),
                    1 => ((1.0 - frac * 2.0) * radius, radius),
                    2 => (-radius, (1.0 - frac * 2.0) * radius),
                    _ => ((frac * 2.0 - 1.0) * radius, -radius),
                }
            }
            BrushShape::Diamond => {
                let t = angle / std::f32::consts::FRAC_PI_2;
                let side = (t.floor() as i32) % 4;
                let frac = t.fract();
                match side {
                    0 => ((1.0 - frac) * radius, frac * radius),
                    1 => (-frac * radius, (1.0 - frac) * radius),
                    2 => (-(1.0 - frac) * radius, -frac * radius),
                    _ => (frac * radius, -(1.0 - frac) * radius),
                }
            }
        };

        let wx = center.x + dx;
        let wz = center.z + dz;

        let height = sample_brush_height(wx, wz, terrain, terrain_pos, chunk_query, terrain_entity)
            .unwrap_or(center.y);

        points.push(Vec3::new(wx, height + 0.15, wz));
    }

    // Draw line segments
    for i in 0..segments {
        let next = (i + 1) % segments;
        gizmos.line(points[i], points[next], color);
    }

    points
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
    mut mesh_ray_cast: MeshRayCast,
) {
    paint_state.brush_visible = false;

    if !viewport.hovered {
        paint_state.hover_position = None;
        paint_state.active_entity = None;
        return;
    }

    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        paint_state.hover_position = None;
        return;
    };

    let vp_pos = viewport.screen_position;
    let vp_size = viewport.screen_size;
    if cursor_pos.x < vp_pos.x
        || cursor_pos.y < vp_pos.y
        || cursor_pos.x > vp_pos.x + vp_size.x
        || cursor_pos.y > vp_pos.y + vp_size.y
    {
        paint_state.hover_position = None;
        return;
    }

    let Some((camera, cam_transform)) = camera_query.iter().next() else {
        paint_state.hover_position = None;
        return;
    };

    if vp_size.x <= 0.0 || vp_size.y <= 0.0 {
        paint_state.hover_position = None;
        return;
    }
    let local_pos = Vec2::new(
        (cursor_pos.x - vp_pos.x) / vp_size.x * viewport.current_size.x as f32,
        (cursor_pos.y - vp_pos.y) / vp_size.y * viewport.current_size.y as f32,
    );
    let Ok(ray) = camera.viewport_to_world(cam_transform, local_pos) else {
        paint_state.hover_position = None;
        return;
    };

    // Mesh raycast — hits actual sculpted geometry. Chunks only, for the
    // same reason as the sculpt hover: painted overlays would otherwise
    // swallow the ray and block repainting already-painted areas.
    let settings = MeshRayCastSettings {
        filter: &|entity| chunk_query.contains(entity),
        ..default()
    };
    let hits = mesh_ray_cast.cast_ray(ray, &settings);

    let mut closest: Option<(Vec3, Entity, Entity, Vec2, f32)> = None;

    for (hit_entity, hit) in hits.iter() {
        // Check if this entity is a terrain chunk
        if let Some((_, _chunk_data, chunk_of, chunk_transform)) =
            chunk_query.iter().find(|(e, _, _, _)| *e == *hit_entity)
        {
            let terrain_entity = chunk_of.0;
            let Ok((_, terrain_data, _)) = terrain_query.get(terrain_entity) else {
                continue;
            };

            // Convert world hit position to chunk UV
            let local_hit = hit.point - chunk_transform.translation();
            let uv = Vec2::new(
                local_hit.x / terrain_data.chunk_size,
                local_hit.z / terrain_data.chunk_size,
            );

            let dist = hit.distance;
            if closest.is_none() || dist < closest.as_ref().unwrap().4 {
                closest = Some((hit.point, *hit_entity, terrain_entity, uv, dist));
            }
        }
    }

    if let Some((hit_pos, chunk_entity, _terrain_entity, uv, _)) = closest {
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

/// Keep `SurfacePaintState.layers_preview` mirroring the target `Painter`'s
/// layers every frame, so the panel's layer list shows live values even when
/// no paint commands ran this frame. Prefers the selected terrain's painter,
/// falling back to the first one (same targeting as `painter_command_system`).
pub fn sync_layer_preview_system(
    mut paint_state: ResMut<paint::SurfacePaintState>,
    selection: Res<EditorSelection>,
    painters: Query<&Painter>,
) {
    let painter = selection
        .get()
        .and_then(|e| painters.get(e).ok())
        .or_else(|| painters.iter().next());
    let Some(painter) = painter else {
        // Last terrain gone: empty the list rather than showing stale rows.
        if paint_state.layer_count != 0 || !paint_state.layers_preview.is_empty() {
            paint_state.layers_preview.clear();
            paint_state.layer_count = 0;
        }
        return;
    };
    let new_preview: Vec<paint::LayerPreview> = painter
        .layers
        .iter()
        .map(|l| paint::LayerPreview {
            name: l.name.clone(),
            material_source: l.material_path.clone(),
            carve_depth: 0.0,
            enabled: l.enabled,
        })
        .collect();
    if new_preview.len() != paint_state.layers_preview.len()
        || new_preview
            .iter()
            .zip(paint_state.layers_preview.iter())
            .any(|(a, b)| {
                a.name != b.name
                    || a.material_source != b.material_source
                    || a.enabled != b.enabled
            })
    {
        paint_state.layers_preview = new_preview;
        paint_state.layer_count = painter.layers.len();
    }
}

// ── Undo/Redo systems ─────────────────────────────────────────────────────

use renzora_terrain::undo::TerrainStrokeSnapshot;

/// Snapshot chunk heightmaps + painter layer masks when a sculpt/paint stroke
/// begins.
pub fn terrain_stroke_begin_system(
    sculpt_state: Res<TerrainSculptState>,
    paint_state: Res<SurfacePaintState>,
    mut snapshot: ResMut<TerrainStrokeSnapshot>,
    chunk_query: Query<(Entity, &TerrainChunkData)>,
    painter_query: Query<(Entity, &Painter)>,
) {
    let sculpting = sculpt_state.is_sculpting;
    let painting = paint_state.is_painting;

    if (sculpting || painting) && !snapshot.active {
        // Capture before-state
        snapshot.active = true;
        snapshot.chunk_snapshots.clear();
        snapshot.layer_snapshots.clear();

        for (_, chunk) in chunk_query.iter() {
            snapshot.chunk_snapshots.push((
                chunk.chunk_x,
                chunk.chunk_z,
                chunk.base_heights.clone(),
            ));
        }
        for (entity, painter) in painter_query.iter() {
            snapshot.layer_snapshots.push((entity, painter.layers.clone()));
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

    // Snapshot the post-stroke ("after") state.
    let mut after = TerrainUndoEntry {
        chunk_snapshots: Vec::new(),
        layer_snapshots: Vec::new(),
    };
    {
        let mut cq = world.query::<&TerrainChunkData>();
        for chunk in cq.iter(world) {
            after
                .chunk_snapshots
                .push((chunk.chunk_x, chunk.chunk_z, chunk.base_heights.clone()));
        }
        let mut sq = world.query::<(Entity, &Painter)>();
        for (entity, painter) in sq.iter(world) {
            after.layer_snapshots.push((entity, painter.layers.clone()));
        }
    }

    // Take the "before" snapshot captured at stroke begin and end the stroke.
    let before = {
        let mut snapshot = world.resource_mut::<TerrainStrokeSnapshot>();
        snapshot.active = false;
        TerrainUndoEntry {
            chunk_snapshots: std::mem::take(&mut snapshot.chunk_snapshots),
            layer_snapshots: std::mem::take(&mut snapshot.layer_snapshots),
        }
    };

    // Only record if the stroke actually changed heights or paint masks — a
    // click that painted nothing shouldn't create an empty undo step. (The old
    // code always pushed for paint; here we compare masks properly.)
    let heights_changed = after.chunk_snapshots.iter().any(|(cx, cz, h)| {
        before
            .chunk_snapshots
            .iter()
            .find(|(bx, bz, _)| bx == cx && bz == cz)
            .map(|(_, _, bh)| bh != h)
            .unwrap_or(true)
    });
    let layers_changed = after.layer_snapshots.iter().any(|(e, layers)| {
        before
            .layer_snapshots
            .iter()
            .find(|(be, _)| be == e)
            .map(|(_, bl)| !painter_layers_equal(bl, layers))
            .unwrap_or(true)
    });
    if !heights_changed && !layers_changed {
        return;
    }

    renzora_undo::record(
        world,
        renzora_undo::UndoContext::Scene,
        Box::new(renzora_undo::SnapshotCmd {
            label: "Terrain".to_string(),
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
/// terrain `SnapshotCmd`. Writes chunk heightmaps and painter layer masks back
/// and flags them dirty so the chunk + layer meshes rebuild. Undo passes the
/// "before" blob, redo passes "after".
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
    for (entity, layers) in &entry.layer_snapshots {
        if let Some(mut painter) = world.get_mut::<Painter>(*entity) {
            // Replace the whole stack: a stroke can have CREATED a layer
            // (first-paint auto-create), so undo must be able to remove it and
            // redo to bring it back. The `Changed<Painter>` sync system
            // despawns/spawns the child mesh entities to match.
            painter.layers = layers.clone();
            painter.active_layer = painter
                .active_layer
                .filter(|_| !layers.is_empty())
                .map(|a| a.min(layers.len().saturating_sub(1)));
            for layer in painter.layers.iter_mut() {
                layer.mesh_dirty = true;
                layer.material_dirty = true;
            }
        }
    }
}

/// Field-wise layer-stack comparison that ignores the transient dirty flags
/// (they'd otherwise flag every rebuilt-but-unchanged stroke as an edit).
fn painter_layers_equal(a: &[PaintLayer], b: &[PaintLayer]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b.iter()).all(|(x, y)| {
            x.name == y.name
                && x.material_path == y.material_path
                && x.mask == y.mask
                && x.coverage_threshold == y.coverage_threshold
                && x.height_offset == y.height_offset
                && x.enabled == y.enabled
        })
}
