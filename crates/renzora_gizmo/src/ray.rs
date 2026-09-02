//! Ray construction and ray-vs-handle geometry.
//!
//! The gizmo does not use Bevy's mesh picking for its own handles — it
//! hit-tests them analytically against the same shapes the draw code emits, so
//! visual, pick and drag agree exactly. That means these helpers and the draw
//! helpers in `lines.rs` have to be kept in step by hand.

use bevy::prelude::*;

use renzora::core::viewport_types::ViewportState;

use crate::types::GizmoAxis;

pub(crate) fn viewport_cursor_ray(
    window: &Window,
    viewport: &ViewportState,
    camera_transform: &GlobalTransform,
    projection: &Projection,
) -> Option<Ray3d> {
    let cursor = window.cursor_position()?;
    let vp_local = cursor - viewport.screen_position;
    if vp_local.x < 0.0
        || vp_local.y < 0.0
        || vp_local.x > viewport.screen_size.x
        || vp_local.y > viewport.screen_size.y
    {
        return None;
    }

    let ndc = Vec2::new(
        (vp_local.x / viewport.screen_size.x) * 2.0 - 1.0,
        1.0 - (vp_local.y / viewport.screen_size.y) * 2.0,
    );
    let near = camera_transform.translation();

    match projection {
        Projection::Perspective(persp) => {
            let hh = (persp.fov * 0.5).tan();
            let hw = hh * persp.aspect_ratio;
            let local_dir = Vec3::new(ndc.x * hw, ndc.y * hh, -1.0).normalize();
            let world_dir = camera_transform
                .affine()
                .matrix3
                .mul_vec3(local_dir)
                .normalize();
            Some(Ray3d {
                origin: near,
                direction: Dir3::new(world_dir).ok()?,
            })
        }
        Projection::Orthographic(ortho) => {
            let hw = ortho.area.width() * 0.5;
            let hh = ortho.area.height() * 0.5;
            let offset =
                camera_transform
                    .affine()
                    .matrix3
                    .mul_vec3(Vec3::new(ndc.x * hw, ndc.y * hh, 0.0));
            Some(Ray3d {
                origin: (near + offset),
                direction: camera_transform.forward(),
            })
        }
        _ => None,
    }
}

/// Parameter `s` of the point on the infinite line `origin + dir*s` (`dir`
/// unit) closest to `ray`. `None` when the ray and line are near-parallel.
/// Used to keep the dragged point pinned under the cursor along an axis.
pub(crate) fn ray_line_param(ray: &Ray3d, origin: Vec3, dir: Vec3) -> Option<f32> {
    let d = ray.direction.as_vec3();
    let b = d.dot(dir);
    let denom = 1.0 - b * b;
    if denom.abs() < 1e-6 {
        return None;
    }
    let w0 = ray.origin - origin;
    Some((dir.dot(w0) - b * d.dot(w0)) / denom)
}

/// World point where `ray` meets the plane through `origin` with `normal`.
/// `None` when the ray is parallel to (or pointing away from) the plane.
pub(crate) fn ray_plane_point(ray: &Ray3d, origin: Vec3, normal: Vec3) -> Option<Vec3> {
    let d = ray.direction.as_vec3();
    let denom = d.dot(normal);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (origin - ray.origin).dot(normal) / denom;
    if t < 0.0 {
        return None;
    }
    Some(ray.origin + d * t)
}

/// The world point under the cursor projected onto the active translate handle:
/// the closest point on the axis line for a single axis, or the ray–plane hit
/// for a plane handle. `None` if the cursor isn't over the viewport or the
/// projection is degenerate.
pub(crate) fn translate_cursor_point(
    ray: &Ray3d,
    pivot: Vec3,
    basis: Quat,
    axis: GizmoAxis,
) -> Option<Vec3> {
    if axis.is_plane() {
        ray_plane_point(ray, pivot, basis * axis.direction())
    } else {
        let dir = basis * axis.direction();
        ray_line_param(ray, pivot, dir).map(|s| pivot + dir * s)
    }
}

pub(crate) fn closest_distance_ray_segment(ray: &Ray3d, seg_a: Vec3, seg_b: Vec3) -> Option<f32> {
    let ro: Vec3 = ray.origin;
    let rd: Vec3 = ray.direction.as_vec3();
    let sd = seg_b - seg_a;
    let sl = sd.length();
    if sl < 1e-6 {
        return None;
    }
    let su = sd / sl;
    let w0 = ro - seg_a;
    let a = rd.dot(rd);
    let b = rd.dot(su);
    let c = su.dot(su);
    let d = rd.dot(w0);
    let e = su.dot(w0);
    let denom = a * c - b * b;
    if denom.abs() < 1e-8 {
        return None;
    }
    let t_ray = (b * e - c * d) / denom;
    let t_seg = (a * e - b * d) / denom;
    if t_ray < 0.0 {
        return None;
    }
    let tc = t_seg.clamp(0.0, sl);
    Some((ro + rd * t_ray - (seg_a + su * tc)).length())
}

