//! Physics collision shape gizmos

use bevy::prelude::*;
use bevy::math::Isometry3d;

use crate::node_system::{CollisionShapeData, CollisionShapeType, PhysicsBodyData};
use super::SelectionGizmoGroup;

/// Draw collision shape gizmos for all physics entities
pub fn draw_physics_gizmos(
    mut gizmos: Gizmos<SelectionGizmoGroup>,
    collision_shapes: Query<(&GlobalTransform, &CollisionShapeData)>,
    physics_bodies: Query<&GlobalTransform, With<PhysicsBodyData>>,
) {
    let shape_color = Color::srgba(0.4, 0.9, 0.5, 0.8);
    let body_color = Color::srgba(0.3, 0.7, 0.9, 0.6);

    // Draw collision shapes
    for (transform, shape) in collision_shapes.iter() {
        let pos = transform.translation();
        let rotation = transform.to_scale_rotation_translation().1;

        match shape.shape_type {
            CollisionShapeType::Box => {
                draw_box_gizmo(&mut gizmos, pos, rotation, shape.half_extents, shape_color);
            }
            CollisionShapeType::Sphere => {
                draw_sphere_gizmo(&mut gizmos, pos, shape.radius, shape_color);
            }
            CollisionShapeType::Capsule => {
                draw_capsule_gizmo(&mut gizmos, pos, rotation, shape.radius, shape.half_height, shape_color);
            }
            CollisionShapeType::Cylinder => {
                draw_cylinder_gizmo(&mut gizmos, pos, rotation, shape.radius, shape.half_height, shape_color);
            }
        }
    }

    // Draw a small indicator for physics bodies without shapes
    for transform in physics_bodies.iter() {
        let pos = transform.translation();
        // Draw a small cross to indicate physics body position
        let size = 0.2;
        gizmos.line(pos - Vec3::X * size, pos + Vec3::X * size, body_color);
        gizmos.line(pos - Vec3::Y * size, pos + Vec3::Y * size, body_color);
        gizmos.line(pos - Vec3::Z * size, pos + Vec3::Z * size, body_color);
    }
}

/// Draw a wireframe box
fn draw_box_gizmo(gizmos: &mut Gizmos<SelectionGizmoGroup>, pos: Vec3, rotation: Quat, half_extents: Vec3, color: Color) {
    let corners = [
        Vec3::new(-half_extents.x, -half_extents.y, -half_extents.z),
        Vec3::new( half_extents.x, -half_extents.y, -half_extents.z),
        Vec3::new( half_extents.x, -half_extents.y,  half_extents.z),
        Vec3::new(-half_extents.x, -half_extents.y,  half_extents.z),
        Vec3::new(-half_extents.x,  half_extents.y, -half_extents.z),
        Vec3::new( half_extents.x,  half_extents.y, -half_extents.z),
        Vec3::new( half_extents.x,  half_extents.y,  half_extents.z),
        Vec3::new(-half_extents.x,  half_extents.y,  half_extents.z),
    ];

    let transformed: Vec<Vec3> = corners.iter().map(|c| pos + rotation * *c).collect();

    // Bottom face
    gizmos.line(transformed[0], transformed[1], color);
    gizmos.line(transformed[1], transformed[2], color);
    gizmos.line(transformed[2], transformed[3], color);
    gizmos.line(transformed[3], transformed[0], color);

    // Top face
    gizmos.line(transformed[4], transformed[5], color);
    gizmos.line(transformed[5], transformed[6], color);
    gizmos.line(transformed[6], transformed[7], color);
    gizmos.line(transformed[7], transformed[4], color);

    // Vertical edges
    gizmos.line(transformed[0], transformed[4], color);
    gizmos.line(transformed[1], transformed[5], color);
    gizmos.line(transformed[2], transformed[6], color);
    gizmos.line(transformed[3], transformed[7], color);
}

