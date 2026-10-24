//! The solvers, as plain math on positions and rotations.
//!
//! Nothing in here touches the ECS. Every solver takes world-space vectors and
//! quaternions and returns world-space results, so each one is unit-tested on
//! plain numbers and the systems in [`crate::apply`] only have to read a pose
//! out of the hierarchy and write the answer back.

pub mod aim;
pub mod foot;
pub mod limit;
pub mod spring;
pub mod two_bone;

use bevy::math::{Quat, Vec3};

/// Rotation about `axis` (unit) that best carries `from` onto `to`, measured in
/// the plane perpendicular to `axis`. `None` when either vector is (nearly)
/// parallel to the axis, where the twist is undefined and any answer would be a
/// guess that flips from frame to frame.
///
/// A signed `atan2` rather than `Quat::from_rotation_arc`: for two vectors 180
/// degrees apart the arc picks *some* perpendicular axis, which is not
/// necessarily `axis`, and the limb would then swing off its target.
pub fn twist_about(axis: Vec3, from: Vec3, to: Vec3) -> Option<Quat> {
    let f = from - axis * from.dot(axis);
    let t = to - axis * to.dot(axis);
    if f.length_squared() < 1e-10 || t.length_squared() < 1e-10 {
        return None;
    }
    let (f, t) = (f.normalize(), t.normalize());
    let angle = f.cross(t).dot(axis).atan2(f.dot(t));
    Some(Quat::from_axis_angle(axis, angle))
}

/// Exponential approach factor for `speed` per second over `dt`, in 0..1.
/// Framerate independent, unlike a fixed lerp fraction per frame: two frames of
/// `dt` land exactly where one frame of `2 * dt` does.
pub fn approach(speed: f32, dt: f32) -> f32 {
    if speed <= 0.0 {
        1.0
    } else {
        1.0 - (-speed * dt.max(0.0)).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twist_about_is_signed_and_handles_opposites() {
        let q = twist_about(Vec3::Y, Vec3::X, Vec3::NEG_X).unwrap();
        assert!((q * Vec3::X).distance(Vec3::NEG_X) < 1e-5);
        // The rotation stays about the axis: Y is untouched.
        assert!((q * Vec3::Y).distance(Vec3::Y) < 1e-5);
        let q = twist_about(Vec3::Y, Vec3::X, Vec3::NEG_Z).unwrap();
        assert!((q * Vec3::X).distance(Vec3::NEG_Z) < 1e-5);
        assert!(twist_about(Vec3::Y, Vec3::Y, Vec3::X).is_none());
    }

    #[test]
    fn approach_is_framerate_independent() {
        let one = approach(5.0, 0.1);
        let two = 1.0 - (1.0 - approach(5.0, 0.05)).powi(2);
        assert!((one - two).abs() < 1e-6);
    }
}
