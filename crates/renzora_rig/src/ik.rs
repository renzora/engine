//! Two-bone inverse kinematics: an arm or a leg reaching for a point.
//!
//! Given the shoulder (or hip), elbow (or knee) and wrist (or ankle) in world
//! space and a target for the wrist, find how much to turn the upper and lower
//! bones. The lengths stay as they are; a target out of reach is reached for
//! as far as the limb goes. The bend keeps to the plane the limb is already
//! bent in, so an elbow does not flip, with `pole` breaking the tie when the
//! limb is dead straight.

use bevy::math::{Quat, Vec3};

/// World-space turns for the upper and lower bones. Apply each to its bone's
/// world rotation (`turn * rotation`); the lower one already includes the
/// upper's.
pub fn solve(a: Vec3, b: Vec3, c: Vec3, target: Vec3, pole: Vec3) -> (Quat, Quat) {
    let eps = 1e-5;
    let lab = (b - a).length();
    let lcb = (c - b).length();
    if lab < eps || lcb < eps {
        return (Quat::IDENTITY, Quat::IDENTITY);
    }
    let lat = (target - a).length().clamp(eps, (lab + lcb) * 0.9999);

    // Interior angles now, and the ones that put the wrist `lat` from the
    // shoulder.
    let angle = |u: Vec3, v: Vec3| u.normalize().dot(v.normalize()).clamp(-1.0, 1.0).acos();
    let ac_ab_0 = angle(c - a, b - a);
    let ba_bc_0 = angle(a - b, c - b);
    let ac_ab_1 = ((lcb * lcb - lab * lab - lat * lat) / (-2.0 * lab * lat)).clamp(-1.0, 1.0).acos();
    let ba_bc_1 = ((lat * lat - lab * lab - lcb * lcb) / (-2.0 * lab * lcb)).clamp(-1.0, 1.0).acos();

    // The bend plane's normal: the limb's own, or from the pole when straight.
    let mut axis = (c - a).cross(b - a);
    if axis.length_squared() < 1e-10 {
        axis = (c - a).cross(pole - a);
    }
    let axis = axis.try_normalize().unwrap_or(Vec3::X);

    let r0 = Quat::from_axis_angle(axis, ac_ab_1 - ac_ab_0);
    let r1 = Quat::from_axis_angle(axis, ba_bc_1 - ba_bc_0);
    // Where the wrist lands after bending, then swing the whole limb onto the target.
    let b1 = a + r0 * (b - a);
    let c1 = b1 + r0 * (r1 * (c - b));
    let r2 = Quat::from_rotation_arc((c1 - a).normalize(), (target - a).normalize());
    let upper = (r2 * r0).normalize();
    let lower = (r2 * r0 * r1).normalize();
    (upper, lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reach(a: Vec3, b: Vec3, c: Vec3, t: Vec3) -> Vec3 {
        let (up, lo) = solve(a, b, c, t, b + Vec3::Z);
        let b2 = a + up * (b - a);
        b2 + lo * (c - b)
    }

    #[test]
    fn a_bent_arm_reaches_a_point_in_range() {
        let (a, b, c) = (Vec3::ZERO, Vec3::new(0.3, -0.05, 0.05), Vec3::new(0.55, 0.0, 0.1));
        for t in [Vec3::new(0.2, -0.3, 0.2), Vec3::new(0.4, 0.2, -0.1), Vec3::new(0.1, 0.1, 0.4)] {
            let got = reach(a, b, c, t);
            assert!(got.distance(t) < 1e-3, "aimed at {t:?}, reached {got:?}");
        }
    }

    #[test]
    fn bone_lengths_are_kept() {
        let (a, b, c) = (Vec3::ZERO, Vec3::new(0.0, -0.4, 0.05), Vec3::new(0.0, -0.8, 0.0));
        let (up, lo) = solve(a, b, c, Vec3::new(0.2, -0.5, 0.3), Vec3::Z);
        let b2 = a + up * (b - a);
        let c2 = b2 + lo * (c - b);
        assert!(((b2 - a).length() - 0.4031).abs() < 1e-3);
        assert!(((c2 - b2).length() - (c - b).length()).abs() < 1e-4);
    }

    #[test]
    fn out_of_reach_points_the_limb_at_the_target() {
        let (a, b, c) = (Vec3::ZERO, Vec3::new(0.3, -0.02, 0.0), Vec3::new(0.6, 0.0, 0.0));
        let t = Vec3::new(0.0, 5.0, 0.0);
        let got = reach(a, b, c, t);
        assert!(got.normalize().dot(Vec3::Y) > 0.99);
    }
}
