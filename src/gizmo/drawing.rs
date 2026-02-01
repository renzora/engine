use bevy::prelude::*;
use bevy::math::Isometry3d;
use bevy_mod_outline::{OutlineVolume, OutlineStencil};

use crate::core::{EditorEntity, SelectionState};
use crate::shared::CameraNodeData;
use crate::shared::CameraRigData;
use crate::terrain::TerrainChunkData;

use super::modal_transform::ModalTransformState;
use super::{DragAxis, EditorTool, GizmoMode, GizmoState, SelectionGizmoGroup, SnapTarget, GIZMO_PLANE_OFFSET, GIZMO_PLANE_SIZE, GIZMO_SIZE};

/// Marker component for entities that currently have selection outline
#[derive(Component)]
pub struct SelectionOutline;

/// System to add/remove outline components based on selection state
pub fn update_selection_outlines(
    mut commands: Commands,
    selection: Res<SelectionState>,
    modal: Res<ModalTransformState>,
    gizmo_state: Res<GizmoState>,
    mesh_entities: Query<Entity, With<Mesh3d>>,
    children_query: Query<&Children>,
    outlined_entities: Query<Entity, With<SelectionOutline>>,
    cameras: Query<(), With<CameraNodeData>>,
    terrain_chunks: Query<(), With<TerrainChunkData>>,
) {
    // Primary and secondary outline colors
    let primary_color = Color::srgb(1.0, 0.5, 0.0); // Orange
    let secondary_color = Color::srgba(1.0, 0.5, 0.0, 0.8); // Lighter orange
    let outline_width = 3.0;

    // Don't show outlines during modal transform or collider edit mode
    let should_show_outlines = !modal.active && !gizmo_state.collider_edit.is_active();

    // Remove all existing outlines first
    for entity in outlined_entities.iter() {
        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.remove::<(OutlineVolume, OutlineStencil, SelectionOutline)>();
        }
    }

    if !should_show_outlines {
        return;
    }

    // Get selected entities
    let all_selected = selection.get_all_selected();

    for &entity in &all_selected {
        // Skip cameras - they have their own gizmo
        if cameras.get(entity).is_ok() {
            continue;
        }

        // Skip terrain chunks - they use border highlight instead
        if terrain_chunks.get(entity).is_ok() {
            continue;
        }

        let is_primary = selection.selected_entity == Some(entity);
        let color = if is_primary { primary_color } else { secondary_color };

        // Add outline to the entity itself if it has a mesh
        if mesh_entities.get(entity).is_ok() {
            if let Ok(mut entity_commands) = commands.get_entity(entity) {
                entity_commands.insert((
                    OutlineVolume {
                        visible: true,
                        width: outline_width,
                        colour: color,
                    },
                    OutlineStencil::default(),
                    SelectionOutline,
                ));
            }
        }

        // Also add outlines to child meshes
        add_outline_to_children(
            &mut commands,
            entity,
            color,
            outline_width,
            &mesh_entities,
            &children_query,
        );
    }
}

fn add_outline_to_children(
    commands: &mut Commands,
    entity: Entity,
    color: Color,
    width: f32,
    mesh_entities: &Query<Entity, With<Mesh3d>>,
    children_query: &Query<&Children>,
) {
    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            // If child has a mesh, add outline
            if mesh_entities.get(child).is_ok() {
                if let Ok(mut entity_commands) = commands.get_entity(child) {
                    entity_commands.insert((
                        OutlineVolume {
                            visible: true,
                            width,
                            colour: color,
                        },
                        OutlineStencil::default(),
                        SelectionOutline,
                    ));
                }
            }
            // Recurse into children
            add_outline_to_children(commands, child, color, width, mesh_entities, children_query);
        }
    }
}

