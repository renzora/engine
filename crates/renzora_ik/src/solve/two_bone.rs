//! Analytic two-bone IK: an arm or a leg reaching for a point.
//!
//! The law-of-cosines core is the same one the editor's rigging tools use
//! (`renzora_rig::ik`), copied rather than depended on: that crate is editor
//! tooling and this one ships in every game, and the two have different jobs on
//! top of the core. The rig's solver only consults the pole when the limb is
//! dead straight; a runtime constraint has to honour a pole *always*, because
//! "the knee points at this marker" is what the user asked for.

use super::twist_about;
use bevy::math::{Quat, Vec3};

/// World-space turns for the upper and lower bones, to apply as
/// `turn * world_rotation`. The lower turn already includes the upper's.
///
/// `a`, `b`, `c` are the root, mid and end joints (shoulder, elbow, wrist). The
/// bone lengths are kept; a target out of reach is reached for as far as the
/// limb goes, pointing straight at it. With `pole` the limb's bend plane is
/// turned about the root-to-end axis until the mid joint faces the pole;
/// without it the plane the limb is already bent in is kept, so an elbow never
/// flips on its own.
pub fn solve(a: Vec3, b: Vec3, c: Vec3, target: Vec3, pole: Option<Vec3>) -> (Quat, Quat) {
    let eps = 1e-5;
    let lab = (b - a).length();
    let lcb = (c - b).length();
    if lab < eps || lcb < eps || (target - a).length() < eps {
        return (Quat::IDENTITY, Quat::IDENTITY);
    }
    // Just short of full extension: a perfectly straight limb has no bend plane
    // left for the next frame to keep.
    let lat = (target - a).length().clamp(eps, (lab + lcb) * 0.9999);

    let angle = |u: Vec3, v: Vec3| u.normalize().dot(v.normalize()).clamp(-1.0, 1.0).acos();
    let ac_ab_0 = angle(c - a, b - a);
    let ba_bc_0 = angle(a - b, c - b);
    let ac_ab_1 = ((lcb * lcb - lab * lab - lat * lat) / (-2.0 * lab * lat)).clamp(-1.0, 1.0).acos();
    let ba_bc_1 = ((lat * lat - lab * lab - lcb * lcb) / (-2.0 * lab * lcb)).clamp(-1.0, 1.0).acos();

    // The bend plane: the limb's own, else the pole's, else anything stable.
    let mut axis = (c - a).cross(b - a);
    if axis.length_squared() < 1e-10 {
        if let Some(p) = pole {
            axis = (c - a).cross(p - a);
        }
    }
    if axis.length_squared() < 1e-10 {
        axis = (c - a).any_orthonormal_vector();
    }
    let axis = axis.normalize();

    let r0 = Quat::from_axis_angle(axis, ac_ab_1 - ac_ab_0);
    let r1 = Quat::from_axis_angle(axis, ba_bc_1 - ba_bc_0);
    let b1 = a + r0 * (b - a);
    let c1 = b1 + r0 * (r1 * (c - b));
    let r2 = Quat::from_rotation_arc((c1 - a).normalize(), (target - a).normalize());
    let mut upper = (r2 * r0).normalize();
    let mut lower = (r2 * r0 * r1).normalize();

    if let Some(p) = pole {
        // The end now sits on the root-to-target line, so a turn about that
        // line moves only the mid joint, which is exactly the pole's say.
        let reach = (target - a).normalize();
        let mid = upper * (b - a);
        if let Some(twist) = twist_about(reach, mid, p - a) {
            upper = (twist * upper).normalize();
            lower = (twist * lower).normalize();
        }
    }
    (upper, lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(a: Vec3, b: Vec3, c: Vec3, t: Vec3, pole: Option<Vec3>) -> (Vec3, Vec3) {
        let (up, lo) = solve(a, b, c, t, pole);
        let b2 = a + up * (b - a);
        (b2, b2 + lo * (c - b))
    }

    #[test]
    fn reaches_points_in_range() {
        let (a, b, c) = (Vec3::ZERO, Vec3::new(0.3, -0.05, 0.05), Vec3::new(0.55, 0.0, 0.1));
        for t in [Vec3::new(0.2, -0.3, 0.2), Vec3::new(0.4, 0.2, -0.1), Vec3::new(0.1, 0.1, 0.4)] {
            let (_, got) = pose(a, b, c, t, None);
            assert!(got.distance(t) < 1e-3, "aimed at {t:?}, reached {got:?}");
            let (_, got) = pose(a, b, c, t, Some(Vec3::new(0.0, 1.0, 0.0)));
            assert!(got.distance(t) < 1e-3, "with pole, aimed at {t:?}, reached {got:?}");
        }
    }

    #[test]
    fn keeps_bone_lengths() {
        let (a, b, c) = (Vec3::ZERO, Vec3::new(0.0, -0.4, 0.05), Vec3::new(0.0, -0.8, 0.0));
        let (b2, c2) = pose(a, b, c, Vec3::new(0.2, -0.5, 0.3), Some(Vec3::Z));
        assert!(((b2 - a).length() - (b - a).length()).abs() < 1e-4);
        assert!(((c2 - b2).length() - (c - b).length()).abs() < 1e-4);
    }

    #[test]
    fn unreachable_target_straightens_toward_it() {
        let (a, b, c) = (Vec3::ZERO, Vec3::new(0.3, -0.02, 0.0), Vec3::new(0.6, 0.0, 0.0));
        let t = Vec3::new(0.0, 5.0, 0.0);
        let (b2, got) = pose(a, b, c, t, None);
        assert!(got.normalize().dot(Vec3::Y) > 0.999);
        // Nearly full length: the limb is stretched as far as it goes.
        assert!(got.length() > 0.59);
        assert!(b2.normalize().dot(Vec3::Y) > 0.99);
    }

    #[test]
    fn pole_decides_which_way_the_mid_joint_bends() {
        // A leg bent slightly forward (+Z), told to point its knee sideways (+X).
        let (a, b, c) = (Vec3::ZERO, Vec3::new(0.0, -0.45, 0.05), Vec3::new(0.0, -0.9, 0.0));
        let t = Vec3::new(0.0, -0.7, 0.0);
        for pole in [Vec3::new(1.0, -0.45, 0.0), Vec3::new(0.0, -0.45, -1.0), Vec3::new(-1.0, -0.45, 0.3)] {
            let (knee, foot) = pose(a, b, c, t, Some(pole));
            assert!(foot.distance(t) < 1e-3);
            let axis = (t - a).normalize();
            let flat = |v: Vec3| (v - axis * v.dot(axis)).normalize();
            let facing = flat(knee - a).dot(flat(pole - a));
            assert!(facing > 0.999, "knee {knee:?} does not face pole {pole:?} ({facing})");
        }
    }

    #[test]
    fn straight_limb_uses_the_pole_for_its_plane() {
        let (a, b, c) = (Vec3::ZERO, Vec3::new(0.0, -0.5, 0.0), Vec3::new(0.0, -1.0, 0.0));
        let (knee, foot) = pose(a, b, c, Vec3::new(0.0, -0.6, 0.0), Some(Vec3::new(0.0, -0.5, 1.0)));
        assert!(foot.distance(Vec3::new(0.0, -0.6, 0.0)) < 1e-3);
        assert!(knee.z > 0.1, "knee should bend toward the pole, got {knee:?}");
    }
}
