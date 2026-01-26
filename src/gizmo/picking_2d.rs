//! 2D picking system for the editor
//!
//! Handles selection of 2D entities in the viewport using bounding box checks.

#![allow(dead_code)]

use bevy::prelude::*;

use crate::core::{EditorEntity, ViewportMode, ViewportState};
use crate::viewport::Camera2DState;

/// Convert screen coordinates to 2D world coordinates
pub fn screen_to_world_2d(
    screen_pos: Vec2,
    viewport: &ViewportState,
    camera2d_state: &Camera2DState,
) -> Vec2 {
    // Calculate position relative to viewport center
    let viewport_center = Vec2::new(
        viewport.position[0] + viewport.size[0] / 2.0,
        viewport.position[1] + viewport.size[1] / 2.0,
    );

    let relative_pos = screen_pos - viewport_center;

    // Apply inverse zoom and pan
    let world_x = relative_pos.x / camera2d_state.zoom + camera2d_state.pan_offset.x;
    let world_y = -relative_pos.y / camera2d_state.zoom + camera2d_state.pan_offset.y; // Y is inverted

    Vec2::new(world_x, world_y)
}

/// Convert 2D world coordinates to screen coordinates
pub fn world_to_screen_2d(
    world_pos: Vec2,
    viewport: &ViewportState,
    camera2d_state: &Camera2DState,
) -> Vec2 {
    let viewport_center = Vec2::new(
        viewport.position[0] + viewport.size[0] / 2.0,
        viewport.position[1] + viewport.size[1] / 2.0,
    );

    let relative_world = world_pos - camera2d_state.pan_offset;
    let screen_x = relative_world.x * camera2d_state.zoom + viewport_center.x;
    let screen_y = -relative_world.y * camera2d_state.zoom + viewport_center.y; // Y is inverted

    Vec2::new(screen_x, screen_y)
}

/// Check if a point is inside a 2D entity's bounding box
pub fn point_in_bounds_2d(
    point: Vec2,
    entity_pos: Vec2,
    entity_size: Vec2,
    entity_scale: Vec2,
) -> bool {
    let half_size = entity_size * entity_scale * 0.5;
    let min = entity_pos - half_size;
    let max = entity_pos + half_size;

    point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
}

/// Default size for entities without explicit size (like empty nodes)
const DEFAULT_ENTITY_SIZE: f32 = 50.0;

/// Pick a 2D entity at the given screen position
pub fn pick_2d_entity(
    screen_pos: Vec2,
    viewport: &ViewportState,
    camera2d_state: &Camera2DState,
    entities: &Query<(Entity, &Transform, &EditorEntity)>,
) -> Option<Entity> {
    // Only pick in 2D mode
    if viewport.viewport_mode != ViewportMode::Mode2D {
        return None;
    }

    let world_pos = screen_to_world_2d(screen_pos, viewport, camera2d_state);

    let mut best_entity: Option<Entity> = None;
    let mut best_distance = f32::MAX;

    for (entity, transform, editor_entity) in entities.iter() {
        // Skip locked entities
        if editor_entity.locked {
            continue;
        }

        let entity_pos = Vec2::new(transform.translation.x, transform.translation.y);
        let entity_scale = Vec2::new(transform.scale.x, transform.scale.y);

        // Use default size for now - in real implementation, this would come from
        // the sprite/UI component bounds
        let entity_size = Vec2::splat(DEFAULT_ENTITY_SIZE);

        if point_in_bounds_2d(world_pos, entity_pos, entity_size, entity_scale) {
            // Calculate distance to center for priority (closer to center = better match)
            let dist = (world_pos - entity_pos).length();
            if dist < best_distance {
                best_distance = dist;
                best_entity = Some(entity);
            }
        }
    }

    best_entity
}

/// System for handling 2D picking with mouse click
pub fn handle_2d_picking(
    viewport: Res<ViewportState>,
    camera2d_state: Res<Camera2DState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    entities: Query<(Entity, &Transform, &EditorEntity)>,
    mut selection: ResMut<crate::core::SelectionState>,
    gizmo_state: Res<crate::gizmo::GizmoState>,
) {
    // Only in 2D mode
    if viewport.viewport_mode != ViewportMode::Mode2D {
        return;
    }

    // Don't pick while dragging a gizmo
    if gizmo_state.is_dragging {
        return;
    }

    // Only on click
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    // Must be hovering viewport
    if !viewport.hovered {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    // Check if clicking on a gizmo first (handled by gizmo system)
    // This picking runs after gizmo hover detection, so we check if gizmo was hovered
    if gizmo_state.hovered_axis.is_some() {
        return;
    }

    // Pick entity
    if let Some(entity) = pick_2d_entity(cursor_pos, &viewport, &camera2d_state, &entities) {
        selection.selected_entity = Some(entity);
    } else {
        // Clicked on empty space - deselect
        selection.selected_entity = None;
    }
}