pub(crate) fn ray_circle_distance(
    ray: &Ray3d,
    center: Vec3,
    normal: Vec3,
    radius: f32,
) -> Option<f32> {
    let (p1, p2) = perpendicular_pair(normal);
    let segs = 32;
    let mut best: Option<f32> = None;
    for i in 0..segs {
        let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / segs as f32) * std::f32::consts::TAU;
        let s0 = center + (p1 * a0.cos() + p2 * a0.sin()) * radius;
        let s1 = center + (p1 * a1.cos() + p2 * a1.sin()) * radius;
        if let Some(d) = closest_distance_ray_segment(ray, s0, s1) {
            if best.is_none_or(|b| d < b) {
                best = Some(d);
            }
        }
    }
    best
}

pub(crate) fn ray_hits_plane_quad(
    ray: &Ray3d,
    corner: Vec3,
    axis_a: Vec3,
    axis_b: Vec3,
    size: f32,
) -> bool {
    let normal = axis_a.cross(axis_b).normalize();
    let ro: Vec3 = ray.origin;
    let rd: Vec3 = ray.direction.as_vec3();
    let denom = normal.dot(rd);
    if denom.abs() < 1e-6 {
        return false;
    }
    let t = normal.dot(corner - ro) / denom;
    if t < 0.0 {
        return false;
    }
    let hit = ro + rd * t;
    let local = hit - corner;
    let u = local.dot(axis_a);
    let v = local.dot(axis_b);
    u >= 0.0 && u <= size && v >= 0.0 && v <= size
}

pub(crate) fn perpendicular_pair(normal: Vec3) -> (Vec3, Vec3) {
    let p1 = if normal.y.abs() > 0.9 {
        Vec3::X
    } else {
        normal.cross(Vec3::Y).normalize()
    };
    let p2 = normal.cross(p1).normalize();
    (p1, p2)
}

