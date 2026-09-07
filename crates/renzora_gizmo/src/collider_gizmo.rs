//! Wireframe gizmos for `CollisionShapeData` so colliders are visible in the editor viewport.
//!
//! Drawn every frame for every entity with a `CollisionShapeData` + `GlobalTransform`.
//! Uses the same `OverlayGizmoGroup` config as the other line-based gizmos so it
//! respects depth bias and render layer 1.
//!
//! Which entities get a wireframe comes from `ViewportSettings::
//! collision_gizmo_visibility` (Gizmos dropdown → Physics). That setting had
//! existed — and been persisted — for a while before anything read it: this
//! drawer hard-coded selected-only, so picking "Always" in the UI silently did
//! nothing. It is now honoured, including the `Off` state.
//!
//! Colour says what the body is: green static, violet kinematic, orange
//! dynamic, blue sensor.
//!
//! Every hull also gets diagonals across its faces. A bare edge wireframe sits
//! on top of the mesh it wraps and reads as a jumble of unrelated lines — you
//! can't tell which edges belong to the near face and which to the far one, and
//! a collider that lines up with a boxy mesh disappears into that mesh's own
//! silhouette. The diagonals give each face a visible surface, so the collider
//! reads as a solid volume at a glance.

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;

use renzora::core::viewport_types::{CollisionGizmoVisibility, ViewportSettings};
use renzora_editor_framework::EditorSelection;
use renzora_physics::{CollisionShapeData, CollisionShapeType};

use crate::OverlayGizmoGroup;

const COLOR_STATIC: Color = Color::srgb(0.30, 0.85, 0.40);
const COLOR_DYNAMIC: Color = Color::srgb(1.0, 0.55, 0.15);
const COLOR_SENSOR: Color = Color::srgb(0.30, 0.70, 1.0);
/// Kinematic bodies used to share the dynamic colour, which hid the one
/// difference most worth seeing: a character that is supposed to be driven by a
/// controller but was left on the default (dynamic) body type looks identical
/// to a correctly set up one, while the solver quietly fights whatever is
/// moving it. Violet is deliberately far from the dynamic orange.
const COLOR_KINEMATIC: Color = Color::srgb(0.70, 0.45, 1.0);

pub fn draw_collider_gizmos(
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    selection: Res<EditorSelection>,
    settings: Option<Res<ViewportSettings>>,
    query: Query<(
        Entity,
        &CollisionShapeData,
        &GlobalTransform,
        Option<&renzora_physics::PhysicsBodyData>,
        Option<&Aabb>,
    )>,
) {
    let visibility = settings
        .map(|s| s.collision_gizmo_visibility)
        .unwrap_or_default();
    if visibility == CollisionGizmoVisibility::Off {
        return;
    }
    let selected_only = visibility == CollisionGizmoVisibility::SelectedOnly;

    for (entity, shape, gt, body, aabb) in &query {
        if selected_only && !selection.is_selected(entity) {
            continue;
        }
        let color = if shape.is_sensor {
            COLOR_SENSOR
        } else {
            match body.map(|b| b.body_type) {
                Some(renzora_physics::PhysicsBodyType::StaticBody) => COLOR_STATIC,
                Some(renzora_physics::PhysicsBodyType::KinematicBody) => COLOR_KINEMATIC,
                _ => COLOR_DYNAMIC,
            }
        };

        let (scale, rot, trans) = gt.to_scale_rotation_translation();
        let center = trans + rot * (scale * shape.offset);
        let iso = Isometry3d::new(center, rot);

        match shape.shape_type {
            CollisionShapeType::Box => {
                let half = shape.half_extents * scale;
                let xform = Transform {
                    translation: center,
                    rotation: rot,
                    scale: half * 2.0,
                };
                gizmos.cube(xform, color);
                draw_box_diagonals(&mut gizmos, center, rot, half, color);
            }
            CollisionShapeType::Sphere => {
                let r = shape.radius * scale.max_element();
                gizmos.sphere(iso, r, color);
                draw_sphere_diagonals(&mut gizmos, center, rot, r, color);
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
                    let half = Vec3::from(aabb.half_extents) * scale;
                    let aabb_center = trans + rot * (scale * Vec3::from(aabb.center));
                    gizmos.cube(
                        Transform {
                            translation: aabb_center,
                            rotation: rot,
                            scale: half * 2.0,
                        },
                        color,
                    );
                    draw_box_diagonals(&mut gizmos, aabb_center, rot, half, color);
                }
            }
        }
    }
}

/// Draw an X across each of the six faces of an oriented box, given its
/// *world-space* half extents (i.e. already multiplied by the transform scale).
///
/// `half` is deliberately half extents rather than the full size `gizmos.cube`
/// wants, because every corner here is `center ± hx ± hy ± hz` — taking the full
/// size and halving it again at each of the twelve line ends only invites the
/// factor-of-two slip.
fn draw_box_diagonals(
    gizmos: &mut Gizmos<OverlayGizmoGroup>,
    center: Vec3,
    rot: Quat,
    half: Vec3,
    color: Color,
) {
    let axes = [
        rot * Vec3::X * half.x,
        rot * Vec3::Y * half.y,
        rot * Vec3::Z * half.z,
    ];
    for i in 0..3 {
        // The two axes spanning the face whose normal is `axes[i]`.
        let u = axes[(i + 1) % 3];
        let v = axes[(i + 2) % 3];
        for sign in [1.0f32, -1.0] {
            let base = center + axes[i] * sign;
            gizmos.line(base + u + v, base - u - v, color);
            gizmos.line(base + u - v, base - u + v, color);
        }
    }
}

/// Two great circles tilted 45° off the equator — a sphere's stand-in for a face
/// diagonal, since `gizmos.sphere` only draws the three axis-aligned ones and a
/// sphere has no flat face to cross.
fn draw_sphere_diagonals(
    gizmos: &mut Gizmos<OverlayGizmoGroup>,
    center: Vec3,
    rot: Quat,
    radius: f32,
    color: Color,
) {
    const TILT: f32 = std::f32::consts::FRAC_PI_4;
    for tilt in [Quat::from_rotation_x(TILT), Quat::from_rotation_y(TILT)] {
        gizmos.circle(Isometry3d::new(center, rot * tilt), radius, color);
    }
}

/// Draw an X across each of the four side panels of a capsule/cylinder hull —
/// the quads bounded by the vertical seams the wireframe already draws.
///
/// For a capsule this covers only the cylindrical middle; the hemispherical caps
/// keep their arcs, which already give them a readable surface.
fn draw_side_diagonals(
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
    let right = rot * Vec3::X;
    let fwd = rot * Vec3::Z;

    // The four vertical seams, in order around the hull so consecutive pairs
    // bound one panel.
    let seams = [right, fwd, -right, -fwd];
    for i in 0..4 {
        let a = seams[i] * radius;
        let b = seams[(i + 1) % 4] * radius;
        gizmos.line(top + a, bot + b, color);
        gizmos.line(top + b, bot + a, color);
    }
}

pub fn draw_capsule(
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
    gizmos.circle(
        Isometry3d::new(
            top,
            rot * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        ),
        radius,
        color,
    );
    gizmos.circle(
        Isometry3d::new(
            bot,
            rot * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        ),
        radius,
        color,
    );

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

    draw_side_diagonals(gizmos, center, rot, radius, half_height, color);
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

    draw_side_diagonals(gizmos, center, rot, radius, half_height, color);
}
