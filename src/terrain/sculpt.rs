//! Terrain sculpting tools

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::core::{InputFocusState, ViewportCamera, ViewportState};
use crate::gizmo::{EditorTool, GizmoState};

use super::{
    TerrainBrushType, TerrainChunkData, TerrainChunkOf, TerrainData,
    TerrainSculptState, TerrainSettings,
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
    let chunk_x = (local_x / terrain.chunk_size).floor() as u32;
    let chunk_z = (local_z / terrain.chunk_size).floor() as u32;

    if chunk_x >= terrain.chunks_x || chunk_z >= terrain.chunks_z {
        return None;
    }

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

        // Get vertex coordinates
        let spacing = terrain.vertex_spacing();
        let vx = (in_chunk_x / spacing).floor() as u32;
        let vz = (in_chunk_z / spacing).floor() as u32;

        let vx = vx.min(terrain.chunk_resolution - 1);
        let vz = vz.min(terrain.chunk_resolution - 1);

        // Get height (could interpolate for smoother results)
        let height_normalized = chunk_data.get_height(vx, vz, terrain.chunk_resolution);
        let height_range = terrain.max_height - terrain.min_height;
        return Some(terrain.min_height + height_normalized * height_range);
    }

    None
}

/// System to apply sculpting operations
pub fn terrain_sculpt_system(
    gizmo_state: Res<GizmoState>,
    sculpt_state: Res<TerrainSculptState>,
    settings: Res<TerrainSettings>,
    time: Res<Time>,
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
            TerrainBrushType::Raise => Color::srgba(0.2, 0.8, 0.2, 0.5),
            TerrainBrushType::Lower => Color::srgba(0.8, 0.2, 0.2, 0.5),
            TerrainBrushType::Smooth => Color::srgba(0.2, 0.5, 0.8, 0.5),
            TerrainBrushType::Flatten => Color::srgba(0.8, 0.8, 0.2, 0.5),
            TerrainBrushType::SetHeight => Color::srgba(0.8, 0.4, 0.8, 0.5),
        };

        // Draw brush circle
        gizmos.circle(
            Isometry3d::new(hover_pos + Vec3::Y * 0.1, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            settings.brush_radius,
            color,
        );
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
                let dist = (dx * dx + dz * dz).sqrt();

                if dist > brush_radius {
                    continue;
                }

                // Calculate falloff
                let falloff = if settings.falloff > 0.5 {
                    // Smooth falloff (cosine)
                    let t = dist / brush_radius;
                    (1.0 - t * t).max(0.0)
                } else {
                    // Linear falloff
                    1.0 - dist / brush_radius
                };

                let effect = strength * falloff;

                match settings.brush_type {
                    TerrainBrushType::Raise => {
                        let delta = effect / height_range;
                        chunk_data.modify_height(vx, vz, resolution, delta);
                    }
                    TerrainBrushType::Lower => {
                        let delta = -effect / height_range;
                        chunk_data.modify_height(vx, vz, resolution, delta);
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
                            let new_height = current + (target - current) * effect * 2.0;
                            chunk_data.set_height(vx, vz, resolution, new_height);
                        }
                    }
                    TerrainBrushType::SetHeight => {
                        let current = chunk_data.get_height(vx, vz, resolution);
                        let target = settings.target_height;
                        let new_height = current + (target - current) * effect * 2.0;
                        chunk_data.set_height(vx, vz, resolution, new_height);
                    }
                }
            }
        }
    }
}
