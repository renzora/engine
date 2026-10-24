//! Left and right: which bone mirrors which, and mirroring a pose.

use bevy::math::{Quat, Vec3};

/// The bone on the other side, by the naming conventions rigs use:
/// `LeftArm`/`RightArm` (Mixamo, Unreal), `arm.L`/`arm.R` and `arm_l`/`arm_r`
/// (Blender, this crate's face rig). `None` for a centre bone.
pub fn mirror_name(name: &str) -> Option<String> {
    for (a, b) in [("Left", "Right"), ("left", "right"), ("LEFT", "RIGHT")] {
        if name.contains(a) {
            return Some(name.replacen(a, b, 1));
        }
        if name.contains(b) {
            return Some(name.replacen(b, a, 1));
        }
    }
    for (a, b) in [(".L", ".R"), ("_L", "_R"), (".l", ".r"), ("_l", "_r")] {
        if let Some(stem) = name.strip_suffix(a) {
            return Some(format!("{stem}{b}"));
        }
        if let Some(stem) = name.strip_suffix(b) {
            return Some(format!("{stem}{a}"));
        }
    }
    None
}

/// Which side of the body a bone is on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Left,
    Right,
}

/// A bone's side, by the same conventions as [`mirror_name`]. `None` for a
/// centre bone.
pub fn side(name: &str) -> Option<Side> {
    let lower = name.to_ascii_lowercase();
    if lower.contains("left") {
        return Some(Side::Left);
    }
    if lower.contains("right") {
        return Some(Side::Right);
    }
    if name.ends_with(".L") || name.ends_with("_L") || name.ends_with(".l") || name.ends_with("_l") {
        return Some(Side::Left);
    }
    if name.ends_with(".R") || name.ends_with("_R") || name.ends_with(".r") || name.ends_with("_r") {
        return Some(Side::Right);
    }
    None
}

/// Reflect a rotation through the plane facing `normal` (the body's
/// left-right axis), the rotation a mirror image of the motion would have.
pub fn mirror_rotation(q: Quat, normal: Vec3) -> Quat {
    // A reflection keeps the rotation's axis component along the plane's
    // normal and flips the rest, with the angle unchanged.
    let n = normal.normalize_or_zero();
    let axis = Vec3::new(q.x, q.y, q.z);
    let along = n * axis.dot(n);
    let mirrored = along - (axis - along);
    Quat::from_xyzw(mirrored.x, mirrored.y, mirrored.z, q.w).normalize()
}

/// Reflect a point (or offset) through the plane through `origin` facing `normal`.
pub fn mirror_point(p: Vec3, origin: Vec3, normal: Vec3) -> Vec3 {
    let n = normal.normalize_or_zero();
    p - n * (2.0 * (p - origin).dot(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_mirror_both_ways() {
        assert_eq!(mirror_name("mixamorig:LeftForeArm").as_deref(), Some("mixamorig:RightForeArm"));
        assert_eq!(mirror_name("mixamorig_rightforearm").as_deref(), Some("mixamorig_leftforearm"));
        assert_eq!(mirror_name("arm.L").as_deref(), Some("arm.R"));
        assert_eq!(mirror_name("face_lid_upper_r").as_deref(), Some("face_lid_upper_l"));
        assert_eq!(mirror_name("mixamorig:Spine"), None);
    }

    #[test]
    fn sides_follow_the_names() {
        assert_eq!(side("mixamorig_leftforearm"), Some(Side::Left));
        assert_eq!(side("face_lid_upper_r"), Some(Side::Right));
        assert_eq!(side("mixamorig:Spine"), None);
    }

    #[test]
    fn a_mirrored_turn_goes_the_other_way() {
        // Turning toward +X (about Y) mirrors across the X plane to turning toward -X.
        let q = Quat::from_rotation_y(0.5);
        let m = mirror_rotation(q, Vec3::X);
        let v = m * Vec3::Z;
        assert!(v.x < 0.0 && (v.x + (q * Vec3::Z).x).abs() < 1e-5);
        // A nod (about X) is its own mirror.
        let nod = Quat::from_rotation_x(0.4);
        assert!(mirror_rotation(nod, Vec3::X).angle_between(nod) < 1e-5);
    }
}
