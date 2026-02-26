use bevy::prelude::*;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};

use crate::commands::{CommandHistory, SetTransformCommand, queue_command};
use crate::core::{EditorEntity, SceneNode, ViewportCamera, SelectionState, ViewportState, SceneManagerState, PlayModeState, PlayState, KeyBindings, EditorAction};
use crate::terrain::{TerrainChunkData, TerrainChunkOf, TerrainData};
use crate::console_info;

use super::modal_transform::ModalTransformState;
use super::picking::{
    get_cursor_ray, ray_box_intersection, ray_circle_intersection_point, ray_plane_intersection,
    ray_quad_intersection, ray_to_axis_closest_point, ray_to_axis_distance, ray_to_circle_distance,
};
use super::{DragAxis, EditorTool, GizmoMode, GizmoState, SnapSettings, SnapTarget, GIZMO_CENTER_SIZE, GIZMO_PICK_THRESHOLD, GIZMO_PLANE_OFFSET, GIZMO_PLANE_SIZE, GIZMO_SIZE, SCREEN_PICK_RADIUS};

pub fn gizmo_hover_system(
    mut gizmo: ResMut<GizmoState>,
    selection: Res<SelectionState>,
    viewport: Res<ViewportState>,
    modal: Res<ModalTransformState>,
    play_mode: Res<PlayModeState>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
    transforms: Query<&Transform, With<EditorEntity>>,
    terrain_chunks: Query<(), With<TerrainChunkData>>,
) {
    // Disable gizmo during modal transform or fullscreen play mode
    if modal.active || matches!(play_mode.state, PlayState::Playing | PlayState::Paused) {
        gizmo.hovered_axis = None;
        return;
    }

    // Don't update hover while dragging
    if gizmo.is_dragging {
        return;
    }

    gizmo.hovered_axis = None;

    // Only check gizmo hover in Transform tool mode, not in collider edit mode
    if gizmo.tool != EditorTool::Transform || gizmo.collider_edit.is_active() {
        return;
    }

    if !viewport.hovered {
        return;
    }

    let Some(selected) = selection.selected_entity else {
        return;
    };

    // Don't show gizmo for terrain chunks - they cannot be transformed
    if terrain_chunks.get(selected).is_ok() {
        return;
    }

    let Ok(obj_transform) = transforms.get(selected) else {
        return;
    };

    let Some(ray) = get_cursor_ray(&viewport, &windows, &camera_query) else {
        return;
    };

    let pos = obj_transform.translation;

    match gizmo.mode {
        GizmoMode::Translate => {
            // Check hits with priority: planes > axes > center
            // Check plane handles (small squares)
            let plane_center_xy = pos + Vec3::new(GIZMO_PLANE_OFFSET, GIZMO_PLANE_OFFSET, 0.0);
            let plane_center_xz = pos + Vec3::new(GIZMO_PLANE_OFFSET, 0.0, GIZMO_PLANE_OFFSET);
            let plane_center_yz = pos + Vec3::new(0.0, GIZMO_PLANE_OFFSET, GIZMO_PLANE_OFFSET);

            let xy_hit = ray_quad_intersection(&ray, plane_center_xy, Vec3::Z, GIZMO_PLANE_SIZE * 0.5);
            let xz_hit = ray_quad_intersection(&ray, plane_center_xz, Vec3::Y, GIZMO_PLANE_SIZE * 0.5);
            let yz_hit = ray_quad_intersection(&ray, plane_center_yz, Vec3::X, GIZMO_PLANE_SIZE * 0.5);

            // Find closest plane hit
            let mut best_plane: Option<(DragAxis, f32)> = None;
            if let Some(t) = xy_hit {
                best_plane = Some((DragAxis::XY, t));
            }
            if let Some(t) = xz_hit {
                if best_plane.is_none() || t < best_plane.unwrap().1 {
                    best_plane = Some((DragAxis::XZ, t));
                }
            }
            if let Some(t) = yz_hit {
                if best_plane.is_none() || t < best_plane.unwrap().1 {
                    best_plane = Some((DragAxis::YZ, t));
                }
            }

            if let Some((axis, _)) = best_plane {
                gizmo.hovered_axis = Some(axis);
                return;
            }

            // Check center cube
            if ray_box_intersection(&ray, pos, GIZMO_CENTER_SIZE).is_some() {
                gizmo.hovered_axis = Some(DragAxis::Free);
                return;
            }

            // Check distance to each axis
            let x_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::X * GIZMO_SIZE);
            let y_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::Y * GIZMO_SIZE);
            let z_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::Z * GIZMO_SIZE);

            let min_dist = x_dist.min(y_dist).min(z_dist);

            if min_dist < GIZMO_PICK_THRESHOLD {
                if (x_dist - min_dist).abs() < 0.001 {
                    gizmo.hovered_axis = Some(DragAxis::X);
                } else if (y_dist - min_dist).abs() < 0.001 {
                    gizmo.hovered_axis = Some(DragAxis::Y);
                } else {
                    gizmo.hovered_axis = Some(DragAxis::Z);
                }
            }
        }
        GizmoMode::Rotate => {
            // Check distance to each rotation circle
            let radius = GIZMO_SIZE * 0.7;
            let threshold = GIZMO_PICK_THRESHOLD * 1.5;

            let x_dist = ray_to_circle_distance(&ray, pos, Vec3::X, radius);
            let y_dist = ray_to_circle_distance(&ray, pos, Vec3::Y, radius);
            let z_dist = ray_to_circle_distance(&ray, pos, Vec3::Z, radius);

            let min_dist = x_dist.min(y_dist).min(z_dist);

            if min_dist < threshold {
                if (x_dist - min_dist).abs() < 0.001 {
                    gizmo.hovered_axis = Some(DragAxis::X);
                } else if (y_dist - min_dist).abs() < 0.001 {
                    gizmo.hovered_axis = Some(DragAxis::Y);
                } else {
                    gizmo.hovered_axis = Some(DragAxis::Z);
                }
            }
        }
        GizmoMode::Scale => {
            // Check distance to each axis (same as translate but without planes)
            let x_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::X * GIZMO_SIZE);
            let y_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::Y * GIZMO_SIZE);
            let z_dist = ray_to_axis_distance(&ray, pos, pos + Vec3::Z * GIZMO_SIZE);

            let min_dist = x_dist.min(y_dist).min(z_dist);

            if min_dist < GIZMO_PICK_THRESHOLD {
                if (x_dist - min_dist).abs() < 0.001 {
                    gizmo.hovered_axis = Some(DragAxis::X);
                } else if (y_dist - min_dist).abs() < 0.001 {
                    gizmo.hovered_axis = Some(DragAxis::Y);
                } else {
                    gizmo.hovered_axis = Some(DragAxis::Z);
                }
            }

            // Check center cube for uniform scale
            if ray_box_intersection(&ray, pos, GIZMO_CENTER_SIZE).is_some() {
                gizmo.hovered_axis = Some(DragAxis::Free);
            }
        }
    }
}