pub(crate) fn pick_threshold(
    cam_gt: &GlobalTransform,
    entity_pos: Vec3,
    projection: &Projection,
    vh: f32,
) -> f32 {
    let dist = (cam_gt.translation() - entity_pos).length();
    let px = 12.0;
    match projection {
        Projection::Perspective(persp) => dist * (persp.fov * 0.5).tan() * 2.0 * px / vh,
        Projection::Orthographic(ortho) => ortho.area.height() * px / vh,
        _ => 0.1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    fn ray(origin: Vec3, dir: Vec3) -> Ray3d {
        Ray3d {
            origin,
            direction: Dir3::new(dir).unwrap(),
        }
    }

    // ── closest_distance_ray_segment ────────────────────────────────────────

    #[test]
    fn ray_segment_distance_at_closest_approach() {
        // Ray crossing 1 unit above the midpoint of a segment on the X axis.
        // The ray runs along +Z so it isn't parallel to the segment (a parallel
        // ray collapses the denominator and returns None — see the degenerate
        // test below).
        let r = ray(Vec3::new(0.0, 1.0, -5.0), Vec3::Z);
        let d = closest_distance_ray_segment(&r, Vec3::new(-1.0, 0.0, 0.0), Vec3::X).unwrap();
        assert!((d - 1.0).abs() < 1e-4, "expected 1.0, got {d}");

        // Ray passing straight through the segment midpoint → ~0.
        let r = ray(Vec3::new(0.0, 5.0, 0.0), Vec3::NEG_Y);
        let d = closest_distance_ray_segment(&r, Vec3::new(-1.0, 0.0, 0.0), Vec3::X).unwrap();
        assert!(d < 1e-4, "expected ~0, got {d}");
    }

    #[test]
    fn ray_segment_distance_clamps_to_endpoints() {
        // Closest point on the infinite line is at x=10, but the segment ends
        // at x=1 — distance must be measured to the endpoint instead.
        let r = ray(Vec3::new(10.0, 5.0, 0.0), Vec3::NEG_Y);
        let d = closest_distance_ray_segment(&r, Vec3::ZERO, Vec3::X).unwrap();
        assert!((d - 9.0).abs() < 1e-3, "expected 9.0, got {d}");
    }

    #[test]
    fn ray_segment_distance_degenerate_cases_return_none() {
        // Parallel ray and segment → denominator collapses.
        let r = ray(Vec3::new(0.0, 1.0, 0.0), Vec3::X);
        assert!(closest_distance_ray_segment(&r, Vec3::ZERO, Vec3::X * 5.0).is_none());

        // Zero-length segment.
        let r = ray(Vec3::new(0.0, 5.0, 0.0), Vec3::NEG_Y);
        assert!(closest_distance_ray_segment(&r, Vec3::ONE, Vec3::ONE).is_none());

        // Closest approach behind the ray origin.
        let r = ray(Vec3::new(0.0, 5.0, 0.0), Vec3::Y);
        assert!(closest_distance_ray_segment(&r, Vec3::new(-1.0, 0.0, 0.0), Vec3::X).is_none());
    }

    // ── ray_circle_distance ─────────────────────────────────────────────────

    #[test]
    fn ray_circle_distance_through_center_is_radius() {
        // Ray down the circle's normal through its center: every point on the
        // circle is `radius` away (modulo the 32-segment polyline chords).
        let r = ray(Vec3::new(0.0, 0.0, 10.0), Vec3::NEG_Z);
        let d = ray_circle_distance(&r, Vec3::ZERO, Vec3::Z, 2.0).unwrap();
        assert!(d > 1.95 && d <= 2.001, "expected ~2.0, got {d}");
    }

    #[test]
    fn ray_circle_distance_at_rim_is_near_zero() {
        let r = ray(Vec3::new(2.0, 0.0, 10.0), Vec3::NEG_Z);
        let d = ray_circle_distance(&r, Vec3::ZERO, Vec3::Z, 2.0).unwrap();
        assert!(d < 0.05, "expected ~0, got {d}");
    }

    // ── ray_hits_plane_quad ─────────────────────────────────────────────────

    #[test]
    fn ray_hits_plane_quad_inside_bounds() {
        // Quad spanning (0,0)..(2,2) on the XY plane, ray hits its middle.
        let r = ray(Vec3::new(1.0, 1.0, 5.0), Vec3::NEG_Z);
        assert!(ray_hits_plane_quad(&r, Vec3::ZERO, Vec3::X, Vec3::Y, 2.0));
    }

    #[test]
    fn ray_hits_plane_quad_rejects_misses() {
        // Hits the plane but outside the quad bounds.
        let r = ray(Vec3::new(3.0, 1.0, 5.0), Vec3::NEG_Z);
        assert!(!ray_hits_plane_quad(&r, Vec3::ZERO, Vec3::X, Vec3::Y, 2.0));

        // Ray parallel to the plane.
        let r = ray(Vec3::new(1.0, 1.0, 5.0), Vec3::X);
        assert!(!ray_hits_plane_quad(&r, Vec3::ZERO, Vec3::X, Vec3::Y, 2.0));

        // Plane behind the ray origin.
        let r = ray(Vec3::new(1.0, 1.0, 5.0), Vec3::Z);
        assert!(!ray_hits_plane_quad(&r, Vec3::ZERO, Vec3::X, Vec3::Y, 2.0));
    }

    // ── perpendicular_pair ──────────────────────────────────────────────────

    #[test]
    fn perpendicular_pair_is_orthonormal() {
        for normal in [Vec3::X, Vec3::Y, Vec3::Z, Vec3::new(1.0, 2.0, 3.0).normalize()] {
            let (p1, p2) = perpendicular_pair(normal);
            assert!((p1.length() - 1.0).abs() < 1e-5, "p1 not unit for {normal}");
            assert!((p2.length() - 1.0).abs() < 1e-5, "p2 not unit for {normal}");
            assert!(p1.dot(p2).abs() < 1e-5, "p1/p2 not orthogonal for {normal}");
            assert!(p1.dot(normal).abs() < 1e-5, "p1 not perp to {normal}");
            assert!(p2.dot(normal).abs() < 1e-5, "p2 not perp to {normal}");
        }
    }

    // ── pick_threshold ──────────────────────────────────────────────────────

    #[test]
    fn pick_threshold_perspective_scales_with_distance() {
        let cam = GlobalTransform::IDENTITY;
        let proj = Projection::Perspective(PerspectiveProjection {
            fov: FRAC_PI_2,
            aspect_ratio: 1.0,
            ..Default::default()
        });
        // tan(fov/2) = 1, so threshold = dist * 2 * 12 / vh.
        let near = pick_threshold(&cam, Vec3::new(0.0, 0.0, -10.0), &proj, 600.0);
        let far = pick_threshold(&cam, Vec3::new(0.0, 0.0, -20.0), &proj, 600.0);
        assert!((near - 0.4).abs() < 1e-4, "got {near}");
        assert!((far - 0.8).abs() < 1e-4, "got {far}");
    }

    #[test]
    fn pick_threshold_orthographic_ignores_distance() {
        let cam = GlobalTransform::IDENTITY;
        let mut ortho = OrthographicProjection::default_3d();
        ortho.area = Rect::new(-5.0, -5.0, 5.0, 5.0);
        let proj = Projection::Orthographic(ortho);
        let near = pick_threshold(&cam, Vec3::new(0.0, 0.0, -10.0), &proj, 600.0);
        let far = pick_threshold(&cam, Vec3::new(0.0, 0.0, -1000.0), &proj, 600.0);
        assert!((near - 0.2).abs() < 1e-4, "got {near}");
        assert_eq!(near, far);
    }
}
