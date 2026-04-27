//! Wireframe gizmos for `CollisionShapeData` so colliders are visible in the editor viewport.
//!
//! Drawn every frame for every entity with a `CollisionShapeData` + `GlobalTransform`.
//! Uses the same `OverlayGizmoGroup` config as the other line-based gizmos so it
//! respects depth bias and render layer 1.

use bevy::prelude::*;
use bevy::camera::primitives::Aabb;

use renzora_editor::EditorSelection;
use renzora_physics::{CollisionShapeData, CollisionShapeType};

use crate::OverlayGizmoGroup;

const COLOR_STATIC: Color = Color::srgb(0.30, 0.85, 0.40);
const COLOR_DYNAMIC: Color = Color::srgb(1.0, 0.55, 0.15);
const COLOR_SENSOR: Color = Color::srgb(0.30, 0.70, 1.0);

pub fn draw_collider_gizmos(
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    selection: Res<EditorSelection>,
    query: Query<(
        Entity,
        &CollisionShapeData,
        &GlobalTransform,
        Option<&renzora_physics::PhysicsBodyData>,
        Option<&Aabb>,
    )>,
) {
    for (entity, shape, gt, body, aabb) in &query {
        if !selection.is_selected(entity) {
            continue;
        }
        let color = if shape.is_sensor {
            COLOR_SENSOR
        } else {
            match body.map(|b| b.body_type) {
                Some(renzora_physics::PhysicsBodyType::StaticBody) => COLOR_STATIC,
                _ => COLOR_DYNAMIC,
            }
        };

        let (scale, rot, trans) = gt.to_scale_rotation_translation();
        let center = trans + rot * (scale * shape.offset);
        let iso = Isometry3d::new(center, rot);

        match shape.shape_type {
            CollisionShapeType::Box => {
                let size = shape.half_extents * 2.0 * scale;
                let xform = Transform {
                    translation: center,
                    rotation: rot,
                    scale: size,
                };
                gizmos.cube(xform, color);
            }
            CollisionShapeType::Sphere => {
                let r = shape.radius * scale.max_element();
                gizmos.sphere(iso, r, color);
            }
            CollisionShapeType::Capsule => {
                let r = shape.radius * scale.x.max(scale.z);
                let hh = shape.half_height * scale.y;
                draw_capsule(&mut gizmos, center, rot, r, hh, color);
            }
            CollisionShapeType::Cylinder => {
                let r = shape.radius * scale.x.max(scale.z);
                let hh = shape.half_height * scale.y;
                draw_cylinder(&mut gizmos, center, rot, r, hh, color);
            }
            CollisionShapeType::Mesh => {
                if let Some(aabb) = aabb {
                    let size = Vec3::from(aabb.half_extents) * 2.0 * scale;
                    let aabb_center = trans + rot * (scale * Vec3::from(aabb.center));
                    gizmos.cube(Transform { translation: aabb_center, rotation: rot, scale: size }, color);
                }
            }
        }
    }
}

fn draw_capsule(
    gizmos: &mut Gizmos<OverlayGizmoGroup>,
    center: Vec3,
    rot: Quat,
    radius: f32,
    half_height: f32,
    color: Color,
) {
    let up = rot * Vec3::Y;
    let right = rot * Vec3::X;
    let fwd = rot * Vec3::Z;
    let top = center + up * half_height;
    let bot = center - up * half_height;

    // Equator circles at the cap joins.
    gizmos.circle(Isometry3d::new(top, rot * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), radius, color);
    gizmos.circle(Isometry3d::new(bot, rot * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), radius, color);

    // Vertical connecting lines between the cap joins.
    gizmos.line(top + right * radius, bot + right * radius, color);
    gizmos.line(top - right * radius, bot - right * radius, color);
    gizmos.line(top + fwd * radius, bot + fwd * radius, color);
    gizmos.line(top - fwd * radius, bot - fwd * radius, color);

    // Hemisphere arcs — drawn by hand as line segments for reliability across
    // Bevy versions. Two arcs per cap (one in XY plane, one in ZY plane of the
    // capsule's local space), each spanning 180°.
    draw_hemi_arc(gizmos, top, up, right, radius, color);
    draw_hemi_arc(gizmos, top, up, fwd, radius, color);
    draw_hemi_arc(gizmos, bot, -up, right, radius, color);
    draw_hemi_arc(gizmos, bot, -up, fwd, radius, color);
}

/// Draw a 180° arc from `center - side*radius` up over `center + up*radius` to
/// `center + side*radius`, using segmented lines.
fn draw_hemi_arc(
    gizmos: &mut Gizmos<OverlayGizmoGroup>,
    center: Vec3,
    up: Vec3,
    side: Vec3,
    radius: f32,
    color: Color,
) {
    const SEGS: usize = 16;
    let mut prev = center - side * radius;
    for i in 1..=SEGS {
        let t = i as f32 / SEGS as f32;
        let angle = std::f32::consts::PI * t;
        // Starts at -side (angle=0) → +up at angle=PI/2 → +side at angle=PI.
        let p = center + (-side * angle.cos() + up * angle.sin()) * radius;
        gizmos.line(prev, p, color);
        prev = p;
    }
}

fn draw_cylinder(
    gizmos: &mut Gizmos<OverlayGizmoGroup>,
    center: Vec3,
    rot: Quat,
    radius: f32,
    half_height: f32,
    color: Color,
) {
    let up = rot * Vec3::Y;
    let top = center + up * half_height;
    let bot = center - up * half_height;

    let cap_rot = rot * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    gizmos.circle(Isometry3d::new(top, cap_rot), radius, color);
    gizmos.circle(Isometry3d::new(bot, cap_rot), radius, color);

    let right = rot * Vec3::X;
    let fwd = rot * Vec3::Z;
    gizmos.line(top + right * radius, bot + right * radius, color);
    gizmos.line(top - right * radius, bot - right * radius, color);
    gizmos.line(top + fwd * radius, bot + fwd * radius, color);
    gizmos.line(top - fwd * radius, bot - fwd * radius, color);
}
