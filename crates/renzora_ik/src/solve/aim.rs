//! Aim (look-at): turn a bone so one of its axes points at a target.
//!
//! The limit is measured from the direction the *animation* aims the axis,
//! not from some fixed rest pose. A head turning toward a target should turn
//! at most so far from where the clip has it looking, so a character glancing
//! back over its shoulder in a clip can still look a little further, while a
//! target directly behind an idle character is followed only to the edge of
//! the cone.

use super::twist_about;
use bevy::math::{Quat, Vec3};

/// The world rotation that aims `aim_axis` (in the bone's local space) from
/// `position` at `target`, starting from the animated `rotation`.
///
/// The turn is clamped to `max_angle` radians away from the animated aim. With
/// a non-zero `world_up` the bone is also rolled about the aim so its local
/// `up_axis` leans as close to `world_up` as the aim allows (a turret that must
/// stay level); with zero the animated roll is kept, the smallest turn that
/// aims, which is what a head wants.
pub fn aim(
    rotation: Quat,
    position: Vec3,
    target: Vec3,
    aim_axis: Vec3,
    up_axis: Vec3,
    world_up: Vec3,
    max_angle: f32,
) -> Quat {
    let Some(aim_local) = aim_axis.try_normalize() else {
        return rotation;
    };
    let Some(wanted) = (target - position).try_normalize() else {
        return rotation;
    };
    let current = rotation * aim_local;
    let dir = clamp_direction(current, wanted, max_angle);
    let mut out = Quat::from_rotation_arc(current, dir) * rotation;

    if let (Some(up_local), Some(world_up)) = (up_axis.try_normalize(), world_up.try_normalize()) {
        if let Some(roll) = twist_about(dir, out * up_local, world_up) {
            out = roll * out;
        }
    }
    out.normalize()
}

/// `wanted`, pulled back toward `from` so the two are at most `max_angle`
/// radians apart. Both unit length.
pub fn clamp_direction(from: Vec3, wanted: Vec3, max_angle: f32) -> Vec3 {
    let max_angle = max_angle.max(0.0);
    let angle = from.dot(wanted).clamp(-1.0, 1.0).acos();
    if angle <= max_angle {
        return wanted;
    }
    let axis = from.cross(wanted);
    // Directly behind: any perpendicular turns the same distance, so pick a
    // stable one rather than none.
    let axis = axis.try_normalize().unwrap_or_else(|| from.any_orthonormal_vector());
    Quat::from_axis_angle(axis, max_angle) * from
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    #[test]
    fn aims_the_axis_at_a_target_within_the_limit() {
        let rot = Quat::IDENTITY;
        let target = Vec3::new(1.0, 0.5, 2.0);
        let out = aim(rot, Vec3::ZERO, target, Vec3::Z, Vec3::Y, Vec3::ZERO, PI);
        assert!((out * Vec3::Z).distance(target.normalize()) < 1e-5);
    }

    #[test]
    fn a_target_beyond_the_limit_is_followed_to_the_cone_edge() {
        let rot = Quat::IDENTITY;
        // Ninety degrees to the side, limit forty-five.
        let out = aim(rot, Vec3::ZERO, Vec3::X, Vec3::Z, Vec3::Y, Vec3::ZERO, FRAC_PI_4);
        let dir = out * Vec3::Z;
        let angle = dir.dot(Vec3::Z).acos();
        assert!((angle - FRAC_PI_4).abs() < 1e-4, "turned {angle}");
        // And toward the target, not some other way.
        assert!(dir.x > 0.5 && dir.y.abs() < 1e-4);
    }

    #[test]
    fn the_limit_is_relative_to_the_animated_aim() {
        // The clip already has the head turned 90 degrees left.
        let rot = Quat::from_rotation_y(FRAC_PI_2);
        let out = aim(rot, Vec3::ZERO, Vec3::new(1.0, 0.0, -1.0), Vec3::Z, Vec3::Y, Vec3::ZERO, FRAC_PI_4);
        let dir = out * Vec3::Z;
        // Started at +X, limited to 45 degrees off it.
        assert!((dir.dot(Vec3::X).acos() - FRAC_PI_4).abs() < 1e-4);
    }

    #[test]
    fn world_up_keeps_the_bone_level() {
        // Animated with a roll; a level turret must lose it.
        let rot = Quat::from_rotation_z(0.6);
        let out = aim(rot, Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0), Vec3::Z, Vec3::Y, Vec3::Y, PI);
        assert!((out * Vec3::Z).distance(Vec3::Z) < 1e-5);
        assert!((out * Vec3::Y).distance(Vec3::Y) < 1e-4);
        // Without world up the roll is kept.
        let kept = aim(rot, Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0), Vec3::Z, Vec3::Y, Vec3::ZERO, PI);
        assert!((kept * Vec3::Y).distance(rot * Vec3::Y) < 1e-4);
    }

    #[test]
    fn a_target_on_the_bone_changes_nothing() {
        let rot = Quat::from_rotation_x(0.3);
        assert_eq!(aim(rot, Vec3::ONE, Vec3::ONE, Vec3::Z, Vec3::Y, Vec3::Y, PI), rot);
    }
}
