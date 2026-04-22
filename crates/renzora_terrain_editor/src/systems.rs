#![allow(dead_code)] // WIP file — many helpers staged for future panel layouts.

//! Terrain sculpting & painting systems — hover detection, brush application, gizmo rendering.

use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::window::PrimaryWindow;

use renzora::core::EditorCamera;
use renzora_terrain::data::*;
use renzora_terrain::sculpt;
use renzora_terrain::paint::{self, PaintableSurfaceData, SurfacePaintSettings, SurfacePaintState};
use renzora_terrain::splatmap_material::TerrainSplatmapMaterial;
use renzora_terrain::splatmap_systems::{self, SplatmapActive};
use renzora_terrain::undo::{TerrainUndoStack, TerrainUndoEntry};
use renzora::core::viewport_types::ViewportState;

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

        return Some(terrain.min_height + height_normalized * terrain.height_range() + terrain_pos.y);
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

    let Ok(window) = window_query.single() else { return };
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

    // Viewport-local coordinates
    let local_pos = Vec2::new(cursor_pos.x - vp_pos.x, cursor_pos.y - vp_pos.y);
    let Ok(ray) = camera.viewport_to_world(cam_transform, local_pos) else {
        sculpt_state.hover_position = None;
        return;
    };

    // Mesh raycast — hits actual sculpted geometry
    let hits = mesh_ray_cast.cast_ray(ray, &MeshRayCastSettings { ..default() });

    // Find closest hit on a terrain chunk entity
    let mut closest_hit: Option<(Vec3, Entity, f32)> = None;

    for (hit_entity, hit) in hits.iter() {
        // Check if this entity is a terrain chunk
        if let Some((_, _, chunk_of, _)) = chunk_query.iter().find(|(e, _, _, _)| *e == *hit_entity) {
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
    let Some(hover_pos) = sculpt_state.hover_position else { return };
    let Some(terrain_entity) = sculpt_state.active_terrain else { return };
    let Ok((terrain_data, terrain_transform)) = terrain_query.get(terrain_entity) else { return };

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
        gizmos, hover_pos, radius, segments, settings, terrain, terrain_pos,
        chunk_query, terrain_entity, color,
    );

    // Inner falloff ring
    if settings.falloff < 0.99 {
        let inner_radius = radius * (1.0 - settings.falloff);
        let inner_color = color.with_alpha(0.4);
        sample_ring(
            gizmos, hover_pos, inner_radius, segments, settings, terrain, terrain_pos,
            chunk_query, terrain_entity, inner_color,
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
    let height_scale = settings.stamp_height_scale * settings.brush_strength * terrain.height_range();

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

        let terrain_y = sample_brush_height(wx, wz, terrain, terrain_pos, chunk_query, terrain_entity)
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
        gizmos, hover_pos, radius, segments, settings, terrain, terrain_pos,
        chunk_query, terrain_entity, boundary_color,
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
        TerrainBrushType::Raise     => Color::srgba(0.2, 0.8, 0.2, 0.9),
        TerrainBrushType::Lower     => Color::srgba(0.8, 0.4, 0.2, 0.9),
        TerrainBrushType::Sculpt    => Color::srgba(0.3, 0.7, 0.3, 0.9),
        TerrainBrushType::Erase     => Color::srgba(0.8, 0.2, 0.2, 0.9),
        TerrainBrushType::Smooth    => Color::srgba(0.2, 0.5, 0.9, 0.9),
        TerrainBrushType::Flatten   => Color::srgba(0.9, 0.9, 0.2, 0.9),
        TerrainBrushType::SetHeight => Color::srgba(0.9, 0.7, 0.1, 0.9),
        TerrainBrushType::Ramp      => Color::srgba(0.9, 0.6, 0.2, 0.9),
        TerrainBrushType::Erosion   => Color::srgba(0.6, 0.35, 0.1, 0.9),
        TerrainBrushType::Hydro     => Color::srgba(0.1, 0.5, 0.9, 0.9),
        TerrainBrushType::Noise     => Color::srgba(0.7, 0.5, 0.8, 0.9),
        TerrainBrushType::Retop     => Color::srgba(0.4, 0.8, 0.4, 0.9),
        TerrainBrushType::Terrace   => Color::srgba(0.8, 0.7, 0.3, 0.9),
        TerrainBrushType::Pinch     => Color::srgba(0.9, 0.3, 0.7, 0.9),
        TerrainBrushType::Relax     => Color::srgba(0.3, 0.8, 0.8, 0.9),
        TerrainBrushType::Cliff     => Color::srgba(0.5, 0.4, 0.3, 0.9),
        TerrainBrushType::Stamp     => Color::srgba(0.9, 0.6, 0.9, 0.9),
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

    let Ok(window) = window_query.single() else { return };
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

    let local_pos = Vec2::new(cursor_pos.x - vp_pos.x, cursor_pos.y - vp_pos.y);
    let Ok(ray) = camera.viewport_to_world(cam_transform, local_pos) else {
        paint_state.hover_position = None;
        return;
    };

    // Mesh raycast — hits actual sculpted geometry
    let hits = mesh_ray_cast.cast_ray(ray, &MeshRayCastSettings { ..default() });

    let mut closest: Option<(Vec3, Entity, Entity, Vec2, f32)> = None;

    for (hit_entity, hit) in hits.iter() {
        // Check if this entity is a terrain chunk
        if let Some((_, _chunk_data, chunk_of, chunk_transform)) =
            chunk_query.iter().find(|(e, _, _, _)| *e == *hit_entity)
        {
            let terrain_entity = chunk_of.0;
            let Ok((_, terrain_data, _)) = terrain_query.get(terrain_entity) else { continue };

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

/// Apply paint brush and draw paint brush gizmo.
pub fn terrain_paint_system(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut splatmap_materials: ResMut<Assets<TerrainSplatmapMaterial>>,
    paint_state: Res<SurfacePaintState>,
    paint_settings: Res<SurfacePaintSettings>,
    time: Res<Time>,
    _terrain_query: Query<(&TerrainData, &GlobalTransform)>,
    mut chunk_query: Query<(
        &mut PaintableSurfaceData,
        &TerrainChunkOf,
        Option<&SplatmapActive>,
    )>,
    mut gizmos: Gizmos,
    layer_tex: Res<splatmap_systems::TerrainLayerTextures>,
) {
    // Draw paint brush gizmo
    if paint_state.brush_visible {
        if let Some(hover_pos) = paint_state.hover_position {
            let radius = paint_settings.brush_radius;
            let color = paint_brush_color(&paint_settings);
            let segments = 48;
            for i in 0..segments {
                let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
                // Paint brush radius is in UV space, convert to approximate world units
                // For now use a fixed world-scale multiplier
                let scale = 64.0; // approximate chunk size for visualization
                let p0 = Vec3::new(
                    hover_pos.x + a0.cos() * radius * scale,
                    hover_pos.y + 0.15,
                    hover_pos.z + a0.sin() * radius * scale,
                );
                let p1 = Vec3::new(
                    hover_pos.x + a1.cos() * radius * scale,
                    hover_pos.y + 0.15,
                    hover_pos.z + a1.sin() * radius * scale,
                );
                gizmos.line(p0, p1, color);
            }
        }
    }

    // Apply painting
    if !paint_state.is_painting {
        return;
    }
    let Some(uv) = paint_state.hover_uv else { return };
    let Some(chunk_entity) = paint_state.active_entity else { return };
    let dt = time.delta_secs();

    // Get the chunk's surface data
    let Ok((mut surface, _chunk_of, splatmap_active)) = chunk_query.get_mut(chunk_entity) else {
        return;
    };

    // Ensure splatmap material is active on this chunk
    if splatmap_active.is_none() {
        splatmap_systems::activate_splatmap_on_chunk(
            &mut commands,
            chunk_entity,
            &surface,
            &mut images,
            &mut splatmap_materials,
            &layer_tex,
        );
    }

    // Apply the paint brush
    paint::apply_paint_brush(&mut surface, &paint_settings, uv, dt);
}

/// Activate splatmap on all terrain chunks when entering paint mode.
pub fn terrain_paint_activate_system(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut splatmap_materials: ResMut<Assets<TerrainSplatmapMaterial>>,
    _settings: Res<TerrainSettings>,
    chunk_query: Query<
        (Entity, &TerrainChunkData, &TerrainChunkOf),
        Without<SplatmapActive>,
    >,
    surface_query: Query<&PaintableSurfaceData>,
    layer_tex: Res<splatmap_systems::TerrainLayerTextures>,
) {
    for (chunk_entity, _chunk_data, _chunk_of) in chunk_query.iter() {
        // Check if parent terrain has PaintableSurfaceData, or add one
        // For simplicity, add PaintableSurfaceData to each chunk that lacks it
        if let Ok(surface) = surface_query.get(chunk_entity) {
            splatmap_systems::activate_splatmap_on_chunk(
                &mut commands,
                chunk_entity,
                surface,
                &mut images,
                &mut splatmap_materials,
                &layer_tex,
            );
        } else {
            // Add default PaintableSurfaceData then activate
            let surface = PaintableSurfaceData::default();
            splatmap_systems::activate_splatmap_on_chunk(
                &mut commands,
                chunk_entity,
                &surface,
                &mut images,
                &mut splatmap_materials,
                &layer_tex,
            );
            commands.entity(chunk_entity).insert(surface);
        }
    }
}

/// Scroll wheel resizes paint brush.
pub fn terrain_paint_scroll_system(
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

/// Process pending commands from the paint UI (add/remove layers, assign materials).
pub fn terrain_paint_command_system(
    mut paint_state: ResMut<SurfacePaintState>,
    mut surface_query: Query<&mut PaintableSurfaceData>,
    vfs: Res<renzora::core::VirtualFileReader>,
    asset_server: Res<AssetServer>,
    mut layer_tex: ResMut<splatmap_systems::TerrainLayerTextures>,
) {
    let commands: Vec<_> = paint_state.pending_commands.drain(..).collect();
    if commands.is_empty() {
        return;
    }

    for mut surface in surface_query.iter_mut() {
        for cmd in &commands {
            match cmd {
                paint::SurfacePaintCommand::AddLayer => {
                    if surface.layers.len() < paint::MAX_LAYERS {
                        let n = surface.layers.len() + 1;
                        surface.layers.push(paint::MaterialLayer {
                            name: format!("Layer {}", n),
                            ..Default::default()
                        });
                        surface.dirty = true;
                    }
                }
                paint::SurfacePaintCommand::RemoveLayer(idx) => {
                    if *idx < surface.layers.len() && surface.layers.len() > 1 {
                        surface.layers.remove(*idx);
                        // Clear texture handles for this layer
                        if *idx < 8 {
                            layer_tex.layer_albedo[*idx] = None;
                            layer_tex.layer_normal[*idx] = None;
                            layer_tex.layer_arm[*idx] = None;
                            layer_tex.dirty = true;
                        }
                        surface.dirty = true;
                    }
                }
                paint::SurfacePaintCommand::AssignMaterial { layer, path } => {
                    if *layer < surface.layers.len() {
                        // Read and parse the .material file
                        if let Some(json) = vfs.read_string(path) {
                            if let Ok(textures) = extract_layer_textures_from_json(&json) {
                                surface.layers[*layer].material_path = Some(path.clone());
                                surface.layers[*layer].albedo_path = textures.0.clone();
                                surface.layers[*layer].normal_path = textures.1.clone();
                                surface.layers[*layer].arm_path = textures.2.clone();

                                // Load textures via asset server
                                if let Some(ref albedo) = textures.0 {
                                    layer_tex.layer_albedo[*layer] = Some(asset_server.load(albedo.clone()));
                                }
                                if let Some(ref normal) = textures.1 {
                                    layer_tex.layer_normal[*layer] = Some(asset_server.load(normal.clone()));
                                }
                                if let Some(ref arm) = textures.2 {
                                    layer_tex.layer_arm[*layer] = Some(asset_server.load(arm.clone()));
                                }

                                layer_tex.dirty = true;
                                surface.dirty = true;
                            }
                        }
                    }
                }
                paint::SurfacePaintCommand::ClearMaterial(idx) => {
                    if *idx < surface.layers.len() {
                        surface.layers[*idx].material_path = None;
                        surface.layers[*idx].albedo_path = None;
                        surface.layers[*idx].normal_path = None;
                        surface.layers[*idx].arm_path = None;

                        if *idx < 8 {
                            layer_tex.layer_albedo[*idx] = None;
                            layer_tex.layer_normal[*idx] = None;
                            layer_tex.layer_arm[*idx] = None;
                            layer_tex.dirty = true;
                        }
                        surface.dirty = true;
                    }
                }
            }
        }

        // Update preview cache
        paint_state.layers_preview = surface
            .layers
            .iter()
            .map(|l| paint::LayerPreview {
                name: l.name.clone(),
                material_source: l.material_path.clone(),
                carve_depth: l.carve_depth,
                enabled: l.enabled,
            })
            .collect();
        paint_state.layer_count = surface.layers.len();
    }
}

/// Keep `SurfacePaintState.layers_preview` in sync with any chunk's
/// `PaintableSurfaceData` every frame, so the inspector shows live values
/// even when no paint commands have run this frame.
pub fn sync_layer_preview_system(
    mut paint_state: ResMut<paint::SurfacePaintState>,
    surfaces: Query<&paint::PaintableSurfaceData>,
) {
    let Some(surface) = surfaces.iter().next() else {
        return;
    };
    let new_preview: Vec<paint::LayerPreview> = surface
        .layers
        .iter()
        .map(|l| paint::LayerPreview {
            name: l.name.clone(),
            material_source: l.material_path.clone(),
            carve_depth: l.carve_depth,
            enabled: l.enabled,
        })
        .collect();
    if new_preview.len() != paint_state.layers_preview.len()
        || new_preview.iter().zip(paint_state.layers_preview.iter()).any(|(a, b)| {
            a.name != b.name
                || a.material_source != b.material_source
                || (a.carve_depth - b.carve_depth).abs() > 1e-5
                || a.enabled != b.enabled
        })
    {
        paint_state.layers_preview = new_preview;
        paint_state.layer_count = surface.layers.len();
    }
}

fn paint_brush_color(settings: &SurfacePaintSettings) -> Color {
    match settings.brush_type {
        paint::PaintBrushType::Paint  => Color::srgba(0.2, 0.7, 0.9, 0.9),
        paint::PaintBrushType::Erase  => Color::srgba(0.9, 0.3, 0.2, 0.9),
        paint::PaintBrushType::Smooth => Color::srgba(0.3, 0.6, 0.9, 0.9),
        paint::PaintBrushType::Fill   => Color::srgba(0.9, 0.8, 0.2, 0.9),
    }
}

// ── Undo/Redo systems ─────────────────────────────────────────────────────

use renzora_terrain::undo::TerrainStrokeSnapshot;

/// Snapshot chunk heightmaps when a sculpt/paint stroke begins.
pub fn terrain_stroke_begin_system(
    sculpt_state: Res<TerrainSculptState>,
    paint_state: Res<SurfacePaintState>,
    mut snapshot: ResMut<TerrainStrokeSnapshot>,
    chunk_query: Query<(Entity, &TerrainChunkData)>,
    surface_query: Query<(Entity, &PaintableSurfaceData)>,
) {
    let sculpting = sculpt_state.is_sculpting;
    let painting = paint_state.is_painting;

    if (sculpting || painting) && !snapshot.active {
        // Capture before-state
        snapshot.active = true;
        snapshot.chunk_snapshots.clear();
        snapshot.layer_mask_snapshots.clear();

        for (_, chunk) in chunk_query.iter() {
            snapshot.chunk_snapshots.push((chunk.chunk_x, chunk.chunk_z, chunk.base_heights.clone()));
        }
        for (entity, surface) in surface_query.iter() {
            let masks: Vec<Vec<f32>> = surface.layers.iter().map(|l| l.mask.clone()).collect();
            snapshot.layer_mask_snapshots.push((entity, masks));
        }
    }
}

/// Push undo entry when a sculpt/paint stroke ends.
pub fn terrain_stroke_end_system(
    sculpt_state: Res<TerrainSculptState>,
    paint_state: Res<SurfacePaintState>,
    mut snapshot: ResMut<TerrainStrokeSnapshot>,
    mut undo_stack: ResMut<TerrainUndoStack>,
    chunk_query: Query<&TerrainChunkData>,
) {
    let sculpting = sculpt_state.is_sculpting;
    let painting = paint_state.is_painting;

    if !sculpting && !painting && snapshot.active {
        // Check if anything actually changed
        let mut changed = false;
        for (_, chunk) in chunk_query.iter().enumerate() {
            if let Some(snap) = snapshot.chunk_snapshots.iter().find(|(cx, cz, _)| *cx == chunk.chunk_x && *cz == chunk.chunk_z) {
                if snap.2 != chunk.base_heights {
                    changed = true;
                    break;
                }
            }
        }
        // Also check splatmaps (rough check — if sculpt didn't change, paint might have)
        if !changed {
            changed = true; // conservatively push if painting was active
        }

        if changed {
            let entry = TerrainUndoEntry {
                chunk_snapshots: std::mem::take(&mut snapshot.chunk_snapshots),
                layer_mask_snapshots: std::mem::take(&mut snapshot.layer_mask_snapshots),
            };
            undo_stack.push_undo(entry);
        }

        snapshot.active = false;
        snapshot.chunk_snapshots.clear();
        snapshot.layer_mask_snapshots.clear();
    }
}

/// Ctrl+Z / Ctrl+Y: undo/redo terrain edits.
pub fn terrain_undo_redo_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut undo_stack: ResMut<TerrainUndoStack>,
    mut chunk_query: Query<&mut TerrainChunkData>,
    mut surface_query: Query<(Entity, &mut PaintableSurfaceData)>,
) {
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    if !ctrl {
        return;
    }

    let undo = keyboard.just_pressed(KeyCode::KeyZ) && !keyboard.pressed(KeyCode::ShiftLeft);
    let redo = keyboard.just_pressed(KeyCode::KeyY)
        || (keyboard.just_pressed(KeyCode::KeyZ) && keyboard.pressed(KeyCode::ShiftLeft));

    if undo {
        if let Some(entry) = undo_stack.undo.pop() {
            // Capture current state as redo entry
            let mut redo_entry = TerrainUndoEntry {
                chunk_snapshots: Vec::new(),
                layer_mask_snapshots: Vec::new(),
            };
            for chunk in chunk_query.iter_mut() {
                redo_entry.chunk_snapshots.push((chunk.chunk_x, chunk.chunk_z, chunk.base_heights.clone()));
            }
            for (entity, surface) in surface_query.iter() {
                let masks: Vec<Vec<f32>> = surface.layers.iter().map(|l| l.mask.clone()).collect();
                redo_entry.layer_mask_snapshots.push((entity, masks));
            }
            undo_stack.redo.push(redo_entry);

            // Restore from undo entry
            apply_undo_entry(&entry, &mut chunk_query, &mut surface_query);
        }
    } else if redo {
        if let Some(entry) = undo_stack.redo.pop() {
            // Capture current state as undo entry
            let mut undo_entry = TerrainUndoEntry {
                chunk_snapshots: Vec::new(),
                layer_mask_snapshots: Vec::new(),
            };
            for chunk in chunk_query.iter_mut() {
                undo_entry.chunk_snapshots.push((chunk.chunk_x, chunk.chunk_z, chunk.base_heights.clone()));
            }
            for (entity, surface) in surface_query.iter() {
                let masks: Vec<Vec<f32>> = surface.layers.iter().map(|l| l.mask.clone()).collect();
                undo_entry.layer_mask_snapshots.push((entity, masks));
            }
            undo_stack.undo.push(undo_entry);

            // Restore from redo entry
            apply_undo_entry(&entry, &mut chunk_query, &mut surface_query);
        }
    }
}

fn apply_undo_entry(
    entry: &TerrainUndoEntry,
    chunk_query: &mut Query<&mut TerrainChunkData>,
    surface_query: &mut Query<(Entity, &mut PaintableSurfaceData)>,
) {
    for (cx, cz, heights) in &entry.chunk_snapshots {
        for mut chunk in chunk_query.iter_mut() {
            if chunk.chunk_x == *cx && chunk.chunk_z == *cz {
                chunk.base_heights = heights.clone();
                chunk.dirty = true;
            }
        }
    }
    for (entity, masks) in &entry.layer_mask_snapshots {
        if let Ok((_, mut surface)) = surface_query.get_mut(*entity) {
            for (i, mask) in masks.iter().enumerate() {
                if let Some(layer) = surface.layers.get_mut(i) {
                    layer.mask = mask.clone();
                }
            }
            surface.dirty = true;
        }
    }
}

// ── Foliage scatter system ────────────────────────────────────────────────

use renzora_terrain::foliage::{TerrainFoliageConfig, FoliageBatch, generate_foliage_instances};

/// Regenerate foliage instances when splatmap or foliage config changes.
pub fn terrain_foliage_scatter_system(
    mut commands: Commands,
    terrain_query: Query<(Entity, &TerrainData)>,
    chunk_query: Query<(Entity, &TerrainChunkData, &TerrainChunkOf, &GlobalTransform, &PaintableSurfaceData)>,
    foliage_query: Query<(Entity, &TerrainFoliageConfig)>,
    existing_batches: Query<(Entity, &FoliageBatch)>,
    asset_server: Res<AssetServer>,
    _meshes: ResMut<Assets<Mesh>>,
) {
    // Only process if there are foliage configs
    if foliage_query.is_empty() {
        return;
    }

    for (config_entity, config) in foliage_query.iter() {
        if !config.enabled || config.mesh_path.is_empty() {
            // Remove existing batches for this config
            for (batch_entity, batch) in existing_batches.iter() {
                if batch.config_entity == config_entity {
                    commands.entity(batch_entity).despawn();
                }
            }
            continue;
        }

        let mesh_handle: Handle<Mesh> = asset_server.load(&config.mesh_path);

        for (_chunk_entity, chunk_data, chunk_of, chunk_transform, surface) in chunk_query.iter() {
            let Ok((_, terrain_data)) = terrain_query.get(chunk_of.0) else { continue };

            // Check if this chunk's splatmap is dirty or batch doesn't exist
            let batch_exists = existing_batches.iter().any(|(_, b)| {
                b.config_entity == config_entity
                    && b.chunk_x == chunk_data.chunk_x
                    && b.chunk_z == chunk_data.chunk_z
            });

            if batch_exists && !surface.dirty {
                continue;
            }

            // Remove old batch for this chunk+config
            for (batch_entity, batch) in existing_batches.iter() {
                if batch.config_entity == config_entity
                    && batch.chunk_x == chunk_data.chunk_x
                    && batch.chunk_z == chunk_data.chunk_z
                {
                    commands.entity(batch_entity).despawn();
                }
            }

            let instances = generate_foliage_instances(
                config,
                &surface.splatmap_weights,
                surface.splatmap_resolution,
                &chunk_data.heights,
                terrain_data.chunk_resolution,
                terrain_data.chunk_size,
                terrain_data.min_height,
                terrain_data.height_range(),
                chunk_data.chunk_x * 1000 + chunk_data.chunk_z,
            );

            // Spawn instances as child entities
            let chunk_pos = chunk_transform.translation();
            for instance_transform in &instances {
                let world_transform = Transform {
                    translation: chunk_pos + instance_transform.translation,
                    rotation: instance_transform.rotation,
                    scale: instance_transform.scale,
                };
                commands.spawn((
                    Mesh3d(mesh_handle.clone()),
                    world_transform,
                    Visibility::default(),
                    FoliageBatch {
                        config_entity,
                        chunk_x: chunk_data.chunk_x,
                        chunk_z: chunk_data.chunk_z,
                    },
                ));
            }
        }
    }
}

/// Extract albedo, normal, and ARM texture paths from a material graph JSON
/// without depending on the renzora_shader crate.
///
/// Returns `(albedo, normal, arm)` as `Option<String>`.
fn extract_layer_textures_from_json(
    json: &str,
) -> Result<(Option<String>, Option<String>, Option<String>), serde_json::Error> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let nodes = v["nodes"].as_array();
    let connections = v["connections"].as_array();

    let (Some(nodes), Some(connections)) = (nodes, connections) else {
        return Ok((None, None, None));
    };

    // Find the output node
    let output_node = nodes.iter().find(|n| {
        n["node_type"]
            .as_str()
            .map_or(false, |t| t.starts_with("output/"))
    });
    let Some(output_node) = output_node else {
        return Ok((None, None, None));
    };
    let output_id = output_node["id"].as_u64().unwrap_or(0);

    let trace_texture = |pin_name: &str| -> Option<String> {
        // Find connection to output_node's pin
        let conn = connections.iter().find(|c| {
            c["to_node"].as_u64() == Some(output_id) && c["to_pin"].as_str() == Some(pin_name)
        })?;
        let from_node_id = conn["from_node"].as_u64()?;
        // Find source node
        let source = nodes.iter().find(|n| n["id"].as_u64() == Some(from_node_id))?;
        // Check if it's a texture sample node
        let node_type = source["node_type"].as_str()?;
        if !node_type.contains("texture") {
            return None;
        }
        // Extract texture path from input_values
        let input_vals = source.get("input_values")?.as_object()?;
        for (_key, val) in input_vals {
            if let Some(s) = val.as_str() {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
            // Also handle {"Texture": "path"} format
            if let Some(obj) = val.as_object() {
                if let Some(tex) = obj.get("Texture").and_then(|v| v.as_str()) {
                    if !tex.is_empty() {
                        return Some(tex.to_string());
                    }
                }
            }
        }
        None
    };

    let albedo = trace_texture("base_color");
    let normal = trace_texture("normal");
    let arm = trace_texture("metallic")
        .or_else(|| trace_texture("roughness"))
        .or_else(|| trace_texture("ao"));

    Ok((albedo, normal, arm))
}
