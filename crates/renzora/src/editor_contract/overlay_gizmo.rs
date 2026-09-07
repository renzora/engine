//! The viewport's shared overlay gizmo group, and the one primitive a plugin
//! most often wants to draw into it.
//!
//! [`OverlayGizmoGroup`] is the depth-biased group every editor overlay draws
//! into: selection boxes, collider wireframes, light ranges, camera frusta. Its
//! configuration is owned by `renzora_gizmo` and toggled at runtime by the
//! viewport's on-top setting, so a plugin must draw into *this* group rather
//! than declare one of its own.
//!
//! That is not a style preference. A gizmo group is identified by its `TypeId`,
//! and a plugin declaring its own would get a group Bevy has never configured:
//! no render layer, no depth bias, default line width. The lines would be
//! drawn, and then either not appear in any viewport or appear buried inside
//! the geometry they are meant to annotate. Nothing would be logged, because
//! nothing is wrong from Bevy's point of view.
//!
//! So the type lives here, where a plugin can name it, while every system that
//! configures and renders it stays in `renzora_gizmo`.
//!
//! # Why `draw_capsule` came with it
//!
//! A capsule is the shape a character controller sweeps and the shape a
//! physics collider most often is, so an overlay drawing one is not a niche
//! case. Bevy's `Gizmos` has no capsule primitive, and the hand-rolled version
//! below is thirty lines of arc plotting that would otherwise be copied into
//! every plugin that needed it. Copies drift: two capsules drawn slightly
//! differently in the same viewport read as two different shapes, which is
//! precisely the confusion a diagnostic overlay exists to remove.

use bevy::gizmos::config::GizmoConfigGroup;
use bevy::prelude::*;

/// The editor's shared, depth-biased overlay group.
///
/// Configured by `renzora_gizmo`. Draw into it with
/// `Gizmos<OverlayGizmoGroup>` as an ordinary system parameter.
#[derive(Default, Reflect, GizmoConfigGroup)]
#[reflect(Default)]
pub struct OverlayGizmoGroup;

/// A wireframe capsule standing along `rot`'s Y axis.
///
/// `half_height` is half the *cylinder* segment, not half the total height, so
/// the full extent is `2.0 * half_height + 2.0 * radius`. That matches how the
/// authored collider stores it, which is what lets a controller's sweep capsule
/// and its collider be drawn by one call and compared by eye.
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

/// An X across each of the four side panels of the hull, bounded by the
/// vertical seams drawn above.
///
/// Only the cylindrical middle gets these; the hemispherical caps keep their
/// arcs, which already read as a surface.
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

/// One 180° arc of a capsule cap, swept from `-side` over `up` to `+side`.
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
