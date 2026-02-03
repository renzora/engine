//! Terrain sculpting tools

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::core::{InputFocusState, ViewportCamera, ViewportState};
use crate::gizmo::{EditorTool, GizmoState};

use super::{
    BrushFalloffType, BrushShape, FlattenMode, TerrainBrushType, TerrainChunkData, TerrainChunkOf,
    TerrainData, TerrainSculptState, TerrainSettings,
};

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
) {
    // Only process in terrain sculpt mode
    if gizmo_state.tool != EditorTool::TerrainSculpt {
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

    // Check if cursor is in viewport using ViewportState's contains_point method
    if !viewport.contains_point(cursor_pos.x, cursor_pos.y) {
        sculpt_state.hover_position = None;
        return;
    }

    // Find the editor camera (marked with ViewportCamera)
    let Some((camera, camera_transform)) = camera_query.iter().next() else {
        return;
    };

    // Convert cursor position to viewport-local coordinates
    let viewport_pos = Vec2::new(
        cursor_pos.x - viewport.position[0],
        cursor_pos.y - viewport.position[1],
    );

    // Get ray from camera
    let Ok(ray) = camera.viewport_to_world(camera_transform, viewport_pos) else {
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
    sculpt_state: Res<TerrainSculptState>,
    settings: Res<TerrainSettings>,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    terrain_query: Query<(&TerrainData, &GlobalTransform)>,
    mut chunk_query: Query<(&mut TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    mut gizmos: Gizmos,
) {
    // Only process in terrain sculpt mode
    if gizmo_state.tool != EditorTool::TerrainSculpt {
        return;
    }

    // Draw brush preview
    if let Some(hover_pos) = sculpt_state.hover_position {
        let color = match settings.brush_type {
            TerrainBrushType::Raise => Color::srgba(0.2, 0.8, 0.2, 0.8),
            TerrainBrushType::Lower => Color::srgba(0.8, 0.4, 0.2, 0.8),
            TerrainBrushType::Sculpt => Color::srgba(0.2, 0.8, 0.2, 0.8),
            TerrainBrushType::Erase => Color::srgba(0.8, 0.2, 0.2, 0.8),
            TerrainBrushType::Smooth => Color::srgba(0.2, 0.5, 0.8, 0.8),
            TerrainBrushType::Flatten => Color::srgba(0.8, 0.8, 0.2, 0.8),
            TerrainBrushType::SetHeight => Color::srgba(0.8, 0.8, 0.2, 0.8),
            TerrainBrushType::Ramp => Color::srgba(0.8, 0.6, 0.2, 0.8),
            TerrainBrushType::Erosion => Color::srgba(0.6, 0.4, 0.2, 0.8),
            TerrainBrushType::Hydro => Color::srgba(0.2, 0.4, 0.8, 0.8),
            TerrainBrushType::Noise => Color::srgba(0.6, 0.6, 0.6, 0.8),
            TerrainBrushType::Retop => Color::srgba(0.4, 0.8, 0.4, 0.8),
            TerrainBrushType::Visibility => Color::srgba(0.8, 0.8, 0.8, 0.8),
            TerrainBrushType::Blueprint => Color::srgba(0.2, 0.2, 0.8, 0.8),
            TerrainBrushType::Mirror => Color::srgba(0.8, 0.2, 0.8, 0.8),
            TerrainBrushType::Select => Color::srgba(1.0, 0.8, 0.0, 0.8),
            TerrainBrushType::Copy => Color::srgba(0.0, 0.8, 0.8, 0.8),
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
                        // Raise terrain
                        let delta = effect / height_range;
                        chunk_data.modify_height(vx, vz, resolution, delta);
                    }
                    TerrainBrushType::Lower => {
                        // Lower terrain
                        let delta = -effect / height_range;
                        chunk_data.modify_height(vx, vz, resolution, delta);
                    }
                    TerrainBrushType::Sculpt => {
                        // Raise terrain (hold Shift to lower)
                        let delta = effect / height_range;
                        let delta = if shift_held { -delta } else { delta };
                        chunk_data.modify_height(vx, vz, resolution, delta);
                    }
                    TerrainBrushType::SetHeight => {
                        // Set terrain to exact target height
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let target = settings.target_height;
                        let new_height = current + (target - current) * effect * 2.0;
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Erase => {
                        // Reset to default height (0.5)
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let target = 0.5;
                        let new_height = current + (target - current) * effect * 2.0;
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Smooth => {
                        // Average with neighbors
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let mut sum = current;
                        let mut count = 1.0;

                        // Sample neighbors
                        for nz in vz.saturating_sub(1)..=(vz + 1).min(resolution - 1) {
                            for nx in vx.saturating_sub(1)..=(vx + 1).min(resolution - 1) {
                                if nx != vx || nz != vz {
                                    sum += chunk_data.get_height(nx, nz, resolution);
                                    count += 1.0;
                                }
                            }
                        }

                        let avg = sum / count;
                        let new_height = current + (avg - current) * effect * 2.0;
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Flatten => {
                        if let Some(target) = sculpt_state.flatten_start_height {
                            let current = chunk_data.get_height(vx, vz, resolution);
                            // Apply flatten mode constraints
                            let should_apply = match settings.flatten_mode {
                                FlattenMode::Both => true,
                                FlattenMode::Raise => current < target,
                                FlattenMode::Lower => current > target,
                            };
                            if should_apply {
                                let new_height = current + (target - current) * effect * 2.0;
                                chunk_data.set_height(vx, vz, resolution, new_height);
                            }
                        }
                    }
                    TerrainBrushType::Noise => {
                        // Add procedural noise (hold Shift to smooth instead)
                        if shift_held {
                            // Smooth when shift held
                            let current = chunk_data.get_height(vx, vz, resolution);
                            let mut sum = current;
                            let mut count = 1.0;
                            for nz in vz.saturating_sub(1)..=(vz + 1).min(resolution - 1) {
                                for nx in vx.saturating_sub(1)..=(vx + 1).min(resolution - 1) {
                                    if nx != vx || nz != vz {
                                        sum += chunk_data.get_height(nx, nz, resolution);
                                        count += 1.0;
                                    }
                                }
                            }
                            let avg = sum / count;
                            let new_height = current + (avg - current) * effect * 2.0;
                            chunk_data.set_height(vx, vz, resolution, new_height);
                        } else {
                            // Multi-octave noise for more natural results
                            let noise1 = ((vertex_world_x * 0.1).sin() * (vertex_world_z * 0.1).cos()) * 0.5;
                            let noise2 = ((vertex_world_x * 0.23).sin() * (vertex_world_z * 0.19).cos()) * 0.25;
                            let noise3 = ((vertex_world_x * 0.47).sin() * (vertex_world_z * 0.41).cos()) * 0.125;
                            let noise_val = noise1 + noise2 + noise3;
                            let delta = effect * noise_val / height_range;
                            chunk_data.modify_height(vx, vz, resolution, delta);
                        }
                    }
                    TerrainBrushType::Erosion => {
                        // Simple thermal erosion - move height towards neighbors
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let mut lowest = current;
                        for nz in vz.saturating_sub(1)..=(vz + 1).min(resolution - 1) {
                            for nx in vx.saturating_sub(1)..=(vx + 1).min(resolution - 1) {
                                let h = chunk_data.get_height(nx, nz, resolution);
                                if h < lowest {
                                    lowest = h;
                                }
                            }
                        }
                        if current > lowest {
                            let delta = (lowest - current) * effect * 0.5;
                            chunk_data.modify_height(vx, vz, resolution, delta);
                        }
                    }
                    TerrainBrushType::Hydro => {
                        // Simple hydraulic erosion simulation
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let mut sum = 0.0;
                        let mut count = 0.0;
                        for nz in vz.saturating_sub(1)..=(vz + 1).min(resolution - 1) {
                            for nx in vx.saturating_sub(1)..=(vx + 1).min(resolution - 1) {
                                let h = chunk_data.get_height(nx, nz, resolution);
                                if h < current {
                                    sum += h;
                                    count += 1.0;
                                }
                            }
                        }
                        if count > 0.0 {
                            let avg_lower = sum / count;
                            let delta = (avg_lower - current) * effect * 0.3;
                            chunk_data.modify_height(vx, vz, resolution, delta);
                        }
                    }
                    TerrainBrushType::Ramp => {
                        // Create a slope from brush edge towards center
                        // Height increases as you get closer to center (Shift to invert)
                        let t = if shift_held {
                            dist / brush_radius // Higher at edges
                        } else {
                            1.0 - (dist / brush_radius) // Higher at center
                        };
                        let target = sculpt_state.flatten_start_height.unwrap_or(0.5);
                        let current = chunk_data.get_height(vx, vz, resolution);
                        // Blend towards target height weighted by distance
                        let ramp_height = current + (target - current) * t;
                        let new_height = current + (ramp_height - current) * effect * 2.0;
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Retop => {
                        // Aggressive smoothing that normalizes the terrain more strongly
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let mut sum = 0.0;
                        let mut count = 0.0;
                        // Sample a wider neighborhood for stronger effect
                        for nz in vz.saturating_sub(2)..=(vz + 2).min(resolution - 1) {
                            for nx in vx.saturating_sub(2)..=(vx + 2).min(resolution - 1) {
                                sum += chunk_data.get_height(nx, nz, resolution);
                                count += 1.0;
                            }
                        }
                        let avg = sum / count;
                        // Stronger blend than regular smooth
                        let new_height = current + (avg - current) * effect * 3.0;
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Visibility => {
                        // Lower terrain below visible range (simulates hiding)
                        // Hold Shift to raise back up
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let delta = if shift_held { effect * 0.5 } else { -effect * 0.5 };
                        let new_height = current + delta;
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Blueprint => {
                        // Blueprint brush - set to flat reference height
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let target = 0.5; // Reference plane
                        let new_height = current + (target - current) * effect * 2.0;
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Mirror => {
                        // Mirror effect - invert height around midpoint
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let midpoint = 0.5;
                        let mirrored = midpoint + (midpoint - current);
                        let new_height = current + (mirrored - current) * effect;
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                    TerrainBrushType::Select => {
                        // Select mode - highlight by slight raise (visual feedback)
                        // In a full implementation this would mark vertices for batch operations
                        let delta = effect * 0.01 / height_range;
                        chunk_data.modify_height(vx, vz, resolution, delta);
                    }
                    TerrainBrushType::Copy => {
                        // Copy mode - sample and store height (first click), paste on subsequent
                        // Simplified: blend towards the flatten start height like stamp
                        if let Some(target) = sculpt_state.flatten_start_height {
                            let current = chunk_data.get_height(vx, vz, resolution);
                            let new_height = current + (target - current) * effect;
                            chunk_data.set_height(vx, vz, resolution, new_height);
                        }
                    }
                }
            }
        }
    }
}
