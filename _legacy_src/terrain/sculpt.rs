//! Terrain sculpting tools

use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;
use bevy::window::{PrimaryWindow, CursorOptions};

use crate::core::{InputFocusState, SelectionState, ViewportCamera, ViewportState};
use crate::gizmo::{EditorTool, GizmoState};

use super::{
    BrushFalloffType, BrushShape, FlattenMode, TerrainBrushType, TerrainChunkData, TerrainChunkOf,
    TerrainData, TerrainSculptState, TerrainSettings,
};
use crate::mesh_sculpt::MeshSculptState;

// ============================================================================
// Noise Utilities
// ============================================================================

/// Integer hash for value noise
#[inline]
fn hash_u32(x: u32) -> u32 {
    let mut h = x.wrapping_mul(2747636419u32);
    h ^= h >> 16;
    h = h.wrapping_mul(2246822519u32);
    h ^= h >> 13;
    h = h.wrapping_mul(3266489917u32);
    h ^= h >> 16;
    h
}

/// 2D hash returning a value in [0, 1]
#[inline]
fn hash2d(x: i32, y: i32, seed: u32) -> f32 {
    let hx = hash_u32(x as u32 ^ seed);
    let hy = hash_u32(y as u32 ^ seed.wrapping_add(0xDEAD_BEEF));
    let h = hash_u32(hx.wrapping_add(hy));
    (h as f32) / (u32::MAX as f32)
}

/// Value noise (smooth lattice noise) at (x, y)
#[inline]
fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x.fract();
    let fy = y.fract();
    // Smoothstep interpolation
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);

    let a = hash2d(ix,     iy,     seed);
    let b = hash2d(ix + 1, iy,     seed);
    let c = hash2d(ix,     iy + 1, seed);
    let d = hash2d(ix + 1, iy + 1, seed);

    let h0 = a + (b - a) * ux;
    let h1 = c + (d - c) * ux;
    h0 + (h1 - h0) * uy
}

/// Fractal Brownian Motion: layered value noise.
/// Returns a value roughly in [0, 1].
fn fbm(x: f32, y: f32, octaves: u32, lacunarity: f32, persistence: f32, seed: u32) -> f32 {
    let mut value = 0.0f32;
    let mut amplitude = 1.0f32;
    let mut frequency = 1.0f32;
    let mut max_amp = 0.0f32;

    for i in 0..octaves {
        let oct_seed = seed.wrapping_add(i.wrapping_mul(12_345));
        value += value_noise(x * frequency, y * frequency, oct_seed) * amplitude;
        max_amp += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    if max_amp > 0.0 { value / max_amp } else { 0.0 }
}

/// System to handle T key shortcut for terrain sculpt tool
pub fn terrain_tool_shortcut_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut gizmo_state: ResMut<GizmoState>,
    input_focus: Res<InputFocusState>,
) {
    // Don't switch tools if egui has focus
    if input_focus.egui_wants_keyboard {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyT) {
        gizmo_state.tool = EditorTool::TerrainSculpt;
    }
}

/// Keeps gizmo.terrain_selected in sync with the actual selection every frame.
/// This ensures the toolbar expands/collapses regardless of how selection changed
/// (viewport click, hierarchy panel, keyboard shortcut, etc.).
pub fn terrain_selection_sync_system(
    selection: Res<SelectionState>,
    mut gizmo: ResMut<GizmoState>,
    terrain_entities: Query<(), Or<(With<TerrainChunkData>, With<TerrainData>)>>,
) {
    let is_terrain = selection.selected_entity
        .map(|e| terrain_entities.contains(e))
        .unwrap_or(false);

    if gizmo.terrain_selected != is_terrain {
        gizmo.terrain_selected = is_terrain;
        if !is_terrain && gizmo.tool == EditorTool::TerrainSculpt {
            gizmo.tool = EditorTool::Select;
        }
    }
}

/// System to adjust terrain brush size with scroll wheel
pub fn terrain_brush_scroll_system(
    gizmo_state: Res<GizmoState>,
    viewport: Res<ViewportState>,
    mut settings: ResMut<TerrainSettings>,
    mut scroll_events: MessageReader<MouseWheel>,
) {
    if gizmo_state.tool != EditorTool::TerrainSculpt || !viewport.hovered {
        return;
    }

    for ev in scroll_events.read() {
        // Scale adjustment relative to current size for smooth feel
        let factor = if ev.y > 0.0 { 1.1 } else { 0.9 };
        settings.brush_radius = (settings.brush_radius * factor).clamp(1.0, 200.0);
    }
}

