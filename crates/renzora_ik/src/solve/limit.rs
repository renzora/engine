//! Swing/twist rotation limits.
//!
//! A joint's rotation away from its reference splits into a *twist* about the
//! bone's own axis (a forearm turning) and a *swing* that tips the axis
//! somewhere else (an arm lifting). Anatomical limits are naturally stated in
//! those two terms, a cone the bone may swing within and a range it may twist
//! through, and clamping each separately keeps the bone's direction when only
//! its twist is out of range, which clamping Euler angles would not.

use bevy::math::{Quat, Vec3};
use std::f32::consts::PI;

/// `q` split as `swing * twist`, with `twist` about `axis` (unit).
pub fn swing_twist(q: Quat, axis: Vec3) -> (Quat, Quat) {
    let v = Vec3::new(q.x, q.y, q.z);
    let p = axis * v.dot(axis);
    let twist = Quat::from_xyzw(p.x, p.y, p.z, q.w);
    // A half-turn swing leaves no twist component to normalise; any twist is
    // then as good as another, and none is the stable choice.
    let twist = if twist.length_squared() < 1e-12 { Quat::IDENTITY } else { twist.normalize() };
    let swing = q * twist.inverse();
    (swing, twist)
}

/// Signed twist angle, radians in -PI..PI, of a pure twist about `axis`.
pub fn twist_angle(twist: Quat, axis: Vec3) -> f32 {
    let s = Vec3::new(twist.x, twist.y, twist.z).dot(axis);
    let mut angle = 2.0 * s.atan2(twist.w);
    if angle > PI {
        angle -= 2.0 * PI;
    } else if angle < -PI {
        angle += 2.0 * PI;
    }
    angle
}

/// Clamp `rotation` (a local rotation) to swing at most `max_swing` radians
/// off `reference`'s `axis`, and twist between `min_twist` and `max_twist`
/// radians about it. `axis` is in the bone's local space.
pub fn clamp(
    rotation: Quat,
    reference: Quat,
    axis: Vec3,
    max_swing: f32,
    min_twist: f32,
    max_twist: f32,
) -> Quat {
    let Some(axis) = axis.try_normalize() else {
        return rotation;
    };
    // Relative to the reference, in the reference's frame, so `axis` means the
    // same thing whatever the parent is doing.
    let rel = reference.inverse() * rotation;
    let (mut swing, twist) = swing_twist(rel, axis);

    let (lo, hi) = if min_twist <= max_twist { (min_twist, max_twist) } else { (max_twist, min_twist) };
    let angle = twist_angle(twist, axis);
    let twist = if angle < lo || angle > hi {
        Quat::from_axis_angle(axis, angle.clamp(lo, hi))
    } else {
        twist
    };

    if swing.w < 0.0 {
        swing = -swing;
    }
    let swing_angle = 2.0 * swing.w.clamp(-1.0, 1.0).acos();
    let max_swing = max_swing.clamp(0.0, PI);
    if swing_angle > max_swing {
        let v = Vec3::new(swing.x, swing.y, swing.z);
        swing = match v.try_normalize() {
            Some(swing_axis) => Quat::from_axis_angle(swing_axis, max_swing),
            None => Quat::IDENTITY,
        };
    }
    (reference * swing * twist).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_6};

    fn close(a: Quat, b: Quat) -> bool {
        a.dot(b).abs() > 1.0 - 1e-5
    }

    #[test]
    fn decomposition_recomposes() {
        let q = Quat::from_euler(bevy::math::EulerRot::XYZ, 0.4, 1.1, -0.3);
        let (s, t) = swing_twist(q, Vec3::Y);
        assert!(close(s * t, q));
        // The swing has no component about the axis.
        assert!((s * Vec3::Y).distance(q * Vec3::Y) < 1e-5);
    }

    #[test]
    fn within_limits_is_untouched() {
        let q = Quat::from_rotation_x(0.3) * Quat::from_rotation_y(0.2);
        let out = clamp(q, Quat::IDENTITY, Vec3::Y, FRAC_PI_4, -FRAC_PI_4, FRAC_PI_4);
        assert!(close(out, q));
    }

    #[test]
    fn over_twist_is_clamped_and_the_direction_kept() {
        let q = Quat::from_rotation_x(0.2) * Quat::from_rotation_y(1.2);
        let out = clamp(q, Quat::IDENTITY, Vec3::Y, FRAC_PI_2, -FRAC_PI_6, FRAC_PI_6);
        let (_, t) = swing_twist(out, Vec3::Y);
        assert!((twist_angle(t, Vec3::Y) - FRAC_PI_6).abs() < 1e-4);
        // Twisting never moves the bone's axis: it points where it did.
        assert!((out * Vec3::Y).distance(q * Vec3::Y) < 1e-4);
    }

    #[test]
    fn over_swing_is_clamped_to_the_cone_in_the_same_direction() {
        let q = Quat::from_rotation_z(1.2);
        let out = clamp(q, Quat::IDENTITY, Vec3::Y, FRAC_PI_6, -PI, PI);
        let dir = out * Vec3::Y;
        assert!((dir.dot(Vec3::Y).acos() - FRAC_PI_6).abs() < 1e-4);
        // Swung toward -X, as the original was.
        assert!(dir.x < -0.4 && dir.z.abs() < 1e-4);
    }

    #[test]
    fn limits_are_relative_to_the_reference() {
        let reference = Quat::from_rotation_x(FRAC_PI_2);
        let q = reference * Quat::from_rotation_z(0.2);
        let out = clamp(q, reference, Vec3::Y, FRAC_PI_4, -0.1, 0.1);
        assert!(close(out, q));
        let far = reference * Quat::from_rotation_z(1.0);
        let out = clamp(far, reference, Vec3::Y, FRAC_PI_4, -0.1, 0.1);
        let swung = (reference.inverse() * out) * Vec3::Y;
        assert!((swung.dot(Vec3::Y).acos() - FRAC_PI_4).abs() < 1e-4);
    }

    #[test]
    fn negative_twist_is_clamped_to_the_minimum() {
        let q = Quat::from_rotation_y(-1.0);
        let out = clamp(q, Quat::IDENTITY, Vec3::Y, PI, -0.25, 0.5);
        let (_, t) = swing_twist(out, Vec3::Y);
        assert!((twist_angle(t, Vec3::Y) + 0.25).abs() < 1e-4);
    }
}
