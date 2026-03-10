//! Terrain sculpting systems — hover detection, brush application, gizmo rendering.

use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;
use bevy::window::PrimaryWindow;

use renzora_core::EditorCamera;
use renzora_terrain::data::*;
use renzora_terrain::sculpt;
use renzora_viewport::ViewportState;

// ── Height sampling ──────────────────────────────────────────────────────────

/// Get terrain height at terrain-local coordinates (0..total_width, 0..total_depth).
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

/// Detect brush position on terrain via plane raycast.
pub fn terrain_sculpt_hover_system(
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    terrain_query: Query<(Entity, &TerrainData, &GlobalTransform)>,
    chunk_query: Query<(&TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    mut sculpt_state: ResMut<TerrainSculptState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    _settings: Res<TerrainSettings>,
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

    // Raycast against terrain bounding plane (y=0)
    let mut closest_hit: Option<(Vec3, Entity, f32)> = None;

    for (terrain_entity, terrain_data, terrain_transform) in terrain_query.iter() {
        let terrain_pos = terrain_transform.translation();
        let half_w = terrain_data.total_width() / 2.0;
        let half_d = terrain_data.total_depth() / 2.0;

        let plane_y = terrain_pos.y;
        if ray.direction.y.abs() < 0.001 {
            continue;
        }

        let t = (plane_y - ray.origin.y) / ray.direction.y;
        if t <= 0.0 {
            continue;
        }

        let hit = ray.origin + ray.direction * t;
        let lx = hit.x - terrain_pos.x;
        let lz = hit.z - terrain_pos.z;

        if lx < -half_w || lx > half_w || lz < -half_d || lz > half_d {
            continue;
        }

        // Get actual height at hit position
        if let Some(height) = get_terrain_height_at(
            lx + half_w,
            lz + half_d,
            terrain_data,
            &chunk_query,
            terrain_entity,
        ) {
            let actual_hit = Vec3::new(hit.x, height + terrain_pos.y, hit.z);
            let dist = (actual_hit - ray.origin).length();

            if closest_hit.is_none() || dist < closest_hit.as_ref().unwrap().2 {
                closest_hit = Some((actual_hit, terrain_entity, dist));
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
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    terrain_query: Query<(&TerrainData, &GlobalTransform)>,
    mut chunk_query: Query<(&mut TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    mut gizmos: Gizmos,
) {
    // Draw brush gizmo
    if sculpt_state.brush_visible {
        if let Some(hover_pos) = sculpt_state.hover_position {
            if let Some(terrain_entity) = sculpt_state.active_terrain {
                if let Ok((terrain_data, terrain_transform)) = terrain_query.get(terrain_entity) {
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

    // Apply sculpting
    if !sculpt_state.is_sculpting {
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
    }
}