pub fn gizmo_interaction_system(
    mut gizmo: ResMut<GizmoState>,
    mut selection: ResMut<SelectionState>,
    viewport: Res<ViewportState>,
    modal: Res<ModalTransformState>,
    play_mode: Res<PlayModeState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
    editor_entities: Query<(Entity, &GlobalTransform, &EditorEntity)>,
    transforms: Query<&Transform, With<EditorEntity>>,
    parent_query: Query<&ChildOf>,
    mut mesh_ray_cast: MeshRayCast,
    mut command_history: ResMut<CommandHistory>,
    terrain_entities: Query<(), Or<(With<TerrainChunkData>, With<TerrainData>)>>,
) {
    // Disable gizmo interaction during modal transform or fullscreen play mode
    if modal.active || matches!(play_mode.state, PlayState::Playing | PlayState::Paused) {
        return;
    }

    let shift_held = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let ctrl_held = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);

    // Handle mouse release
    if mouse_button.just_released(MouseButton::Left) {
        // Handle gizmo drag end - create undo command for transform change
        if gizmo.is_dragging {
            if let (Some(entity), Some(start_transform)) = (gizmo.drag_entity, gizmo.drag_start_transform) {
                if let Ok(current_transform) = transforms.get(entity) {
                    // Only create command if transform actually changed
                    if *current_transform != start_transform {
                        let mut cmd = SetTransformCommand::new(entity, *current_transform);
                        cmd.old_transform = Some(start_transform);
                        queue_command(&mut command_history, Box::new(cmd));
                    }
                }
            }
            gizmo.is_dragging = false;
            gizmo.drag_axis = None;
            gizmo.drag_start_transform = None;
            gizmo.drag_entity = None;
            // Clear snap target state
            gizmo.snap_target = super::SnapTarget::None;
            gizmo.snap_target_position = None;
        }

        // Handle box selection end
        if gizmo.box_selection.active {
            if gizmo.box_selection.is_drag() {
                // Perform box selection
                let (min_x, min_y, max_x, max_y) = gizmo.box_selection.get_rect();

                // Convert viewport-relative coordinates
                let vp_x = viewport.position[0];
                let vp_y = viewport.position[1];

                // Find all entities within the box
                let mut entities_in_box = Vec::new();

                for (entity, global_transform, editor_entity) in editor_entities.iter() {
                    if editor_entity.locked {
                        continue;
                    }

                    // Project entity position to screen space
                    if let Ok((camera, cam_transform)) = camera_query.single() {
                        let world_pos = global_transform.translation();
                        if let Some(ndc) = camera.world_to_ndc(cam_transform, world_pos) {
                            // Convert NDC to screen coordinates
                            let screen_x = vp_x + (ndc.x + 1.0) * 0.5 * viewport.size[0];
                            let screen_y = vp_y + (1.0 - ndc.y) * 0.5 * viewport.size[1];

                            // Check if within selection box
                            if screen_x >= min_x && screen_x <= max_x &&
                               screen_y >= min_y && screen_y <= max_y &&
                               ndc.z > 0.0 && ndc.z < 1.0 {
                                entities_in_box.push(entity);
                            }
                        }
                    }
                }

                // Apply selection based on modifiers
                if !entities_in_box.is_empty() {
                    if shift_held {
                        // Add to existing selection
                        for entity in entities_in_box {
                            selection.add_to_selection(entity);
                        }
                    } else if ctrl_held {
                        // Toggle selection for entities in box
                        for entity in entities_in_box {
                            selection.toggle_selection(entity);
                        }
                    } else {
                        // Replace selection
                        selection.clear();
                        for entity in entities_in_box {
                            selection.add_to_selection(entity);
                        }
                    }
                } else if !shift_held && !ctrl_held {
                    // Clear selection if dragging empty area without modifiers
                    selection.clear();
                }
            } else if !shift_held && !ctrl_held {
                // Single click on empty space (not a drag) - clear selection
                selection.clear();
            }
            gizmo.box_selection.active = false;
        }
        return;
    }

    if !viewport.hovered {
        return;
    }

    // Update box selection position while dragging
    if gizmo.box_selection.active && mouse_button.pressed(MouseButton::Left) {
        if let Some(window) = windows.iter().next() {
            if let Some(cursor_pos) = window.cursor_position() {
                gizmo.box_selection.current_pos = [cursor_pos.x, cursor_pos.y];
            }
        }
        return;
    }

    // Check for keyboard-triggered select (picks entity under cursor without initiating drag/box select)
    let key_select = keybindings.just_pressed(EditorAction::SelectUnderCursor, &keyboard);

    // Only process clicks or keyboard select
    if !mouse_button.just_pressed(MouseButton::Left) && !key_select {
        return;
    }

    // Skip if camera is being dragged (prevents selection during camera movement)
    if viewport.camera_dragging {
        return;
    }

    // Skip if in collider edit mode - collider_edit_interaction_system handles clicks
    if gizmo.collider_edit.is_active() {
        return;
    }

    // If in Transform mode and hovering over a gizmo axis, start axis-constrained drag
    // (only for mouse clicks, not keyboard select)
    if !key_select && gizmo.tool == EditorTool::Transform {
        if let Some(axis) = gizmo.hovered_axis {
            // Calculate initial drag state based on gizmo mode
            if let Some(selected) = selection.selected_entity {
                // Don't allow gizmo drag on terrain chunks
                if terrain_entities.get(selected).is_ok() {
                    return;
                }
                if let Ok(obj_transform) = transforms.get(selected) {
                    if let Some(ray) = get_cursor_ray(&viewport, &windows, &camera_query) {
                        let pos = obj_transform.translation;
                        let cam_transform = camera_query.single().map(|(_, t)| t);

                        match gizmo.mode {
                            GizmoMode::Translate => {
                                // Calculate where cursor intersects the constraint
                                let drag_point = match axis {
                                    DragAxis::X => ray_to_axis_closest_point(&ray, pos, Vec3::X),
                                    DragAxis::Y => ray_to_axis_closest_point(&ray, pos, Vec3::Y),
                                    DragAxis::Z => ray_to_axis_closest_point(&ray, pos, Vec3::Z),
                                    DragAxis::XY => ray_plane_intersection(&ray, pos, Vec3::Z).unwrap_or(pos),
                                    DragAxis::XZ => ray_plane_intersection(&ray, pos, Vec3::Y).unwrap_or(pos),
                                    DragAxis::YZ => ray_plane_intersection(&ray, pos, Vec3::X).unwrap_or(pos),
                                    DragAxis::Free => {
                                        if let Ok(cam_t) = cam_transform {
                                            let cam_forward = cam_t.forward();
                                            ray_plane_intersection(&ray, pos, *cam_forward).unwrap_or(pos)
                                        } else {
                                            pos
                                        }
                                    }
                                };
                                gizmo.drag_start_offset = drag_point - pos;
                            }
                            GizmoMode::Rotate => {
                                // Store initial rotation and angle
                                gizmo.drag_start_rotation = obj_transform.rotation;
                                let axis_vec = match axis {
                                    DragAxis::X => Vec3::X,
                                    DragAxis::Y => Vec3::Y,
                                    DragAxis::Z => Vec3::Z,
                                    _ => Vec3::Y,
                                };
                                if let Some(hit_point) = ray_circle_intersection_point(&ray, pos, axis_vec) {
                                    let to_hit = (hit_point - pos).normalize();
                                    gizmo.drag_start_angle = to_hit.x.atan2(to_hit.z);
                                    gizmo.drag_start_offset = to_hit;
                                }
                            }
                            GizmoMode::Scale => {
                                // Store initial scale and distance from center
                                gizmo.drag_start_scale = obj_transform.scale;
                                let axis_vec = match axis {
                                    DragAxis::X => Vec3::X,
                                    DragAxis::Y => Vec3::Y,
                                    DragAxis::Z => Vec3::Z,
                                    DragAxis::Free => Vec3::ONE,
                                    _ => Vec3::ONE,
                                };
                                let drag_point = ray_to_axis_closest_point(&ray, pos, axis_vec);
                                gizmo.drag_start_distance = (drag_point - pos).length();
                                gizmo.drag_start_offset = drag_point - pos;
                            }
                        }

                        // Store original transform for undo
                        gizmo.drag_start_transform = Some(*obj_transform);
                        gizmo.drag_entity = Some(selected);
                    }
                }
            }

            gizmo.is_dragging = true;
            gizmo.drag_axis = Some(axis);
            return;
        }
    }

    // Do object picking using Bevy's mesh ray casting
    let Some(ray) = get_cursor_ray(&viewport, &windows, &camera_query) else {
        return;
    };

    // Cast ray against all meshes
    let hits = mesh_ray_cast.cast_ray(ray, &MeshRayCastSettings::default());

    // Find the closest hit that belongs to an EditorEntity (or is a descendant of one)
    let mut closest_entity: Option<Entity> = None;
    let mut closest_distance = f32::MAX;

    for (hit_entity, hit) in hits.iter() {
        // Find the EditorEntity ancestor of this mesh
        if let Some(editor_entity) = find_editor_entity_ancestor(*hit_entity, &editor_entities, &parent_query) {
            // Check if this EditorEntity is locked
            if let Ok((_, _, ee)) = editor_entities.get(editor_entity) {
                if ee.locked {
                    continue;
                }
            }

            if hit.distance < closest_distance {
                closest_distance = hit.distance;
                closest_entity = Some(editor_entity);
            }
        }
    }

    // Screen-space picking for entities without meshes (e.g. particle effects)
    // This runs always and competes with mesh raycast — closer-to-camera wins.
    if let Ok((camera, cam_transform)) = camera_query.single() {
        let vp_x = viewport.position[0];
        let vp_y = viewport.position[1];
        let cam_pos = cam_transform.translation();

        if let Some(window) = windows.iter().next() {
            if let Some(cursor_pos) = window.cursor_position() {
                let mut best_screen_dist = SCREEN_PICK_RADIUS;

                for (entity, global_transform, editor_entity) in editor_entities.iter() {
                    if editor_entity.locked {
                        continue;
                    }

                    let world_pos = global_transform.translation();
                    if let Some(ndc) = camera.world_to_ndc(cam_transform, world_pos) {
                        if ndc.z <= 0.0 || ndc.z >= 1.0 {
                            continue;
                        }

                        let screen_x = vp_x + (ndc.x + 1.0) * 0.5 * viewport.size[0];
                        let screen_y = vp_y + (1.0 - ndc.y) * 0.5 * viewport.size[1];

                        let dx = screen_x - cursor_pos.x;
                        let dy = screen_y - cursor_pos.y;
                        let screen_dist = (dx * dx + dy * dy).sqrt();

                        if screen_dist < best_screen_dist {
                            let cam_dist = (world_pos - cam_pos).length();
                            // Pick this entity if it's closer to camera than any mesh hit
                            if cam_dist < closest_distance {
                                best_screen_dist = screen_dist;
                                closest_distance = cam_dist;
                                closest_entity = Some(entity);
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(clicked) = closest_entity {
        console_info!("Selection", "Clicked on entity {:?} (dist: {:.2})", clicked, closest_distance);

        // In Transform mode, prevent selecting children to avoid accidental child selection while dragging
        // In Select mode, always allow selecting any entity
        let should_select = if gizmo.tool == EditorTool::Transform {
            if let Some(current) = selection.selected_entity {
                !is_descendant_of(clicked, current, &parent_query)
            } else {
                true
            }
        } else {
            true
        };

        if should_select {
            // Handle selection based on modifiers
            if ctrl_held {
                // Toggle selection
                selection.toggle_selection(clicked);
            } else if shift_held {
                // Add to selection
                selection.add_to_selection(clicked);
            } else {
                // Single select
                selection.select(clicked);
            }

        }
    } else {
        console_info!("Selection", "Clicked on empty space - clearing selection");

        // No entity clicked - start box selection only for mouse clicks (not keyboard select)
        if !key_select && gizmo.tool == EditorTool::Select {
            // Start box selection in Select mode
            if let Some(window) = windows.iter().next() {
                if let Some(cursor_pos) = window.cursor_position() {
                    gizmo.box_selection.active = true;
                    gizmo.box_selection.start_pos = [cursor_pos.x, cursor_pos.y];
                    gizmo.box_selection.current_pos = [cursor_pos.x, cursor_pos.y];
                }
            }
        }

        // Always clear selection when clicking empty space without modifiers (in any mode)
        if !shift_held && !ctrl_held && !gizmo.box_selection.active {
            selection.clear();
        }
    }
}

/// Find the EditorEntity ancestor of a mesh entity
/// Meshes are typically children (or descendants) of EditorEntity nodes
fn find_editor_entity_ancestor(
    entity: Entity,
    editor_entities: &Query<(Entity, &GlobalTransform, &EditorEntity)>,
    parent_query: &Query<&ChildOf>,
) -> Option<Entity> {
    // Check if this entity itself is an EditorEntity
    if editor_entities.get(entity).is_ok() {
        return Some(entity);
    }

    // Walk up the parent chain to find an EditorEntity
    let mut current = entity;
    while let Ok(child_of) = parent_query.get(current) {
        let parent = child_of.0;
        if editor_entities.get(parent).is_ok() {
            return Some(parent);
        }
        current = parent;
    }

    None
}

/// Check if an entity is a descendant of another entity
fn is_descendant_of(entity: Entity, ancestor: Entity, parents: &Query<&ChildOf>) -> bool {
    let mut current = entity;
    while let Ok(child_of) = parents.get(current) {
        if child_of.0 == ancestor {
            return true;
        }
        current = child_of.0;
    }
    false
}

pub fn object_drag_system(
    mut gizmo: ResMut<GizmoState>,
    selection: Res<SelectionState>,
    viewport: Res<ViewportState>,
    modal: Res<ModalTransformState>,
    play_mode: Res<PlayModeState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
    mut transforms: Query<&mut Transform, With<EditorEntity>>,
    editor_entities: Query<(Entity, &GlobalTransform), With<EditorEntity>>,
    mut scene_state: ResMut<SceneManagerState>,
    terrain_chunks: Query<(), With<TerrainChunkData>>,
) {
    // Disable gizmo drag during modal transform or fullscreen play mode
    if modal.active || matches!(play_mode.state, PlayState::Playing | PlayState::Paused) {
        return;
    }

    if !gizmo.is_dragging || !mouse_button.pressed(MouseButton::Left) {
        return;
    }

    // Mark scene as modified when dragging starts
    scene_state.mark_modified();

    let Some(selected) = selection.selected_entity else {
        return;
    };

    // Don't allow dragging terrain chunks
    if terrain_chunks.get(selected).is_ok() {
        return;
    }

    let Ok(mut obj_transform) = transforms.get_mut(selected) else {
        return;
    };

    let Some(ray) = get_cursor_ray(&viewport, &windows, &camera_query) else {
        return;
    };

    let Ok((_, cam_global)) = camera_query.single() else {
        return;
    };

    // Check if Ctrl is held for grid snapping
    let ctrl_held = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);

    // Create effective snap settings - enable grid snap when Ctrl is held
    let snap = if ctrl_held {
        SnapSettings {
            translate_enabled: true,
            translate_snap: gizmo.snap.translate_snap.max(1.0), // Use configured snap or default to 1.0
            rotate_enabled: true,
            rotate_snap: gizmo.snap.rotate_snap.max(15.0), // Use configured snap or default to 15 degrees
            scale_enabled: true,
            scale_snap: gizmo.snap.scale_snap.max(0.25), // Use configured snap or default to 0.25
            // Preserve object snap settings from gizmo state
            object_snap_enabled: gizmo.snap.object_snap_enabled,
            object_snap_distance: gizmo.snap.object_snap_distance,
            floor_snap_enabled: gizmo.snap.floor_snap_enabled,
            floor_y: gizmo.snap.floor_y,
        }
    } else {
        gizmo.snap
    };

    match gizmo.mode {
        GizmoMode::Translate => {
            let current_pos = obj_transform.translation;
            let offset = gizmo.drag_start_offset;

            let new_pos = match gizmo.drag_axis {
                Some(DragAxis::X) => {
                    let point = ray_to_axis_closest_point(&ray, current_pos, Vec3::X);
                    Vec3::new(point.x - offset.x, current_pos.y, current_pos.z)
                }
                Some(DragAxis::Y) => {
                    let point = ray_to_axis_closest_point(&ray, current_pos, Vec3::Y);
                    Vec3::new(current_pos.x, point.y - offset.y, current_pos.z)
                }
                Some(DragAxis::Z) => {
                    let point = ray_to_axis_closest_point(&ray, current_pos, Vec3::Z);
                    Vec3::new(current_pos.x, current_pos.y, point.z - offset.z)
                }
                Some(DragAxis::XY) => {
                    if let Some(point) = ray_plane_intersection(&ray, current_pos, Vec3::Z) {
                        Vec3::new(point.x - offset.x, point.y - offset.y, current_pos.z)
                    } else {
                        current_pos
                    }
                }
                Some(DragAxis::XZ) => {
                    if let Some(point) = ray_plane_intersection(&ray, current_pos, Vec3::Y) {
                        Vec3::new(point.x - offset.x, current_pos.y, point.z - offset.z)
                    } else {
                        current_pos
                    }
                }
                Some(DragAxis::YZ) => {
                    if let Some(point) = ray_plane_intersection(&ray, current_pos, Vec3::X) {
                        Vec3::new(current_pos.x, point.y - offset.y, point.z - offset.z)
                    } else {
                        current_pos
                    }
                }
                Some(DragAxis::Free) => {
                    let cam_forward = *cam_global.forward();
                    if let Some(point) = ray_plane_intersection(&ray, current_pos, cam_forward) {
                        point - offset
                    } else {
                        current_pos
                    }
                }
                None => current_pos,
            };

            // Apply grid snapping first (uses Ctrl-aware snap settings)
            let mut final_pos = snap.snap_translate_vec3(new_pos);

            // Clear previous snap target
            gizmo.snap_target = SnapTarget::None;
            gizmo.snap_target_position = None;

            // Apply object snapping if enabled
            if gizmo.snap.object_snap_enabled {
                let snap_distance = gizmo.snap.object_snap_distance;
                let mut closest_entity: Option<Entity> = None;
                let mut closest_distance = f32::MAX;
                let mut closest_pos = Vec3::ZERO;

                // Find the nearest object to snap to
                for (other_entity, other_global) in editor_entities.iter() {
                    // Skip self
                    if Some(other_entity) == gizmo.drag_entity {
                        continue;
                    }

                    let other_pos = other_global.translation();
                    let distance = (final_pos - other_pos).length();

                    if distance < snap_distance && distance < closest_distance {
                        closest_distance = distance;
                        closest_entity = Some(other_entity);
                        closest_pos = other_pos;
                    }
                }

                // Snap to nearest object if found
                if let Some(entity) = closest_entity {
                    // Snap to the object's position based on the current drag axis
                    match gizmo.drag_axis {
                        Some(DragAxis::X) => {
                            if (final_pos.x - closest_pos.x).abs() < snap_distance {
                                final_pos.x = closest_pos.x;
                                gizmo.snap_target = SnapTarget::Entity(entity);
                                gizmo.snap_target_position = Some(closest_pos);
                            }
                        }
                        Some(DragAxis::Y) => {
                            if (final_pos.y - closest_pos.y).abs() < snap_distance {
                                final_pos.y = closest_pos.y;
                                gizmo.snap_target = SnapTarget::Entity(entity);
                                gizmo.snap_target_position = Some(closest_pos);
                            }
                        }
                        Some(DragAxis::Z) => {
                            if (final_pos.z - closest_pos.z).abs() < snap_distance {
                                final_pos.z = closest_pos.z;
                                gizmo.snap_target = SnapTarget::Entity(entity);
                                gizmo.snap_target_position = Some(closest_pos);
                            }
                        }
                        Some(DragAxis::XY) => {
                            let mut snapped = false;
                            if (final_pos.x - closest_pos.x).abs() < snap_distance {
                                final_pos.x = closest_pos.x;
                                snapped = true;
                            }
                            if (final_pos.y - closest_pos.y).abs() < snap_distance {
                                final_pos.y = closest_pos.y;
                                snapped = true;
                            }
                            if snapped {
                                gizmo.snap_target = SnapTarget::Entity(entity);
                                gizmo.snap_target_position = Some(closest_pos);
                            }
                        }
                        Some(DragAxis::XZ) => {
                            let mut snapped = false;
                            if (final_pos.x - closest_pos.x).abs() < snap_distance {
                                final_pos.x = closest_pos.x;
                                snapped = true;
                            }
                            if (final_pos.z - closest_pos.z).abs() < snap_distance {
                                final_pos.z = closest_pos.z;
                                snapped = true;
                            }
                            if snapped {
                                gizmo.snap_target = SnapTarget::Entity(entity);
                                gizmo.snap_target_position = Some(closest_pos);
                            }
                        }
                        Some(DragAxis::YZ) => {
                            let mut snapped = false;
                            if (final_pos.y - closest_pos.y).abs() < snap_distance {
                                final_pos.y = closest_pos.y;
                                snapped = true;
                            }
                            if (final_pos.z - closest_pos.z).abs() < snap_distance {
                                final_pos.z = closest_pos.z;
                                snapped = true;
                            }
                            if snapped {
                                gizmo.snap_target = SnapTarget::Entity(entity);
                                gizmo.snap_target_position = Some(closest_pos);
                            }
                        }
                        Some(DragAxis::Free) | None => {
                            // Full 3D snap
                            final_pos = closest_pos;
                            gizmo.snap_target = SnapTarget::Entity(entity);
                            gizmo.snap_target_position = Some(closest_pos);
                        }
                    }
                }
            }

            // Apply floor snapping if enabled and no object snap occurred
            if gizmo.snap.floor_snap_enabled && gizmo.snap_target == SnapTarget::None {
                let floor_y = gizmo.snap.floor_y;
                let snap_distance = gizmo.snap.object_snap_distance;

                // Only snap Y to floor if we're moving on an axis that affects Y
                let affects_y = matches!(
                    gizmo.drag_axis,
                    Some(DragAxis::Y) | Some(DragAxis::XY) | Some(DragAxis::YZ) | Some(DragAxis::Free) | None
                );

                if affects_y && (final_pos.y - floor_y).abs() < snap_distance {
                    final_pos.y = floor_y;
                    gizmo.snap_target = SnapTarget::Floor;
                    gizmo.snap_target_position = Some(Vec3::new(final_pos.x, floor_y, final_pos.z));
                }
            }

            obj_transform.translation = final_pos;
        }
        GizmoMode::Rotate => {
            let pos = obj_transform.translation;
            let axis_vec = match gizmo.drag_axis {
                Some(DragAxis::X) => Vec3::X,
                Some(DragAxis::Y) => Vec3::Y,
                Some(DragAxis::Z) => Vec3::Z,
                _ => return,
            };

            if let Some(hit_point) = ray_circle_intersection_point(&ray, pos, axis_vec) {
                let to_hit = (hit_point - pos).normalize();
                let start_dir = gizmo.drag_start_offset;

                // Calculate angle between start direction and current direction
                let cross = start_dir.cross(to_hit);
                let dot = start_dir.dot(to_hit);
                let angle = cross.dot(axis_vec).atan2(dot);

                // Apply rotation snapping (uses Ctrl-aware snap settings)
                let snapped_angle = snap.snap_rotate(angle);

                // Apply rotation relative to start rotation
                let rotation_delta = Quat::from_axis_angle(axis_vec, snapped_angle);
                obj_transform.rotation = rotation_delta * gizmo.drag_start_rotation;
            }
        }
        GizmoMode::Scale => {
            let pos = obj_transform.translation;
            let start_scale = gizmo.drag_start_scale;
            let start_dist = gizmo.drag_start_distance.max(0.001);

            match gizmo.drag_axis {
                Some(DragAxis::X) => {
                    let point = ray_to_axis_closest_point(&ray, pos, Vec3::X);
                    let current_dist = (point - pos).dot(Vec3::X).abs();
                    let scale_factor = current_dist / start_dist;
                    let new_scale = (start_scale.x * scale_factor).max(0.01);
                    obj_transform.scale.x = snap.snap_scale(new_scale);
                }
                Some(DragAxis::Y) => {
                    let point = ray_to_axis_closest_point(&ray, pos, Vec3::Y);
                    let current_dist = (point - pos).dot(Vec3::Y).abs();
                    let scale_factor = current_dist / start_dist;
                    let new_scale = (start_scale.y * scale_factor).max(0.01);
                    obj_transform.scale.y = snap.snap_scale(new_scale);
                }
                Some(DragAxis::Z) => {
                    let point = ray_to_axis_closest_point(&ray, pos, Vec3::Z);
                    let current_dist = (point - pos).dot(Vec3::Z).abs();
                    let scale_factor = current_dist / start_dist;
                    let new_scale = (start_scale.z * scale_factor).max(0.01);
                    obj_transform.scale.z = snap.snap_scale(new_scale);
                }
                Some(DragAxis::Free) => {
                    // Uniform scale - use camera plane
                    let cam_forward = *cam_global.forward();
                    if let Some(point) = ray_plane_intersection(&ray, pos, cam_forward) {
                        let current_dist = (point - pos).length();
                        let scale_factor = current_dist / start_dist;
                        let new_scale = (start_scale * scale_factor).max(Vec3::splat(0.01));
                        obj_transform.scale = snap.snap_scale_vec3(new_scale);
                    }
                }
                _ => {}
            }
        }
    }
}

/// System to draw yellow border around selected terrain chunks or entire terrain
pub fn terrain_chunk_selection_system(
    selection: Res<SelectionState>,
    modal: Res<super::modal_transform::ModalTransformState>,
    play_mode: Res<PlayModeState>,
    gizmo_state: Res<super::GizmoState>,
    terrain_chunks: Query<(Entity, &TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    terrain_query: Query<(Entity, &crate::terrain::TerrainData, &GlobalTransform)>,
    mut gizmos: Gizmos<super::TerrainSelectionGizmoGroup>,
) {
    // Don't show during modal transform, collider edit mode, or fullscreen play mode
    if modal.active || gizmo_state.collider_edit.is_active() || matches!(play_mode.state, PlayState::Playing | PlayState::Paused) {
        return;
    }

    let all_selected = selection.get_all_selected();

    // Check if any terrain parent entity is selected - draw outer border for the whole terrain
    for (terrain_entity, terrain_data, terrain_transform) in terrain_query.iter() {
        if !all_selected.contains(&terrain_entity) {
            continue;
        }

        let is_primary = selection.selected_entity == Some(terrain_entity);
        let color = if is_primary {
            Color::srgb(1.0, 1.0, 0.0)
        } else {
            Color::srgba(1.0, 1.0, 0.0, 0.8)
        };

        // Collect chunks belonging to this terrain
        let chunks: Vec<_> = terrain_chunks
            .iter()
            .filter(|(_, _, chunk_of, _)| chunk_of.0 == terrain_entity)
            .collect();

        draw_terrain_outer_border(
            &mut gizmos,
            terrain_data,
            terrain_transform,
            &chunks,
            color,
        );
    }

    // Draw per-chunk borders for individually selected chunks
    for (entity, chunk_data, chunk_of, global_transform) in terrain_chunks.iter() {
        if !all_selected.contains(&entity) {
            continue;
        }

        // Skip if the parent terrain is already selected (we drew the full border above)
        if all_selected.contains(&chunk_of.0) {
            continue;
        }

        let Ok((_, terrain_data, _)) = terrain_query.get(chunk_of.0) else {
            continue;
        };

        let is_primary = selection.selected_entity == Some(entity);
        let color = if is_primary {
            Color::srgb(1.0, 1.0, 0.0)
        } else {
            Color::srgba(1.0, 1.0, 0.0, 0.8)
        };

        draw_terrain_chunk_border(
            &mut gizmos,
            chunk_data,
            terrain_data,
            global_transform,
            color,
        );
    }
}

/// Draw the outer border of the entire terrain (only exterior edges of boundary chunks)
fn draw_terrain_outer_border(
    gizmos: &mut Gizmos<super::TerrainSelectionGizmoGroup>,
    terrain_data: &crate::terrain::TerrainData,
    terrain_transform: &GlobalTransform,
    chunks: &[(Entity, &TerrainChunkData, &TerrainChunkOf, &GlobalTransform)],
    color: Color,
) {
    let resolution = terrain_data.chunk_resolution;
    let spacing = terrain_data.vertex_spacing();
    let height_range = terrain_data.max_height - terrain_data.min_height;
    let min_height = terrain_data.min_height;
    let y_offset = 0.15;

    // Helper: get world position for a vertex within a chunk
    let get_vertex_pos = |chunk: &TerrainChunkData, chunk_transform: &GlobalTransform, vx: u32, vz: u32| -> Vec3 {
        let height_normalized = chunk.get_height(vx, vz, resolution);
        let height = min_height + height_normalized * height_range;
        let pos = chunk_transform.translation();
        Vec3::new(
            pos.x + vx as f32 * spacing,
            pos.y + height + y_offset,
            pos.z + vz as f32 * spacing,
        )
    };

    for &(_, chunk_data, _, chunk_transform) in chunks {
        let cx = chunk_data.chunk_x;
        let cz = chunk_data.chunk_z;

        // Front edge of terrain (chunk_z == 0, draw z=0 edge)
        if cz == 0 {
            for vx in 0..(resolution - 1) {
                let p1 = get_vertex_pos(chunk_data, chunk_transform, vx, 0);
                let p2 = get_vertex_pos(chunk_data, chunk_transform, vx + 1, 0);
                gizmos.line(p1, p2, color);
            }
        }

        // Back edge of terrain (chunk_z == chunks_z-1, draw z=max edge)
        if cz == terrain_data.chunks_z - 1 {
            for vx in 0..(resolution - 1) {
                let p1 = get_vertex_pos(chunk_data, chunk_transform, vx, resolution - 1);
                let p2 = get_vertex_pos(chunk_data, chunk_transform, vx + 1, resolution - 1);
                gizmos.line(p1, p2, color);
            }
        }

        // Left edge of terrain (chunk_x == 0, draw x=0 edge)
        if cx == 0 {
            for vz in 0..(resolution - 1) {
                let p1 = get_vertex_pos(chunk_data, chunk_transform, 0, vz);
                let p2 = get_vertex_pos(chunk_data, chunk_transform, 0, vz + 1);
                gizmos.line(p1, p2, color);
            }
        }

        // Right edge of terrain (chunk_x == chunks_x-1, draw x=max edge)
        if cx == terrain_data.chunks_x - 1 {
            for vz in 0..(resolution - 1) {
                let p1 = get_vertex_pos(chunk_data, chunk_transform, resolution - 1, vz);
                let p2 = get_vertex_pos(chunk_data, chunk_transform, resolution - 1, vz + 1);
                gizmos.line(p1, p2, color);
            }
        }
    }
}

/// Helper to draw a border around a terrain chunk
fn draw_terrain_chunk_border(
    gizmos: &mut Gizmos<super::TerrainSelectionGizmoGroup>,
    chunk_data: &TerrainChunkData,
    terrain_data: &crate::terrain::TerrainData,
    global_transform: &GlobalTransform,
    color: Color,
) {
    let resolution = terrain_data.chunk_resolution;
    let spacing = terrain_data.vertex_spacing();
    let height_range = terrain_data.max_height - terrain_data.min_height;
    let min_height = terrain_data.min_height;
    let pos = global_transform.translation();

    // Small offset above terrain to prevent z-fighting
    let y_offset = 0.15;

    // Helper to get world position for a vertex
    let get_vertex_pos = |vx: u32, vz: u32| -> Vec3 {
        let height_normalized = chunk_data.get_height(vx, vz, resolution);
        let height = min_height + height_normalized * height_range;
        Vec3::new(
            pos.x + vx as f32 * spacing,
            pos.y + height + y_offset,
            pos.z + vz as f32 * spacing,
        )
    };

    // Draw front edge (z = 0)
    for vx in 0..(resolution - 1) {
        let p1 = get_vertex_pos(vx, 0);
        let p2 = get_vertex_pos(vx + 1, 0);
        gizmos.line(p1, p2, color);
    }

    // Draw back edge (z = max)
    for vx in 0..(resolution - 1) {
        let p1 = get_vertex_pos(vx, resolution - 1);
        let p2 = get_vertex_pos(vx + 1, resolution - 1);
        gizmos.line(p1, p2, color);
    }

    // Draw left edge (x = 0)
    for vz in 0..(resolution - 1) {
        let p1 = get_vertex_pos(0, vz);
        let p2 = get_vertex_pos(0, vz + 1);
        gizmos.line(p1, p2, color);
    }

    // Draw right edge (x = max)
    for vz in 0..(resolution - 1) {
        let p1 = get_vertex_pos(resolution - 1, vz);
        let p2 = get_vertex_pos(resolution - 1, vz + 1);
        gizmos.line(p1, p2, color);
    }
}