/// System to detect hover position on terrain
pub fn terrain_sculpt_hover_system(
    gizmo_state: Res<GizmoState>,
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
    terrain_query: Query<(Entity, &TerrainData, &GlobalTransform)>,
    chunk_query: Query<(&TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    mut sculpt_state: ResMut<TerrainSculptState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mesh_sculpt_state: Option<Res<MeshSculptState>>,
) {
    // Only process in terrain sculpt mode
    if gizmo_state.tool != EditorTool::TerrainSculpt {
        sculpt_state.hover_position = None;
        sculpt_state.active_terrain = None;
        sculpt_state.brush_visible = false;
        sculpt_state.cursor_hidden_by_us = false;
        return;
    }

    // If mesh sculpt is actively hovering a mesh, skip terrain hover
    if let Some(ref ms) = mesh_sculpt_state {
        if ms.hover_position.is_some() {
            sculpt_state.hover_position = None;
            sculpt_state.active_terrain = None;
            return;
        }
    }

    let Ok(window) = window_query.single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        sculpt_state.hover_position = None;
        return;
    };

    // Check if cursor is in viewport using ViewportState's contains_point method
    if !viewport.contains_point(cursor_pos.x, cursor_pos.y) {
        sculpt_state.hover_position = None;
        return;
    }

    // Find the editor camera (marked with ViewportCamera)
    let Some((camera, camera_transform)) = camera_query.iter().next() else {
        sculpt_state.hover_position = None;
        return;
    };

    // Convert cursor position to viewport-local coordinates
    let viewport_pos = Vec2::new(
        cursor_pos.x - viewport.position[0],
        cursor_pos.y - viewport.position[1],
    );

    // Get ray from camera
    let Ok(ray) = camera.viewport_to_world(camera_transform, viewport_pos) else {
        sculpt_state.hover_position = None;
        return;
    };

    // Raycast against terrain chunks
    let mut closest_hit: Option<(Vec3, Entity, f32)> = None;

    for (terrain_entity, terrain_data, terrain_transform) in terrain_query.iter() {
        // Simple raycast: intersect with horizontal planes at various heights
        // For a proper implementation, you'd want to do triangle intersection
        // This approximation works reasonably well for sculpting

        let terrain_pos = terrain_transform.translation();
        let half_width = terrain_data.total_width() / 2.0;
        let half_depth = terrain_data.total_depth() / 2.0;

        // Check intersection with terrain bounds at y=0 plane first
        let plane_y = terrain_pos.y;
        if ray.direction.y.abs() > 0.001 {
            let t = (plane_y - ray.origin.y) / ray.direction.y;
            if t > 0.0 {
                let hit_point = ray.origin + ray.direction * t;
                let local_x = hit_point.x - terrain_pos.x;
                let local_z = hit_point.z - terrain_pos.z;

                // Check if within terrain bounds
                if local_x >= -half_width
                    && local_x <= half_width
                    && local_z >= -half_depth
                    && local_z <= half_depth
                {
                    // Now find the actual height at this position
                    if let Some(height) = get_terrain_height_at(
                        local_x + half_width,
                        local_z + half_depth,
                        terrain_data,
                        &chunk_query,
                        terrain_entity,
                    ) {
                        let actual_hit = Vec3::new(hit_point.x, height + terrain_pos.y, hit_point.z);
                        let dist = (actual_hit - ray.origin).length();

                        if closest_hit.is_none() || dist < closest_hit.as_ref().unwrap().2 {
                            closest_hit = Some((actual_hit, terrain_entity, dist));
                        }
                    }
                }
            }
        }
    }

    if let Some((hit_pos, terrain_entity, _)) = closest_hit {
        sculpt_state.hover_position = Some(hit_pos);
        sculpt_state.active_terrain = Some(terrain_entity);

        // Track sculpting state
        if mouse_button.just_pressed(MouseButton::Left) {
            sculpt_state.is_sculpting = true;

            // For flatten tool, capture the starting height
            if let Some((_terrain_entity, terrain_data, terrain_transform)) =
                terrain_query.iter().find(|(e, _, _)| *e == terrain_entity)
            {
                let local_y = hit_pos.y - terrain_transform.translation().y;
                let height_range = terrain_data.max_height - terrain_data.min_height;
                let normalized = (local_y - terrain_data.min_height) / height_range;
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

/// Get terrain height at a local position (in terrain-local coordinates)
fn get_terrain_height_at(
    local_x: f32,
    local_z: f32,
    terrain: &TerrainData,
    chunk_query: &Query<(&TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    terrain_entity: Entity,
) -> Option<f32> {
    // Determine which chunk this position falls in
    let chunk_x = (local_x / terrain.chunk_size).floor() as i32;
    let chunk_z = (local_z / terrain.chunk_size).floor() as i32;

    if chunk_x < 0 || chunk_z < 0 || chunk_x >= terrain.chunks_x as i32 || chunk_z >= terrain.chunks_z as i32 {
        return None;
    }

    let chunk_x = chunk_x as u32;
    let chunk_z = chunk_z as u32;

    // Find the chunk
    for (chunk_data, chunk_of, _) in chunk_query.iter() {
        if chunk_of.0 != terrain_entity {
            continue;
        }
        if chunk_data.chunk_x != chunk_x || chunk_data.chunk_z != chunk_z {
            continue;
        }

        // Get position within chunk
        let in_chunk_x = local_x - chunk_x as f32 * terrain.chunk_size;
        let in_chunk_z = local_z - chunk_z as f32 * terrain.chunk_size;

        // Get vertex coordinates for bilinear interpolation
        let spacing = terrain.vertex_spacing();
        let fx = in_chunk_x / spacing;
        let fz = in_chunk_z / spacing;

        let vx0 = (fx.floor() as u32).min(terrain.chunk_resolution - 1);
        let vz0 = (fz.floor() as u32).min(terrain.chunk_resolution - 1);
        let vx1 = (vx0 + 1).min(terrain.chunk_resolution - 1);
        let vz1 = (vz0 + 1).min(terrain.chunk_resolution - 1);

        // Fractional parts for interpolation
        let tx = fx - fx.floor();
        let tz = fz - fz.floor();

        // Sample four corners
        let h00 = chunk_data.get_height(vx0, vz0, terrain.chunk_resolution);
        let h10 = chunk_data.get_height(vx1, vz0, terrain.chunk_resolution);
        let h01 = chunk_data.get_height(vx0, vz1, terrain.chunk_resolution);
        let h11 = chunk_data.get_height(vx1, vz1, terrain.chunk_resolution);

        // Bilinear interpolation for smooth height sampling
        let h0 = h00 * (1.0 - tx) + h10 * tx;
        let h1 = h01 * (1.0 - tx) + h11 * tx;
        let height_normalized = h0 * (1.0 - tz) + h1 * tz;

        let height_range = terrain.max_height - terrain.min_height;
        return Some(terrain.min_height + height_normalized * height_range);
    }

    None
}

/// Sample terrain height at a world position for brush preview
fn sample_brush_height(
    world_x: f32,
    world_z: f32,
    terrain: &TerrainData,
    terrain_pos: Vec3,
    chunk_query: &Query<(&mut TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    terrain_entity: Entity,
) -> Option<f32> {
    let half_width = terrain.total_width() / 2.0;
    let half_depth = terrain.total_depth() / 2.0;

    // Convert to terrain-local coordinates
    let local_x = world_x - terrain_pos.x + half_width;
    let local_z = world_z - terrain_pos.z + half_depth;

    // Determine which chunk this position falls in
    let chunk_x = (local_x / terrain.chunk_size).floor() as i32;
    let chunk_z = (local_z / terrain.chunk_size).floor() as i32;

    if chunk_x < 0 || chunk_z < 0 || chunk_x >= terrain.chunks_x as i32 || chunk_z >= terrain.chunks_z as i32 {
        return None;
    }

    let chunk_x = chunk_x as u32;
    let chunk_z = chunk_z as u32;

    // Find the chunk
    for (chunk_data, chunk_of, _) in chunk_query.iter() {
        if chunk_of.0 != terrain_entity {
            continue;
        }
        if chunk_data.chunk_x != chunk_x || chunk_data.chunk_z != chunk_z {
            continue;
        }

        // Get position within chunk
        let in_chunk_x = local_x - chunk_x as f32 * terrain.chunk_size;
        let in_chunk_z = local_z - chunk_z as f32 * terrain.chunk_size;

        // Get vertex coordinates for bilinear interpolation
        let spacing = terrain.vertex_spacing();
        let fx = in_chunk_x / spacing;
        let fz = in_chunk_z / spacing;

        let vx0 = (fx.floor() as u32).min(terrain.chunk_resolution - 1);
        let vz0 = (fz.floor() as u32).min(terrain.chunk_resolution - 1);
        let vx1 = (vx0 + 1).min(terrain.chunk_resolution - 1);
        let vz1 = (vz0 + 1).min(terrain.chunk_resolution - 1);

        // Fractional parts for interpolation
        let tx = fx - fx.floor();
        let tz = fz - fz.floor();

        // Sample four corners
        let h00 = chunk_data.get_height(vx0, vz0, terrain.chunk_resolution);
        let h10 = chunk_data.get_height(vx1, vz0, terrain.chunk_resolution);
        let h01 = chunk_data.get_height(vx0, vz1, terrain.chunk_resolution);
        let h11 = chunk_data.get_height(vx1, vz1, terrain.chunk_resolution);

        // Bilinear interpolation for smooth height sampling
        let h0 = h00 * (1.0 - tx) + h10 * tx;
        let h1 = h01 * (1.0 - tx) + h11 * tx;
        let height_normalized = h0 * (1.0 - tz) + h1 * tz;

        let height_range = terrain.max_height - terrain.min_height;
        return Some(terrain.min_height + height_normalized * height_range + terrain_pos.y);
    }

    None
}

/// System to apply sculpting operations
pub fn terrain_sculpt_system(
    gizmo_state: Res<GizmoState>,
    mut sculpt_state: ResMut<TerrainSculptState>,
    settings: Res<TerrainSettings>,
    viewport: Res<ViewportState>,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    terrain_query: Query<(&TerrainData, &GlobalTransform)>,
    mut chunk_query: Query<(&mut TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    mut gizmos: Gizmos,
) {
    // Reset brush visibility flag at start
    sculpt_state.brush_visible = false;

    // Only process in terrain sculpt mode
    if gizmo_state.tool != EditorTool::TerrainSculpt {
        return;
    }

    // Only draw brush preview when viewport is hovered
    let should_draw_brush = viewport.hovered;

    // Draw brush preview only when viewport is hovered and we have a position
    if should_draw_brush && sculpt_state.hover_position.is_some() {
        // Mark brush as visible for UI cursor handling
        sculpt_state.brush_visible = true;
        let hover_pos = sculpt_state.hover_position.unwrap();
        let color = match settings.brush_type {
            TerrainBrushType::Raise    => Color::srgba(0.2, 0.8, 0.2, 0.9),
            TerrainBrushType::Lower    => Color::srgba(0.8, 0.4, 0.2, 0.9),
            TerrainBrushType::Sculpt   => Color::srgba(0.3, 0.7, 0.3, 0.9),
            TerrainBrushType::Erase    => Color::srgba(0.8, 0.2, 0.2, 0.9),
            TerrainBrushType::Smooth   => Color::srgba(0.2, 0.5, 0.9, 0.9),
            TerrainBrushType::Flatten  => Color::srgba(0.9, 0.9, 0.2, 0.9),
            TerrainBrushType::SetHeight=> Color::srgba(0.9, 0.7, 0.1, 0.9),
            TerrainBrushType::Ramp     => Color::srgba(0.9, 0.6, 0.2, 0.9),
            TerrainBrushType::Erosion  => Color::srgba(0.6, 0.35, 0.1, 0.9),
            TerrainBrushType::Hydro    => Color::srgba(0.1, 0.5, 0.9, 0.9),
            TerrainBrushType::Noise    => Color::srgba(0.7, 0.5, 0.8, 0.9),
            TerrainBrushType::Retop    => Color::srgba(0.4, 0.8, 0.4, 0.9),
            TerrainBrushType::Terrace  => Color::srgba(0.8, 0.7, 0.3, 0.9),
            TerrainBrushType::Pinch    => Color::srgba(0.9, 0.3, 0.7, 0.9),
            TerrainBrushType::Relax    => Color::srgba(0.3, 0.8, 0.8, 0.9),
            TerrainBrushType::Cliff    => Color::srgba(0.5, 0.4, 0.3, 0.9),
            TerrainBrushType::Visibility=> Color::srgba(0.8, 0.8, 0.8, 0.9),
            TerrainBrushType::Blueprint => Color::srgba(0.2, 0.2, 0.9, 0.9),
            TerrainBrushType::Mirror   => Color::srgba(0.8, 0.2, 0.8, 0.9),
            TerrainBrushType::Select   => Color::srgba(1.0, 0.8, 0.0, 0.9),
            TerrainBrushType::Copy     => Color::srgba(0.0, 0.8, 0.8, 0.9),
        };

        // Draw contoured brush circle that follows terrain
        if let Some(terrain_entity) = sculpt_state.active_terrain {
            if let Ok((terrain_data, terrain_transform)) = terrain_query.get(terrain_entity) {
                let terrain_pos = terrain_transform.translation();

                // Number of segments for the brush outline (more = smoother)
                let segments = 48;
                let brush_radius = settings.brush_radius;

                // Sample points around the circumference based on brush shape
                let mut points: Vec<Vec3> = Vec::with_capacity(segments);

                for i in 0..segments {
                    let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
                    let (sin_a, cos_a) = angle.sin_cos();

                    // Calculate offset based on brush shape
                    let (dx, dz) = match settings.brush_shape {
                        BrushShape::Circle => {
                            (cos_a * brush_radius, sin_a * brush_radius)
                        }
                        BrushShape::Square => {
                            // Parametric square
                            let t = angle / std::f32::consts::FRAC_PI_2;
                            let side = (t.floor() as i32) % 4;
                            let frac = t.fract();
                            match side {
                                0 => (brush_radius, (frac * 2.0 - 1.0) * brush_radius),
                                1 => ((1.0 - frac * 2.0) * brush_radius, brush_radius),
                                2 => (-brush_radius, (1.0 - frac * 2.0) * brush_radius),
                                _ => ((frac * 2.0 - 1.0) * brush_radius, -brush_radius),
                            }
                        }
                        BrushShape::Diamond => {
                            // Parametric diamond
                            let t = angle / std::f32::consts::FRAC_PI_2;
                            let side = (t.floor() as i32) % 4;
                            let frac = t.fract();
                            match side {
                                0 => ((1.0 - frac) * brush_radius, frac * brush_radius),
                                1 => (-frac * brush_radius, (1.0 - frac) * brush_radius),
                                2 => (-(1.0 - frac) * brush_radius, -frac * brush_radius),
                                _ => (frac * brush_radius, -(1.0 - frac) * brush_radius),
                            }
                        }
                    };

                    let world_x = hover_pos.x + dx;
                    let world_z = hover_pos.z + dz;

                    // Sample terrain height at this point
                    let height = sample_brush_height(
                        world_x,
                        world_z,
                        terrain_data,
                        terrain_pos,
                        &chunk_query,
                        terrain_entity,
                    )
                    .unwrap_or(hover_pos.y);

                    // Add small offset above terrain surface
                    points.push(Vec3::new(world_x, height + 0.15, world_z));
                }

                // Draw line segments connecting the points
                for i in 0..segments {
                    let next = (i + 1) % segments;
                    gizmos.line(points[i], points[next], color);
                }

                // Also draw inner falloff circle if falloff is less than 1.0
                if settings.falloff < 0.99 {
                    let inner_radius = brush_radius * (1.0 - settings.falloff);
                    let inner_color = color.with_alpha(0.4);

                    let mut inner_points: Vec<Vec3> = Vec::with_capacity(segments);

                    for i in 0..segments {
                        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
                        let (sin_a, cos_a) = angle.sin_cos();

                        let (dx, dz) = match settings.brush_shape {
                            BrushShape::Circle => {
                                (cos_a * inner_radius, sin_a * inner_radius)
                            }
                            BrushShape::Square => {
                                let t = angle / std::f32::consts::FRAC_PI_2;
                                let side = (t.floor() as i32) % 4;
                                let frac = t.fract();
                                match side {
                                    0 => (inner_radius, (frac * 2.0 - 1.0) * inner_radius),
                                    1 => ((1.0 - frac * 2.0) * inner_radius, inner_radius),
                                    2 => (-inner_radius, (1.0 - frac * 2.0) * inner_radius),
                                    _ => ((frac * 2.0 - 1.0) * inner_radius, -inner_radius),
                                }
                            }
                            BrushShape::Diamond => {
                                let t = angle / std::f32::consts::FRAC_PI_2;
                                let side = (t.floor() as i32) % 4;
                                let frac = t.fract();
                                match side {
                                    0 => ((1.0 - frac) * inner_radius, frac * inner_radius),
                                    1 => (-frac * inner_radius, (1.0 - frac) * inner_radius),
                                    2 => (-(1.0 - frac) * inner_radius, -frac * inner_radius),
                                    _ => (frac * inner_radius, -(1.0 - frac) * inner_radius),
                                }
                            }
                        };

                        let world_x = hover_pos.x + dx;
                        let world_z = hover_pos.z + dz;

                        let height = sample_brush_height(
                            world_x,
                            world_z,
                            terrain_data,
                            terrain_pos,
                            &chunk_query,
                            terrain_entity,
                        )
                        .unwrap_or(hover_pos.y);

                        inner_points.push(Vec3::new(world_x, height + 0.15, world_z));
                    }

                    for i in 0..segments {
                        let next = (i + 1) % segments;
                        gizmos.line(inner_points[i], inner_points[next], inner_color);
                    }
                }
            }
        }
    }

    // Apply sculpting if active
    if !sculpt_state.is_sculpting {
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
    let half_width = terrain_data.total_width() / 2.0;
    let half_depth = terrain_data.total_depth() / 2.0;

    // Convert hover position to terrain-local coordinates
    let local_x = hover_pos.x - terrain_pos.x + half_width;
    let local_z = hover_pos.z - terrain_pos.z + half_depth;

    let brush_radius = settings.brush_radius;
    let strength = settings.brush_strength * time.delta_secs() * 2.0;
    let height_range = terrain_data.max_height - terrain_data.min_height;

    // Process each chunk that might be affected
    for (mut chunk_data, chunk_of, _) in chunk_query.iter_mut() {
        if chunk_of.0 != terrain_entity {
            continue;
        }

        let chunk_origin_x = chunk_data.chunk_x as f32 * terrain_data.chunk_size;
        let chunk_origin_z = chunk_data.chunk_z as f32 * terrain_data.chunk_size;
        let chunk_end_x = chunk_origin_x + terrain_data.chunk_size;
        let chunk_end_z = chunk_origin_z + terrain_data.chunk_size;

        // Quick bounds check - skip if brush doesn't overlap chunk
        if local_x + brush_radius < chunk_origin_x
            || local_x - brush_radius > chunk_end_x
            || local_z + brush_radius < chunk_origin_z
            || local_z - brush_radius > chunk_end_z
        {
            continue;
        }

        let spacing = terrain_data.vertex_spacing();
        let resolution = terrain_data.chunk_resolution;

        // Iterate over vertices in the chunk
        for vz in 0..resolution {
            for vx in 0..resolution {
                let vertex_world_x = chunk_origin_x + vx as f32 * spacing;
                let vertex_world_z = chunk_origin_z + vz as f32 * spacing;

                let dx = vertex_world_x - local_x;
                let dz = vertex_world_z - local_z;

                // Calculate distance based on brush shape
                let dist = match settings.brush_shape {
                    BrushShape::Circle => (dx * dx + dz * dz).sqrt(),
                    BrushShape::Square => dx.abs().max(dz.abs()),
                    BrushShape::Diamond => dx.abs() + dz.abs(),
                };

                if dist > brush_radius {
                    continue;
                }

                // Calculate normalized distance (0 at center, 1 at edge)
                let t = dist / brush_radius;

                // Inner radius where full strength applies (based on falloff setting)
                let inner_t = 1.0 - settings.falloff;

                // Calculate falloff based on falloff type
                let falloff = if t <= inner_t {
                    1.0
                } else {
                    let outer_t = (t - inner_t) / (1.0 - inner_t).max(0.001);
                    match settings.falloff_type {
                        BrushFalloffType::Smooth => {
                            // Smooth cosine falloff
                            (1.0 + (outer_t * std::f32::consts::PI).cos()) * 0.5
                        }
                        BrushFalloffType::Linear => {
                            // Linear falloff
                            1.0 - outer_t
                        }
                        BrushFalloffType::Spherical => {
                            // Spherical (hemisphere) falloff
                            (1.0 - outer_t * outer_t).sqrt().max(0.0)
                        }
                        BrushFalloffType::Tip => {
                            // Tip falloff - strong at center, quick dropoff
                            (1.0 - outer_t).powi(3)
                        }
                        BrushFalloffType::Flat => {
                            // Flat - uniform strength across entire brush
                            1.0
                        }
                    }
                };

                let effect = strength * falloff;

                // Check if Shift is held for inverse operations
                let shift_held = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

                match settings.brush_type {
                    TerrainBrushType::Raise => {
                        let delta = effect / height_range;
                        chunk_data.modify_height(vx, vz, resolution, delta);
                    }
                    TerrainBrushType::Lower => {
                        let delta = -effect / height_range;
                        chunk_data.modify_height(vx, vz, resolution, delta);
                    }
                    TerrainBrushType::Sculpt => {
                        // Raise (Shift = lower)
                        let delta = effect / height_range;
                        let delta = if shift_held { -delta } else { delta };
                        chunk_data.modify_height(vx, vz, resolution, delta);
                    }
                    TerrainBrushType::SetHeight => {
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let target = settings.target_height;
                        let new_height = current + (target - current) * (effect * 3.0).min(1.0);
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Erase => {
                        // Blend back towards flat mid-point
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let new_height = current + (0.2 - current) * (effect * 2.0).min(1.0);
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Smooth => {
                        // Gaussian-weighted 3×3 kernel for a natural softening effect
                        let current = chunk_data.get_height(vx, vz, resolution);
                        const KERNEL: &[(i32, i32, f32)] = &[
                            (-1,-1,0.0625), (0,-1,0.125), (1,-1,0.0625),
                            (-1, 0,0.125),  (0, 0,0.25),  (1, 0,0.125),
                            (-1, 1,0.0625), (0, 1,0.125), (1, 1,0.0625),
                        ];
                        let mut weighted = 0.0f32;
                        let mut total_w = 0.0f32;
                        for &(kx, kz, w) in KERNEL {
                            let nx = vx as i32 + kx;
                            let nz = vz as i32 + kz;
                            if nx >= 0 && nx < resolution as i32 && nz >= 0 && nz < resolution as i32 {
                                weighted += chunk_data.get_height(nx as u32, nz as u32, resolution) * w;
                                total_w += w;
                            }
                        }
                        let avg = weighted / total_w;
                        let new_height = current + (avg - current) * (effect * 2.0).min(1.0);
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Flatten => {
                        if let Some(target) = sculpt_state.flatten_start_height {
                            let current = chunk_data.get_height(vx, vz, resolution);
                            let should_apply = match settings.flatten_mode {
                                FlattenMode::Both  => true,
                                FlattenMode::Raise => current < target,
                                FlattenMode::Lower => current > target,
                            };
                            if should_apply {
                                let new_height = current + (target - current) * (effect * 2.0).min(1.0);
                                chunk_data.set_height(vx, vz, resolution, new_height);
                            }
                        }
                    }
                    TerrainBrushType::Noise => {
                        if shift_held {
                            // Shift: Gaussian smooth
                            let current = chunk_data.get_height(vx, vz, resolution);
                            let mut sum = 0.0;
                            let mut count = 0.0;
                            for nz in vz.saturating_sub(1)..=(vz + 1).min(resolution - 1) {
                                for nx in vx.saturating_sub(1)..=(vx + 1).min(resolution - 1) {
                                    sum += chunk_data.get_height(nx, nz, resolution);
                                    count += 1.0;
                                }
                            }
                            let avg = sum / count;
                            chunk_data.set_height(vx, vz, resolution, current + (avg - current) * effect);
                        } else {
                            // FBM value noise — much more natural than sin/cos
                            let scale = settings.noise_scale.max(0.1);
                            let n = fbm(
                                vertex_world_x / scale,
                                vertex_world_z / scale,
                                settings.noise_octaves.clamp(1, 8),
                                settings.noise_lacunarity,
                                settings.noise_persistence,
                                settings.noise_seed,
                            );
                            // Center around zero so noise adds AND removes height
                            let centered = n - 0.5;
                            let delta = effect * centered / height_range;
                            chunk_data.modify_height(vx, vz, resolution, delta);
                        }
                    }
                    TerrainBrushType::Erosion => {
                        // Thermal erosion: material slides down slopes steeper than talus angle.
                        // This produces realistic scree slopes and mountain ridges.
                        let current = chunk_data.get_height(vx, vz, resolution);
                        // Talus angle expressed in normalised height units per vertex spacing
                        let talus = 0.004;
                        let neighbors = [
                            (vx.wrapping_sub(1), vz),
                            (vx + 1,              vz),
                            (vx,  vz.wrapping_sub(1)),
                            (vx,  vz + 1),
                        ];
                        let mut total_excess = 0.0f32;
                        let mut steep_count = 0u32;
                        for (nx, nz) in neighbors {
                            if nx < resolution && nz < resolution {
                                let diff = current - chunk_data.get_height(nx, nz, resolution);
                                if diff > talus {
                                    total_excess += diff - talus;
                                    steep_count += 1;
                                }
                            }
                        }
                        if steep_count > 0 {
                            let erode = (total_excess / steep_count as f32) * effect * 0.6;
                            chunk_data.modify_height(vx, vz, resolution, -erode);
                        }
                    }
                    TerrainBrushType::Hydro => {
                        // Hydraulic erosion: water flows downhill carrying sediment, carving channels.
                        // High points surrounded by lower areas erode faster (positive drainage).
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let neighbors = [
                            (vx.wrapping_sub(1), vz),
                            (vx + 1,              vz),
                            (vx,  vz.wrapping_sub(1)),
                            (vx,  vz + 1),
                        ];
                        let mut max_drop = 0.0f32;
                        let mut drop_count = 0u32;
                        for (nx, nz) in neighbors {
                            if nx < resolution && nz < resolution {
                                let drop = current - chunk_data.get_height(nx, nz, resolution);
                                if drop > 0.001 {
                                    max_drop += drop;
                                    drop_count += 1;
                                }
                            }
                        }
                        if drop_count > 0 {
                            // Erode proportional to water flow (steeper = more erosion)
                            let sediment = (max_drop / drop_count as f32) * effect * 0.45;
                            chunk_data.modify_height(vx, vz, resolution, -sediment);
                        }
                    }
                    TerrainBrushType::Ramp => {
                        // Linear ramp from click height at brush edge to flat at center.
                        // Shift inverts the ramp direction.
                        let t = if shift_held {
                            dist / brush_radius
                        } else {
                            1.0 - dist / brush_radius
                        };
                        let target = sculpt_state.flatten_start_height.unwrap_or(0.5);
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let ramp_h = current + (target - current) * t;
                        let new_height = current + (ramp_h - current) * (effect * 2.0).min(1.0);
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Retop => {
                        // Wide-kernel aggressive smooth for retopologising noisy terrain.
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let mut sum = 0.0f32;
                        let mut count = 0.0f32;
                        for nz in vz.saturating_sub(2)..=(vz + 2).min(resolution - 1) {
                            for nx in vx.saturating_sub(2)..=(vx + 2).min(resolution - 1) {
                                sum += chunk_data.get_height(nx, nz, resolution);
                                count += 1.0;
                            }
                        }
                        let avg = sum / count;
                        let new_height = current + (avg - current) * (effect * 3.0).min(1.0);
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Terrace => {
                        // Quantise height into discrete steps, creating Gaea-style terraces.
                        // Sharpness controls how hard the step edges are.
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let steps = settings.terrace_steps.max(1) as f32;
                        let sharpness = settings.terrace_sharpness.clamp(0.0, 1.0);

                        let stepped = (current * steps).round() / steps;
                        // Blend between soft and hard step based on sharpness
                        let soft_target = stepped;
                        let blend = effect * (0.5 + sharpness * 1.5).min(1.0);
                        let new_height = current + (soft_target - current) * blend;
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Pinch => {
                        // Pinch/sharpen ridges: pushes vertices away from the local average,
                        // amplifying peaks and sharpening ridge lines.
                        // Shift inverts: acts as a gentle smooth towards average.
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let left  = if vx > 0              { chunk_data.get_height(vx - 1, vz,     resolution) } else { current };
                        let right = if vx < resolution - 1 { chunk_data.get_height(vx + 1, vz,     resolution) } else { current };
                        let up    = if vz < resolution - 1 { chunk_data.get_height(vx,     vz + 1, resolution) } else { current };
                        let down  = if vz > 0              { chunk_data.get_height(vx,     vz - 1, resolution) } else { current };
                        let avg = (left + right + up + down) * 0.25;
                        let deviation = current - avg;
                        let target = if shift_held {
                            // Smooth: pull towards average
                            current - deviation * effect
                        } else {
                            // Pinch: push away from average (amplify ridges)
                            current + deviation * effect * 0.5
                        };
                        chunk_data.set_height(vx, vz, resolution, target.clamp(0.0, 1.0));
                    }
                    TerrainBrushType::Relax => {
                        // Laplacian relaxation: redistributes height more evenly than box smooth.
                        // Ideal for removing small artefacts while preserving large features.
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let left  = if vx > 0              { chunk_data.get_height(vx - 1, vz,     resolution) } else { current };
                        let right = if vx < resolution - 1 { chunk_data.get_height(vx + 1, vz,     resolution) } else { current };
                        let up    = if vz < resolution - 1 { chunk_data.get_height(vx,     vz + 1, resolution) } else { current };
                        let down  = if vz > 0              { chunk_data.get_height(vx,     vz - 1, resolution) } else { current };
                        // Discrete Laplacian
                        let laplacian = (left + right + up + down) * 0.25 - current;
                        let new_height = current + laplacian * (effect * 2.5).min(1.0);
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Cliff => {
                        // Cliff: amplifies the local slope gradient, steepening gentle inclines
                        // into sharp cliff faces. Shift softens them back.
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let left  = if vx > 0              { chunk_data.get_height(vx - 1, vz,     resolution) } else { current };
                        let right = if vx < resolution - 1 { chunk_data.get_height(vx + 1, vz,     resolution) } else { current };
                        let up    = if vz < resolution - 1 { chunk_data.get_height(vx,     vz + 1, resolution) } else { current };
                        let down  = if vz > 0              { chunk_data.get_height(vx,     vz - 1, resolution) } else { current };
                        let dh_dx = (right - left) * 0.5;
                        let dh_dz = (up - down) * 0.5;
                        let slope = (dh_dx * dh_dx + dh_dz * dh_dz).sqrt();
                        if slope > 0.001 {
                            let delta = if shift_held {
                                -slope * effect * 0.4   // Soften cliffs
                            } else {
                                slope * effect * 0.4    // Steepen slopes
                            };
                            chunk_data.modify_height(vx, vz, resolution, delta);
                        }
                    }
                    TerrainBrushType::Visibility => {
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let delta = if shift_held { effect * 0.5 } else { -effect * 0.5 };
                        chunk_data.set_height(vx, vz, resolution, current + delta);
                    }
                    TerrainBrushType::Blueprint => {
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let new_height = current + (0.5 - current) * (effect * 2.0).min(1.0);
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Mirror => {
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let mirrored = 1.0 - current;
                        let new_height = current + (mirrored - current) * effect;
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Select => {
                        // Visual feedback only
                        let delta = effect * 0.005 / height_range;
                        chunk_data.modify_height(vx, vz, resolution, delta);
                    }
                    TerrainBrushType::Copy => {
                        if let Some(target) = sculpt_state.flatten_start_height {
                            let current = chunk_data.get_height(vx, vz, resolution);
                            chunk_data.set_height(vx, vz, resolution, current + (target - current) * effect);
                        }
                    }
                }
            }
        }
    }
}

/// System to hide/show cursor based on terrain brush visibility
pub fn terrain_brush_cursor_system(
    mut sculpt_state: ResMut<TerrainSculptState>,
    viewport: Res<ViewportState>,
    mut cursor_query: Query<&mut CursorOptions>,
    mesh_sculpt_state: Option<Res<MeshSculptState>>,
) {
    let Ok(mut cursor) = cursor_query.single_mut() else {
        return;
    };

    // Don't manage cursor if camera is dragging (that has its own cursor management)
    if viewport.camera_dragging {
        // If camera started dragging, we're no longer responsible for cursor state
        sculpt_state.cursor_hidden_by_us = false;
        return;
    }

    // Check if mesh sculpt brush is also visible
    let mesh_brush_visible = mesh_sculpt_state
        .as_ref()
        .is_some_and(|ms| ms.brush_visible);

    // Hide cursor when brush is visible and viewport is hovered
    if (sculpt_state.brush_visible || mesh_brush_visible) && viewport.hovered {
        cursor.visible = false;
        sculpt_state.cursor_hidden_by_us = true;
    } else if sculpt_state.cursor_hidden_by_us {
        // Only restore cursor if WE were the ones who hid it
        cursor.visible = true;
        sculpt_state.cursor_hidden_by_us = false;
    }
}
