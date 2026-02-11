//! Physics debug visualization gizmos
//!
//! Renders wireframe colliders, velocity arrows, center of mass markers,
//! AABBs, and contact points based on PhysicsDebugToggles.

use bevy::prelude::*;
use bevy::math::Isometry3d;

use crate::core::resources::physics_debug::PhysicsDebugState;

/// Custom gizmo config group for physics debug visualization
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct PhysicsVizGizmoGroup;

/// System that reads PhysicsDebugToggles and draws debug gizmos for physics entities
pub fn render_physics_debug_gizmos(
    mut gizmos: Gizmos<PhysicsVizGizmoGroup>,
    physics_debug: Res<PhysicsDebugState>,
    colliders: Query<(&GlobalTransform, &avian3d::prelude::Collider), With<avian3d::prelude::RigidBody>>,
    velocities: Query<(&GlobalTransform, &avian3d::prelude::LinearVelocity), With<avian3d::prelude::RigidBody>>,
    centers_of_mass: Query<(&GlobalTransform, &avian3d::prelude::CenterOfMass), With<avian3d::prelude::RigidBody>>,
    aabbs: Query<&avian3d::prelude::ColliderAabb>,
    colliding_entities: Query<&avian3d::prelude::CollidingEntities>,
    positions: Query<&GlobalTransform>,
) {
    let toggles = &physics_debug.debug_toggles;

    // Draw collider wireframes
    if toggles.show_colliders {
        let color = Color::srgba(0.2, 0.8, 0.4, 0.6);
        for (transform, collider) in colliders.iter() {
            let pos = transform.translation();
            let rotation = transform.to_scale_rotation_translation().1;
            let shape = collider.shape_scaled();

            if let Some(ball) = shape.as_ball() {
                draw_sphere(&mut gizmos, pos, ball.radius, color);
            } else if let Some(cuboid) = shape.as_cuboid() {
                let half = Vec3::new(cuboid.half_extents.x, cuboid.half_extents.y, cuboid.half_extents.z);
                draw_box(&mut gizmos, pos, rotation, half, color);
            } else if let Some(capsule) = shape.as_capsule() {
                draw_capsule(&mut gizmos, pos, rotation, capsule.radius, capsule.half_height(), color);
            } else if let Some(cylinder) = shape.as_cylinder() {
                draw_cylinder(&mut gizmos, pos, rotation, cylinder.radius, cylinder.half_height, color);
            }
        }
    }

    // Draw velocity arrows
    if toggles.show_velocities {
        let color = Color::srgba(1.0, 0.4, 0.1, 0.8);
        for (transform, lin_vel) in velocities.iter() {
            let pos = transform.translation();
            let vel = lin_vel.0;
            let speed = vel.length();
            if speed > 0.01 {
                let end = pos + vel.normalize() * (speed.min(10.0) * 0.5);
                gizmos.arrow(pos, end, color);
            }
        }
    }

    // Draw center of mass markers
    if toggles.show_center_of_mass {
        let color = Color::srgba(1.0, 1.0, 0.2, 0.9);
        let size = 0.1;
        for (transform, com) in centers_of_mass.iter() {
            let pos = transform.translation() + transform.to_scale_rotation_translation().1 * com.0;
            gizmos.line(pos - Vec3::X * size, pos + Vec3::X * size, color);
            gizmos.line(pos - Vec3::Y * size, pos + Vec3::Y * size, color);
            gizmos.line(pos - Vec3::Z * size, pos + Vec3::Z * size, color);
        }
    }

    // Draw AABBs
    if toggles.show_aabbs {
        let color = Color::srgba(0.5, 0.5, 1.0, 0.4);
        for aabb in aabbs.iter() {
            let min = Vec3::new(aabb.min.x, aabb.min.y, aabb.min.z);
            let max = Vec3::new(aabb.max.x, aabb.max.y, aabb.max.z);
            let center = (min + max) * 0.5;
            let half = (max - min) * 0.5;
            draw_box(&mut gizmos, center, Quat::IDENTITY, half, color);
        }
    }

    // Draw contact points (from CollidingEntities — show a marker between colliding pairs)
    if toggles.show_contacts {
        let color = Color::srgba(1.0, 0.2, 0.2, 0.9);
        for colliding in colliding_entities.iter() {
            for &other in colliding.iter() {
                // Draw a small marker at the midpoint between the two entities
                if let (Ok(t1), Ok(t2)) = (positions.get(colliding.iter().next().copied().unwrap_or(other)), positions.get(other)) {
                    let midpoint = (t1.translation() + t2.translation()) * 0.5;
                    let s = 0.08;
                    gizmos.line(midpoint - Vec3::X * s, midpoint + Vec3::X * s, color);
                    gizmos.line(midpoint - Vec3::Y * s, midpoint + Vec3::Y * s, color);
                    gizmos.line(midpoint - Vec3::Z * s, midpoint + Vec3::Z * s, color);
                }
            }
        }
    }
}