pub fn draw_selection_gizmo(
    selection: Res<SelectionState>,
    gizmo_state: Res<GizmoState>,
    modal: Res<ModalTransformState>,
    mut gizmos: Gizmos<SelectionGizmoGroup>,
    transforms: Query<&Transform, With<EditorEntity>>,
    cameras: Query<&CameraNodeData>,
    camera_rigs: Query<(&CameraRigData, Option<&ChildOf>)>,
    parent_transforms: Query<&Transform, Without<CameraRigData>>,
) {
    // Don't draw gizmo during modal transform (G/R/S mode)
    if modal.active {
        return;
    }

    // Don't draw selection visuals when in collider edit mode
    if gizmo_state.collider_edit.is_active() {
        return;
    }

    // Draw camera gizmos for selected cameras/rigs
    let all_selected = selection.get_all_selected();
    for entity in &all_selected {
        let Ok(transform) = transforms.get(*entity) else {
            continue;
        };

        let is_primary = selection.selected_entity == Some(*entity);

        // Only draw camera gizmos for primary selection
        if is_primary {
            if let Ok(camera_data) = cameras.get(*entity) {
                draw_camera_gizmo(&mut gizmos, transform, camera_data);
            }

            // Check if this is a camera rig - draw camera rig gizmo
            if let Ok((rig_data, parent)) = camera_rigs.get(*entity) {
                let parent_pos = parent
                    .and_then(|p| parent_transforms.get(p.0).ok())
                    .map(|t| t.translation)
                    .unwrap_or(Vec3::ZERO);
                draw_camera_rig_gizmo(&mut gizmos, transform, rig_data, parent_pos);
            }
        }
    }

    // Only draw transform gizmo in Transform tool mode and if there's a primary selection
    let Some(selected) = selection.selected_entity else {
        return;
    };

    let Ok(transform) = transforms.get(selected) else {
        return;
    };

    // Don't show transform gizmo in collider edit mode or if not in transform tool
    if gizmo_state.tool != EditorTool::Transform || gizmo_state.collider_edit.is_active() {
        return;
    }

    let pos = transform.translation;

    // Determine axis colors based on hover/drag state
    let active_axis = gizmo_state.drag_axis.or(gizmo_state.hovered_axis);

    let highlight = Color::srgb(1.0, 1.0, 0.3); // Yellow highlight
    let x_base = Color::srgb(0.9, 0.2, 0.2);
    let y_base = Color::srgb(0.2, 0.9, 0.2);
    let z_base = Color::srgb(0.2, 0.2, 0.9);

    let x_color = if matches!(active_axis, Some(DragAxis::X) | Some(DragAxis::XY) | Some(DragAxis::XZ)) {
        highlight
    } else {
        x_base
    };

    let y_color = if matches!(active_axis, Some(DragAxis::Y) | Some(DragAxis::XY) | Some(DragAxis::YZ)) {
        highlight
    } else {
        y_base
    };

    let z_color = if matches!(active_axis, Some(DragAxis::Z) | Some(DragAxis::XZ) | Some(DragAxis::YZ)) {
        highlight
    } else {
        z_base
    };

    let gizmo_size = GIZMO_SIZE;

    match gizmo_state.mode {
        GizmoMode::Translate => {
            // Arrows and center cube are rendered using 3D meshes (see meshes.rs)
            // Draw wireframe plane handles here
            let xy_color = if active_axis == Some(DragAxis::XY) { highlight } else { Color::srgba(0.9, 0.9, 0.2, 0.9) };
            let xz_color = if active_axis == Some(DragAxis::XZ) { highlight } else { Color::srgba(0.9, 0.2, 0.9, 0.9) };
            let yz_color = if active_axis == Some(DragAxis::YZ) { highlight } else { Color::srgba(0.2, 0.9, 0.9, 0.9) };

            let plane_half = GIZMO_PLANE_SIZE * 0.5;

            // XY plane handle (wireframe square)
            let xy_center = pos + Vec3::new(GIZMO_PLANE_OFFSET, GIZMO_PLANE_OFFSET, 0.0);
            gizmos.line(xy_center + Vec3::new(-plane_half, -plane_half, 0.0), xy_center + Vec3::new(plane_half, -plane_half, 0.0), xy_color);
            gizmos.line(xy_center + Vec3::new(plane_half, -plane_half, 0.0), xy_center + Vec3::new(plane_half, plane_half, 0.0), xy_color);
            gizmos.line(xy_center + Vec3::new(plane_half, plane_half, 0.0), xy_center + Vec3::new(-plane_half, plane_half, 0.0), xy_color);
            gizmos.line(xy_center + Vec3::new(-plane_half, plane_half, 0.0), xy_center + Vec3::new(-plane_half, -plane_half, 0.0), xy_color);

            // XZ plane handle (wireframe square)
            let xz_center = pos + Vec3::new(GIZMO_PLANE_OFFSET, 0.0, GIZMO_PLANE_OFFSET);
            gizmos.line(xz_center + Vec3::new(-plane_half, 0.0, -plane_half), xz_center + Vec3::new(plane_half, 0.0, -plane_half), xz_color);
            gizmos.line(xz_center + Vec3::new(plane_half, 0.0, -plane_half), xz_center + Vec3::new(plane_half, 0.0, plane_half), xz_color);
            gizmos.line(xz_center + Vec3::new(plane_half, 0.0, plane_half), xz_center + Vec3::new(-plane_half, 0.0, plane_half), xz_color);
            gizmos.line(xz_center + Vec3::new(-plane_half, 0.0, plane_half), xz_center + Vec3::new(-plane_half, 0.0, -plane_half), xz_color);

            // YZ plane handle (wireframe square)
            let yz_center = pos + Vec3::new(0.0, GIZMO_PLANE_OFFSET, GIZMO_PLANE_OFFSET);
            gizmos.line(yz_center + Vec3::new(0.0, -plane_half, -plane_half), yz_center + Vec3::new(0.0, plane_half, -plane_half), yz_color);
            gizmos.line(yz_center + Vec3::new(0.0, plane_half, -plane_half), yz_center + Vec3::new(0.0, plane_half, plane_half), yz_color);
            gizmos.line(yz_center + Vec3::new(0.0, plane_half, plane_half), yz_center + Vec3::new(0.0, -plane_half, plane_half), yz_color);
            gizmos.line(yz_center + Vec3::new(0.0, -plane_half, plane_half), yz_center + Vec3::new(0.0, -plane_half, -plane_half), yz_color);
        }
        GizmoMode::Rotate => {
            let radius = gizmo_size * 0.7;
            // X axis circle (YZ plane)
            let x_iso = Isometry3d::new(pos, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
            gizmos.circle(x_iso, radius, x_color);
            // Y axis circle (XZ plane)
            let y_iso = Isometry3d::new(pos, Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
            gizmos.circle(y_iso, radius, y_color);
            // Z axis circle (XY plane)
            let z_iso = Isometry3d::new(pos, Quat::IDENTITY);
            gizmos.circle(z_iso, radius, z_color);
        }
        GizmoMode::Scale => {
            // X axis with box
            gizmos.line(pos, pos + Vec3::X * gizmo_size, x_color);
            gizmos.cube(Transform::from_translation(pos + Vec3::X * gizmo_size).with_scale(Vec3::splat(0.15)), x_color);

            // Y axis with box
            gizmos.line(pos, pos + Vec3::Y * gizmo_size, y_color);
            gizmos.cube(Transform::from_translation(pos + Vec3::Y * gizmo_size).with_scale(Vec3::splat(0.15)), y_color);

            // Z axis with box
            gizmos.line(pos, pos + Vec3::Z * gizmo_size, z_color);
            gizmos.cube(Transform::from_translation(pos + Vec3::Z * gizmo_size).with_scale(Vec3::splat(0.15)), z_color);
        }
    }

    // Draw snap indicator when snapping is active
    if gizmo_state.is_dragging {
        draw_snap_indicator(&mut gizmos, &gizmo_state, pos);
    }
}

/// Draw visual indicator when snapping to an object or the floor
fn draw_snap_indicator(
    gizmos: &mut Gizmos<SelectionGizmoGroup>,
    gizmo_state: &GizmoState,
    current_pos: Vec3,
) {
    let snap_color = Color::srgb(0.0, 1.0, 0.5); // Bright green for snap indicator
    let floor_color = Color::srgb(0.5, 0.8, 1.0); // Light blue for floor snap

    match gizmo_state.snap_target {
        SnapTarget::Entity(_) => {
            if let Some(target_pos) = gizmo_state.snap_target_position {
                // Draw a line connecting the two objects
                gizmos.line(current_pos, target_pos, snap_color);

                // Draw a diamond/cross at the snap target
                let indicator_size = 0.2;
                gizmos.line(
                    target_pos + Vec3::new(-indicator_size, 0.0, 0.0),
                    target_pos + Vec3::new(indicator_size, 0.0, 0.0),
                    snap_color,
                );
                gizmos.line(
                    target_pos + Vec3::new(0.0, -indicator_size, 0.0),
                    target_pos + Vec3::new(0.0, indicator_size, 0.0),
                    snap_color,
                );
                gizmos.line(
                    target_pos + Vec3::new(0.0, 0.0, -indicator_size),
                    target_pos + Vec3::new(0.0, 0.0, indicator_size),
                    snap_color,
                );

                // Draw a small ring around the snapped position
                let iso = bevy::math::Isometry3d::new(target_pos, Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
                gizmos.circle(iso, 0.15, snap_color);
            }
        }
        SnapTarget::Floor => {
            if let Some(floor_pos) = gizmo_state.snap_target_position {
                // Draw vertical line from object to floor
                gizmos.line(current_pos, floor_pos, floor_color);

                // Draw a cross on the floor plane
                let indicator_size = 0.3;
                gizmos.line(
                    floor_pos + Vec3::new(-indicator_size, 0.0, 0.0),
                    floor_pos + Vec3::new(indicator_size, 0.0, 0.0),
                    floor_color,
                );
                gizmos.line(
                    floor_pos + Vec3::new(0.0, 0.0, -indicator_size),
                    floor_pos + Vec3::new(0.0, 0.0, indicator_size),
                    floor_color,
                );

                // Draw a small circle on the floor
                let iso = bevy::math::Isometry3d::new(floor_pos, Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
                gizmos.circle(iso, 0.2, floor_color);
            }
        }
        SnapTarget::None => {}
    }
}

/// Draw a camera frustum gizmo for camera nodes
fn draw_camera_gizmo(gizmos: &mut Gizmos<SelectionGizmoGroup>, transform: &Transform, camera_data: &CameraNodeData) {
    let pos = transform.translation;
    let rotation = transform.rotation;

    // Camera body color
    let camera_color = Color::srgb(0.8, 0.8, 0.9);
    let frustum_color = Color::srgba(0.6, 0.7, 1.0, 0.7);

    // Get local directions
    let forward = rotation * Vec3::NEG_Z;
    let right = rotation * Vec3::X;
    let up = rotation * Vec3::Y;

    // Camera body dimensions
    let body_length = 0.4;
    let body_width = 0.3;
    let body_height = 0.2;

    // Draw camera body (box shape)
    let body_center = pos - forward * (body_length * 0.5);

    // Body corners
    let blf = body_center - right * body_width * 0.5 - up * body_height * 0.5 - forward * body_length * 0.5;
    let brf = body_center + right * body_width * 0.5 - up * body_height * 0.5 - forward * body_length * 0.5;
    let tlf = body_center - right * body_width * 0.5 + up * body_height * 0.5 - forward * body_length * 0.5;
    let trf = body_center + right * body_width * 0.5 + up * body_height * 0.5 - forward * body_length * 0.5;
    let blb = body_center - right * body_width * 0.5 - up * body_height * 0.5 + forward * body_length * 0.5;
    let brb = body_center + right * body_width * 0.5 - up * body_height * 0.5 + forward * body_length * 0.5;
    let tlb = body_center - right * body_width * 0.5 + up * body_height * 0.5 + forward * body_length * 0.5;
    let trb = body_center + right * body_width * 0.5 + up * body_height * 0.5 + forward * body_length * 0.5;

    // Front face
    gizmos.line(blf, brf, camera_color);
    gizmos.line(brf, trf, camera_color);
    gizmos.line(trf, tlf, camera_color);
    gizmos.line(tlf, blf, camera_color);

    // Back face
    gizmos.line(blb, brb, camera_color);
    gizmos.line(brb, trb, camera_color);
    gizmos.line(trb, tlb, camera_color);
    gizmos.line(tlb, blb, camera_color);

    // Connecting edges
    gizmos.line(blf, blb, camera_color);
    gizmos.line(brf, brb, camera_color);
    gizmos.line(tlf, tlb, camera_color);
    gizmos.line(trf, trb, camera_color);

    // Draw lens (cylinder-like shape on front)
    let lens_center = pos;
    let lens_radius = 0.12;
    let lens_depth = 0.1;

    // Draw lens circles
    let segments = 12;
    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let next_angle = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

        let p1 = lens_center + (right * angle.cos() + up * angle.sin()) * lens_radius;
        let p2 = lens_center + (right * next_angle.cos() + up * next_angle.sin()) * lens_radius;
        let p3 = lens_center + forward * lens_depth + (right * angle.cos() + up * angle.sin()) * lens_radius;
        let p4 = lens_center + forward * lens_depth + (right * next_angle.cos() + up * next_angle.sin()) * lens_radius;

        gizmos.line(p1, p2, camera_color);
        gizmos.line(p3, p4, camera_color);
        if i % 3 == 0 {
            gizmos.line(p1, p3, camera_color);
        }
    }

    // Draw view frustum
    let fov_rad = camera_data.fov.to_radians();
    let near = 0.5;
    let far = 3.0;
    let aspect = 16.0 / 9.0;

    let near_height = near * (fov_rad / 2.0).tan();
    let near_width = near_height * aspect;
    let far_height = far * (fov_rad / 2.0).tan();
    let far_width = far_height * aspect;

    // Near plane corners
    let near_center = pos + forward * near;
    let near_bl = near_center - right * near_width - up * near_height;
    let near_br = near_center + right * near_width - up * near_height;
    let near_tl = near_center - right * near_width + up * near_height;
    let near_tr = near_center + right * near_width + up * near_height;

    // Far plane corners
    let far_center = pos + forward * far;
    let far_bl = far_center - right * far_width - up * far_height;
    let far_br = far_center + right * far_width - up * far_height;
    let far_tl = far_center - right * far_width + up * far_height;
    let far_tr = far_center + right * far_width + up * far_height;

    // Draw near plane
    gizmos.line(near_bl, near_br, frustum_color);
    gizmos.line(near_br, near_tr, frustum_color);
    gizmos.line(near_tr, near_tl, frustum_color);
    gizmos.line(near_tl, near_bl, frustum_color);

    // Draw far plane
    gizmos.line(far_bl, far_br, frustum_color);
    gizmos.line(far_br, far_tr, frustum_color);
    gizmos.line(far_tr, far_tl, frustum_color);
    gizmos.line(far_tl, far_bl, frustum_color);

    // Draw frustum edges (connecting near to far)
    gizmos.line(near_bl, far_bl, frustum_color);
    gizmos.line(near_br, far_br, frustum_color);
    gizmos.line(near_tl, far_tl, frustum_color);
    gizmos.line(near_tr, far_tr, frustum_color);

    // Draw forward direction indicator
    let arrow_start = pos + forward * 0.3;
    let arrow_end = pos + forward * 0.8;
    gizmos.line(arrow_start, arrow_end, Color::srgb(0.3, 0.6, 1.0));

    // Small arrow head
    let arrow_head_size = 0.1;
    gizmos.line(arrow_end, arrow_end - forward * arrow_head_size + right * arrow_head_size * 0.5, Color::srgb(0.3, 0.6, 1.0));
    gizmos.line(arrow_end, arrow_end - forward * arrow_head_size - right * arrow_head_size * 0.5, Color::srgb(0.3, 0.6, 1.0));
}

/// Draw a camera rig gizmo showing the rig arm and camera position
fn draw_camera_rig_gizmo(
    gizmos: &mut Gizmos<SelectionGizmoGroup>,
    transform: &Transform,
    rig_data: &CameraRigData,
    target_pos: Vec3,
) {
    let rig_color = Color::srgb(0.9, 0.6, 0.2);
    let arm_color = Color::srgba(0.9, 0.6, 0.2, 0.6);
    let frustum_color = Color::srgba(0.6, 0.8, 0.4, 0.5);

    // Camera position is at the rig's transform
    let camera_pos = transform.translation;

    // Draw target indicator (small cross at target position)
    let target_size = 0.3;
    gizmos.line(
        target_pos - Vec3::X * target_size,
        target_pos + Vec3::X * target_size,
        rig_color,
    );
    gizmos.line(
        target_pos - Vec3::Y * target_size,
        target_pos + Vec3::Y * target_size,
        rig_color,
    );
    gizmos.line(
        target_pos - Vec3::Z * target_size,
        target_pos + Vec3::Z * target_size,
        rig_color,
    );

    // Draw rig arm from target to camera
    gizmos.line(target_pos, camera_pos, arm_color);

    // Draw height indicator (vertical line from target)
    let height_point = target_pos + Vec3::Y * rig_data.height;
    gizmos.line(target_pos, height_point, Color::srgba(0.2, 0.8, 0.2, 0.5));

    // Draw distance indicator (horizontal from height point)
    let distance_point = height_point - Vec3::Z * rig_data.distance;
    gizmos.line(height_point, distance_point, Color::srgba(0.2, 0.2, 0.8, 0.5));

    // Draw horizontal offset if any
    if rig_data.horizontal_offset.abs() > 0.01 {
        let offset_point = distance_point + Vec3::X * rig_data.horizontal_offset;
        gizmos.line(distance_point, offset_point, Color::srgba(0.8, 0.2, 0.2, 0.5));
    }

    // Draw camera icon at camera position
    let rotation = transform.rotation;
    let forward = rotation * Vec3::NEG_Z;
    let right = rotation * Vec3::X;
    let up = rotation * Vec3::Y;

    // Camera body (simplified box)
    let body_size = 0.2;
    let half = body_size * 0.5;

    // Draw camera box
    let corners = [
        camera_pos + (-right - up - forward) * half,
        camera_pos + (right - up - forward) * half,
        camera_pos + (right + up - forward) * half,
        camera_pos + (-right + up - forward) * half,
        camera_pos + (-right - up + forward) * half,
        camera_pos + (right - up + forward) * half,
        camera_pos + (right + up + forward) * half,
        camera_pos + (-right + up + forward) * half,
    ];

    // Front face
    gizmos.line(corners[0], corners[1], rig_color);
    gizmos.line(corners[1], corners[2], rig_color);
    gizmos.line(corners[2], corners[3], rig_color);
    gizmos.line(corners[3], corners[0], rig_color);

    // Back face
    gizmos.line(corners[4], corners[5], rig_color);
    gizmos.line(corners[5], corners[6], rig_color);
    gizmos.line(corners[6], corners[7], rig_color);
    gizmos.line(corners[7], corners[4], rig_color);

    // Connecting edges
    gizmos.line(corners[0], corners[4], rig_color);
    gizmos.line(corners[1], corners[5], rig_color);
    gizmos.line(corners[2], corners[6], rig_color);
    gizmos.line(corners[3], corners[7], rig_color);

    // Draw lens circle
    let lens_center = camera_pos + forward * half;
    let lens_radius = 0.08;
    let segments = 8;
    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let next_angle = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
        let p1 = lens_center + (right * angle.cos() + up * angle.sin()) * lens_radius;
        let p2 = lens_center + (right * next_angle.cos() + up * next_angle.sin()) * lens_radius;
        gizmos.line(p1, p2, rig_color);
    }

    // Draw simplified view frustum
    let fov_rad = rig_data.fov.to_radians();
    let _near = 0.3;
    let far = 2.0;
    let aspect = 16.0 / 9.0;

    let far_height = far * (fov_rad / 2.0).tan();
    let far_width = far_height * aspect;

    let far_center = camera_pos + forward * far;
    let far_bl = far_center - right * far_width - up * far_height;
    let far_br = far_center + right * far_width - up * far_height;
    let far_tl = far_center - right * far_width + up * far_height;
    let far_tr = far_center + right * far_width + up * far_height;

    // Draw frustum edges from camera to far plane
    gizmos.line(camera_pos, far_bl, frustum_color);
    gizmos.line(camera_pos, far_br, frustum_color);
    gizmos.line(camera_pos, far_tl, frustum_color);
    gizmos.line(camera_pos, far_tr, frustum_color);

    // Draw far plane
    gizmos.line(far_bl, far_br, frustum_color);
    gizmos.line(far_br, far_tr, frustum_color);
    gizmos.line(far_tr, far_tl, frustum_color);
    gizmos.line(far_tl, far_bl, frustum_color);

    // Draw "look at target" indicator
    let look_dir = (target_pos - camera_pos).normalize_or_zero();
    let look_end = camera_pos + look_dir * 1.0;
    gizmos.line(camera_pos, look_end, Color::srgb(1.0, 0.8, 0.2));
}