/// Draw a wireframe sphere (3 circles)
fn draw_sphere_gizmo(gizmos: &mut Gizmos<SelectionGizmoGroup>, pos: Vec3, radius: f32, color: Color) {
    // XY plane circle
    let xy_iso = Isometry3d::new(pos, Quat::IDENTITY);
    gizmos.circle(xy_iso, radius, color);

    // XZ plane circle
    let xz_iso = Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    gizmos.circle(xz_iso, radius, color);

    // YZ plane circle
    let yz_iso = Isometry3d::new(pos, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
    gizmos.circle(yz_iso, radius, color);
}

/// Draw a wireframe capsule
fn draw_capsule_gizmo(gizmos: &mut Gizmos<SelectionGizmoGroup>, pos: Vec3, rotation: Quat, radius: f32, half_height: f32, color: Color) {
    let up = rotation * Vec3::Y;
    let right = rotation * Vec3::X;
    let forward = rotation * Vec3::Z;

    let top_center = pos + up * half_height;
    let bottom_center = pos - up * half_height;

    // Draw top and bottom circles
    let top_iso = Isometry3d::new(top_center, rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    let bottom_iso = Isometry3d::new(bottom_center, rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    gizmos.circle(top_iso, radius, color);
    gizmos.circle(bottom_iso, radius, color);

    // Draw vertical lines connecting the circles
    gizmos.line(top_center + right * radius, bottom_center + right * radius, color);
    gizmos.line(top_center - right * radius, bottom_center - right * radius, color);
    gizmos.line(top_center + forward * radius, bottom_center + forward * radius, color);
    gizmos.line(top_center - forward * radius, bottom_center - forward * radius, color);

    // Draw hemisphere arcs on top and bottom
    draw_hemisphere_arcs(gizmos, top_center, rotation, radius, true, color);
    draw_hemisphere_arcs(gizmos, bottom_center, rotation, radius, false, color);
}

/// Draw a wireframe cylinder
fn draw_cylinder_gizmo(gizmos: &mut Gizmos<SelectionGizmoGroup>, pos: Vec3, rotation: Quat, radius: f32, half_height: f32, color: Color) {
    let up = rotation * Vec3::Y;
    let right = rotation * Vec3::X;
    let forward = rotation * Vec3::Z;

    let top_center = pos + up * half_height;
    let bottom_center = pos - up * half_height;

    // Draw top and bottom circles
    let top_iso = Isometry3d::new(top_center, rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    let bottom_iso = Isometry3d::new(bottom_center, rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    gizmos.circle(top_iso, radius, color);
    gizmos.circle(bottom_iso, radius, color);

    // Draw vertical lines connecting the circles
    gizmos.line(top_center + right * radius, bottom_center + right * radius, color);
    gizmos.line(top_center - right * radius, bottom_center - right * radius, color);
    gizmos.line(top_center + forward * radius, bottom_center + forward * radius, color);
    gizmos.line(top_center - forward * radius, bottom_center - forward * radius, color);
}

/// Helper to draw hemisphere arcs for capsule ends
fn draw_hemisphere_arcs(gizmos: &mut Gizmos<SelectionGizmoGroup>, center: Vec3, rotation: Quat, radius: f32, top: bool, color: Color) {
    let segments = 8;
    let right = rotation * Vec3::X;
    let forward = rotation * Vec3::Z;
    let up = rotation * Vec3::Y;

    let sign = if top { 1.0 } else { -1.0 };

    // Draw arc in XY plane (from side view)
    for i in 0..segments {
        let angle1 = (i as f32 / segments as f32) * std::f32::consts::FRAC_PI_2;
        let angle2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::FRAC_PI_2;

        let p1 = center + right * radius * angle1.cos() + up * radius * angle1.sin() * sign;
        let p2 = center + right * radius * angle2.cos() + up * radius * angle2.sin() * sign;
        gizmos.line(p1, p2, color);

        let p3 = center - right * radius * angle1.cos() + up * radius * angle1.sin() * sign;
        let p4 = center - right * radius * angle2.cos() + up * radius * angle2.sin() * sign;
        gizmos.line(p3, p4, color);
    }

    // Draw arc in ZY plane (from front view)
    for i in 0..segments {
        let angle1 = (i as f32 / segments as f32) * std::f32::consts::FRAC_PI_2;
        let angle2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::FRAC_PI_2;

        let p1 = center + forward * radius * angle1.cos() + up * radius * angle1.sin() * sign;
        let p2 = center + forward * radius * angle2.cos() + up * radius * angle2.sin() * sign;
        gizmos.line(p1, p2, color);

        let p3 = center - forward * radius * angle1.cos() + up * radius * angle1.sin() * sign;
        let p4 = center - forward * radius * angle2.cos() + up * radius * angle2.sin() * sign;
        gizmos.line(p3, p4, color);
    }
}