// ============================================================================
// Drawing helpers (generic over PhysicsVizGizmoGroup)
// ============================================================================

fn draw_box(gizmos: &mut Gizmos<PhysicsVizGizmoGroup>, pos: Vec3, rotation: Quat, half_extents: Vec3, color: Color) {
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
    let t: Vec<Vec3> = corners.iter().map(|c| pos + rotation * *c).collect();

    // Bottom face
    gizmos.line(t[0], t[1], color);
    gizmos.line(t[1], t[2], color);
    gizmos.line(t[2], t[3], color);
    gizmos.line(t[3], t[0], color);
    // Top face
    gizmos.line(t[4], t[5], color);
    gizmos.line(t[5], t[6], color);
    gizmos.line(t[6], t[7], color);
    gizmos.line(t[7], t[4], color);
    // Vertical edges
    gizmos.line(t[0], t[4], color);
    gizmos.line(t[1], t[5], color);
    gizmos.line(t[2], t[6], color);
    gizmos.line(t[3], t[7], color);
}

fn draw_sphere(gizmos: &mut Gizmos<PhysicsVizGizmoGroup>, pos: Vec3, radius: f32, color: Color) {
    gizmos.circle(Isometry3d::new(pos, Quat::IDENTITY), radius, color);
    gizmos.circle(Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), radius, color);
    gizmos.circle(Isometry3d::new(pos, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)), radius, color);
}

fn draw_capsule(gizmos: &mut Gizmos<PhysicsVizGizmoGroup>, pos: Vec3, rotation: Quat, radius: f32, half_height: f32, color: Color) {
    let up = rotation * Vec3::Y;
    let right = rotation * Vec3::X;
    let forward = rotation * Vec3::Z;

    let top_center = pos + up * half_height;
    let bottom_center = pos - up * half_height;

    gizmos.circle(Isometry3d::new(top_center, rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), radius, color);
    gizmos.circle(Isometry3d::new(bottom_center, rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), radius, color);

    gizmos.line(top_center + right * radius, bottom_center + right * radius, color);
    gizmos.line(top_center - right * radius, bottom_center - right * radius, color);
    gizmos.line(top_center + forward * radius, bottom_center + forward * radius, color);
    gizmos.line(top_center - forward * radius, bottom_center - forward * radius, color);
}

fn draw_cylinder(gizmos: &mut Gizmos<PhysicsVizGizmoGroup>, pos: Vec3, rotation: Quat, radius: f32, half_height: f32, color: Color) {
    let up = rotation * Vec3::Y;
    let right = rotation * Vec3::X;
    let forward = rotation * Vec3::Z;

    let top_center = pos + up * half_height;
    let bottom_center = pos - up * half_height;

    gizmos.circle(Isometry3d::new(top_center, rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), radius, color);
    gizmos.circle(Isometry3d::new(bottom_center, rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), radius, color);

    gizmos.line(top_center + right * radius, bottom_center + right * radius, color);
    gizmos.line(top_center - right * radius, bottom_center - right * radius, color);
    gizmos.line(top_center + forward * radius, bottom_center + forward * radius, color);
    gizmos.line(top_center - forward * radius, bottom_center - forward * radius, color);
}
