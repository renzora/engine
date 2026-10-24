//! A humanoid skeleton from seven points, skinned to the body.
//!
//! The bones are Mixamo's, by name and by orientation, which is the point:
//! every Mixamo clip, and every clip made for a Mixamo rig, then plays on the
//! result unchanged. Clips bind bones by name (see `renzora_animation`'s
//! `bone_target`), and a Mixamo clip's rotations are written relative to
//! Mixamo's own bone orientations, so a skeleton with the right names and
//! different orientations would play the clip with its limbs twisted.
//!
//! [`TEMPLATE`] is a Mixamo rig's rest pose (model space, T-pose, facing +Z,
//! character's left at +X). Each generated bone takes its template orientation
//! turned by the swing from the template's bone direction to the body's, so an
//! A-posed body gets bones along its own arms with Mixamo's twist. Positions
//! come from the markers, and where there is no marker (spine, neck, toes),
//! from the template scaled to the body.
//!
//! Weights are measured across the skin (`weights::surface_raw`): each vertex
//! goes to the bones nearest it along the surface, falling off steeply, so an
//! arm resting against the ribs or a finger beside the next does not pull
//! skin it only touches through the air. They are also kept to the vertex's
//! own side of the body, away from the middle, so an arm resting against the
//! hip does not pull skin from the leg.

use bevy::math::{Quat, Vec3};

use crate::fingers;
use crate::glb::{Glb, GlbError};
use crate::skin::{self, Applied, Bone};
use crate::weights;

/// The points the user places, in the model's space. One side is enough: the
/// other is mirrored across the body's centre (the groin's left-right
/// position).
#[derive(Clone, Copy, Debug)]
pub struct BodyMarkers {
    pub chin: Vec3,
    pub shoulder: Vec3,
    pub elbow: Vec3,
    pub wrist: Vec3,
    pub groin: Vec3,
    pub knee: Vec3,
    pub ankle: Vec3,
}

/// `(name without the mixamorig: prefix, model-space rotation xyzw, position)`
/// from a Mixamo rig in its rest pose.
pub const TEMPLATE: &[(&str, [f32; 4], [f32; 3])] = &[
    ("Hips", [0.000000, -0.000002, 0.000044, 1.000000], [0.0, 0.9979, 0.0]),
    ("Spine", [-0.060730, -0.000004, 0.000050, 0.998154], [0.0, 1.0972, -0.0123]),
    ("Spine1", [-0.060534, -0.000005, 0.000040, 0.998166], [0.0, 1.2136, -0.0265]),
    ("Spine2", [-0.002828, 0.0, 0.000044, 0.999996], [0.0, 1.3472, -0.0428]),
    ("Neck", [0.0, 0.0, 0.000044, 1.0], [0.0, 1.4975, -0.0348]),
    ("Head", [0.0, 0.0, 0.000044, 1.0], [0.0, 1.6008, -0.0034]),
    ("HeadTop_End", [0.0, 0.0, 0.000044, 1.0], [-0.0001, 1.7855, 0.0630]),
    ("LeftShoulder", [-0.452566, -0.543308, 0.552658, -0.441100], [0.0610, 1.4383, -0.0357]),
    ("LeftArm", [-0.499978, -0.500022, 0.499978, -0.500022], [0.1876, 1.4357, -0.0617]),
    ("LeftForeArm", [-0.499978, -0.500022, 0.499978, -0.500022], [0.4616, 1.4357, -0.0617]),
    ("LeftHand", [-0.499978, -0.500022, 0.499978, -0.500022], [0.7378, 1.4357, -0.0617]),
    ("LeftUpLeg", [-0.000019, 0.006350, -0.999975, 0.003013], [0.0912, 0.9314, -0.0006]),
    ("LeftLeg", [0.000072, 0.024463, -0.999697, -0.002909], [0.0937, 0.5254, -0.0057]),
    ("LeftFoot", [0.013460, -0.519860, -0.853859, 0.022108], [0.0912, 0.1049, -0.0263]),
    ("LeftToeBase", [-0.001649, -0.700789, -0.713273, -0.011579], [0.0950, 0.0328, 0.1134]),
    ("LeftToe_End", [-0.001649, -0.700789, -0.713273, -0.011579], [0.0936, 0.0311, 0.2133]),
    ("RightShoulder", [-0.452616, 0.543267, -0.552698, -0.441050], [-0.0611, 1.4383, -0.0357]),
    ("RightArm", [-0.500022, 0.499978, -0.500022, -0.499978], [-0.1876, 1.4356, -0.0617]),
    ("RightForeArm", [-0.500022, 0.499978, -0.500022, -0.499978], [-0.4617, 1.4356, -0.0617]),
    ("RightHand", [-0.500022, 0.499978, -0.500022, -0.499978], [-0.7378, 1.4356, -0.0617]),
    ("RightUpLeg", [-0.000019, -0.006334, 0.999975, 0.003014], [-0.0912, 0.9314, -0.0006]),
    ("RightLeg", [0.000071, -0.024480, 0.999696, -0.002908], [-0.0937, 0.5254, -0.0057]),
    ("RightFoot", [0.013460, 0.519860, 0.853859, 0.022108], [-0.0912, 0.1049, -0.0263]),
    ("RightToeBase", [-0.001362, 0.700801, 0.713266, -0.011278], [-0.0950, 0.0328, 0.1134]),
    ("RightToe_End", [-0.001362, 0.700801, 0.713266, -0.011278], [-0.0936, 0.0311, 0.2133]),
];

/// A finger bone's finger and joint number: `("LeftHandIndex", 2)` for
/// `LeftHandIndex2`.
fn finger_joint(name: &str) -> Option<(&str, u32)> {
    if !name.contains("Hand") {
        return None;
    }
    let digit = name.chars().last()?.to_digit(10)?;
    Some((&name[..name.len() - 1], digit))
}

/// Each template bone's parent, by name.
fn parent_of(name: &str) -> Option<&'static str> {
    let side = |n: &str| if n.starts_with("Left") { "Left" } else { "Right" };
    if let Some((finger, joint)) = finger_joint(name) {
        let parent = if joint == 1 { format!("{}Hand", side(name)) } else { format!("{finger}{}", joint - 1) };
        return all_templates().map(|t| t.0).find(|t| *t == parent);
    }
    Some(match name {
        "Hips" => return None,
        "Spine" => "Hips",
        "Spine1" => "Spine",
        "Spine2" => "Spine1",
        "Neck" => "Spine2",
        "Head" => "Neck",
        "HeadTop_End" => "Head",
        n if n.ends_with("Shoulder") => "Spine2",
        n if n.ends_with("UpLeg") => "Hips",
        n => {
            let rest = n.trim_start_matches(side(n));
            let parent = match rest {
                "Arm" => "Shoulder",
                "ForeArm" => "Arm",
                "Hand" => "ForeArm",
                "Leg" => "UpLeg",
                "Foot" => "Leg",
                "ToeBase" => "Foot",
                "Toe_End" => "ToeBase",
                _ => return None,
            };
            return TEMPLATE.iter().map(|t| t.0).find(|t| *t == format!("{}{parent}", side(n)));
        }
    })
}

/// The child a bone points at, which sets its direction. `None` for a leaf.
/// The hips point up the spine, not down a leg: that is Mixamo's hips.
fn child_of(name: &str) -> Option<String> {
    if let Some((finger, joint)) = finger_joint(name) {
        return (joint < 4).then(|| format!("{finger}{}", joint + 1));
    }
    let (side, part) = match name.strip_prefix("Left") {
        Some(rest) => ("Left", rest),
        None => match name.strip_prefix("Right") {
            Some(rest) => ("Right", rest),
            None => ("", name),
        },
    };
    let child = match part {
        "Hips" => "Spine",
        "Spine" => "Spine1",
        "Spine1" => "Spine2",
        "Spine2" => "Neck",
        "Neck" => "Head",
        "Head" => "HeadTop_End",
        "Shoulder" => "Arm",
        "Arm" => "ForeArm",
        "ForeArm" => "Hand",
        // When the fingers were found: the hand then lies along the hand
        // rather than carrying on the forearm's line.
        "Hand" => "HandMiddle1",
        "UpLeg" => "Leg",
        "Leg" => "Foot",
        "Foot" => "ToeBase",
        "ToeBase" => "Toe_End",
        _ => return None,
    };
    Some(format!("{side}{child}"))
}

/// The body's template bones, then the fingers'.
fn all_templates() -> impl Iterator<Item = &'static (&'static str, [f32; 4], [f32; 3])> {
    TEMPLATE.iter().chain(fingers::TEMPLATE)
}

fn template(name: &str) -> Option<(Quat, Vec3)> {
    all_templates().find(|t| t.0 == name).map(|t| (Quat::from_array(t.1).normalize(), Vec3::from_array(t.2)))
}

/// The prefix every bone name gets, matching Mixamo's own.
pub const PREFIX: &str = "mixamorig:";

/// Positions for every template bone, from the markers.
fn positions(m: &BodyMarkers) -> Vec<(&'static str, Vec3)> {
    let centre_x = m.groin.x;
    let mirror = |p: Vec3| Vec3::new(2.0 * centre_x - p.x, p.y, p.z);
    // The marked side, and the other.
    let left_marked = m.shoulder.x > centre_x;
    let side = |p: Vec3, left: bool| if left == left_marked { p } else { mirror(p) };

    let t = |n: &str| template(n).map(|t| t.1).unwrap_or(Vec3::ZERO);
    let torso_k = ((m.shoulder.y - m.groin.y) / (t("LeftArm").y - t("Hips").y)).max(1e-3);
    let leg_k = ((m.groin.y - m.ankle.y) / (t("Hips").y - t("LeftFoot").y)).max(1e-3);
    let along_torso = |n: &str| {
        let d = t(n) - t("Hips");
        Vec3::new(m.groin.x + d.x * torso_k, m.groin.y + d.y * torso_k, m.groin.z + d.z * torso_k)
    };
    let mut out: Vec<(&'static str, Vec3)> = Vec::new();
    for (name, ..) in TEMPLATE {
        let left = name.starts_with("Left");
        let p = match *name {
            "Hips" => m.groin,
            "Spine" | "Spine1" | "Spine2" | "Neck" => along_torso(name),
            "Head" => {
                let h = along_torso("Head");
                Vec3::new(h.x, h.y.max(m.chin.y + 0.03 * torso_k), h.z)
            }
            "HeadTop_End" => {
                let head = out.iter().find(|(n, _)| *n == "Head").map(|(_, p)| *p).unwrap_or(m.chin);
                head + (t("HeadTop_End") - t("Head")) * torso_k
            }
            n if n.ends_with("Shoulder") => {
                let neck = along_torso("Neck");
                let arm = side(m.shoulder, left);
                Vec3::new(neck.x + (arm.x - neck.x) * 0.33, arm.y, (neck.z + arm.z) * 0.5)
            }
            n if n.ends_with("ForeArm") => side(m.elbow, left),
            n if n.ends_with("Arm") => side(m.shoulder, left),
            n if n.ends_with("Hand") => side(m.wrist, left),
            n if n.ends_with("UpLeg") => {
                let knee = side(m.knee, left);
                Vec3::new(knee.x, m.groin.y - (t("Hips").y - t("LeftUpLeg").y) * leg_k, m.groin.z)
            }
            n if n.ends_with("Leg") => side(m.knee, left),
            n if n.ends_with("Foot") => side(m.ankle, left),
            n if n.ends_with("ToeBase") => {
                let a = side(m.ankle, left);
                let d = t("LeftToeBase") - t("LeftFoot");
                a + Vec3::new(0.0, d.y * leg_k, d.z * leg_k)
            }
            n if n.ends_with("Toe_End") => {
                let a = side(m.ankle, left);
                let d = t("LeftToe_End") - t("LeftFoot");
                a + Vec3::new(0.0, d.y * leg_k, d.z * leg_k)
            }
            _ => continue,
        };
        out.push((name, p));
    }
    out
}

/// The skeleton, placed, in template order (index 0 is the hips).
pub fn bones(m: &BodyMarkers) -> Vec<Bone> {
    bones_with(m, &[])
}

/// [`bones`] plus finger bones at `fingers` (from [`fingers::detect`]).
fn bones_with(m: &BodyMarkers, fingers: &[(&'static str, Vec3)]) -> Vec<Bone> {
    let mut pos = positions(m);
    pos.extend_from_slice(fingers);
    let at = |n: &str| pos.iter().find(|(p, _)| *p == n).map(|(_, v)| *v);
    let mut bones: Vec<Bone> = Vec::new();
    for (name, _, _) in all_templates() {
        let Some((t_rot, t_pos)) = template(name) else { continue };
        let Some(head) = at(name) else { continue };
        // Turn the template orientation by the swing from its bone direction
        // to this body's, so the bone lies along the limb with Mixamo's twist.
        let rotation = match child_of(name).and_then(|c| Some((template(&c)?.1, at(&c)?))) {
            Some((t_child, child)) => match ((t_child - t_pos).try_normalize(), (child - head).try_normalize()) {
                (Some(from), Some(to)) => (Quat::from_rotation_arc(from, to) * t_rot).normalize(),
                _ => t_rot,
            },
            None => {
                // A leaf takes its parent's swing.
                let parent = parent_of(name).and_then(|p| bones.iter().find(|b| b.name == format!("{PREFIX}{p}")));
                match (parent, parent_of(name).and_then(template)) {
                    (Some(pb), Some((pt_rot, _))) => (pb.rotation * pt_rot.inverse() * t_rot).normalize(),
                    _ => t_rot,
                }
            }
        };
        let parent = parent_of(name).and_then(|p| bones.iter().position(|b| b.name == format!("{PREFIX}{p}")));
        bones.push(Bone { name: format!("{PREFIX}{name}"), parent, head, rotation });
    }
    bones
}

/// Envelope weights for one vertex: inverse distance to each bone's segment,
/// steeply. For a mesh whose triangles could not be read; every other mesh
/// is weighted across its surface (`weights::surface_raw`).
fn envelope(bones: &[Bone], tails: &[Vec3], scale: f32, p: Vec3) -> Vec<(usize, f32)> {
    bones
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let d = distance_to_segment(p, b.head, tails[i]) / scale.max(1e-6);
            (i, 1.0 / (d * d + 1e-4).powi(3))
        })
        .collect()
}

/// Keep a vertex's weights to its own side of the body: off to one side of
/// the middle, the other side's limbs do not reach.
fn sided(bones: &[Bone], centre_x: f32, scale: f32, p: Vec3, mut raw: Vec<(usize, f32)>) -> Vec<(usize, f32)> {
    let vert_side = p.x - centre_x;
    for (i, w) in raw.iter_mut() {
        let b = &bones[*i];
        let one_side = b.name.contains("Left") || b.name.contains("Right");
        if one_side && vert_side.abs() > 0.03 * scale && vert_side.signum() != (b.head.x - centre_x).signum() {
            *w *= 1e-4;
        }
    }
    raw
}

/// Where each bone's segment ends: at the child it points at, or a little past
/// a leaf along its own direction.
fn tails(bones: &[Bone], scale: f32) -> Vec<Vec3> {
    bones
        .iter()
        .map(|b| {
            let short = b.name.trim_start_matches(PREFIX);
            child_of(short)
                .and_then(|c| bones.iter().find(|o| o.name == format!("{PREFIX}{c}")))
                .map(|c| c.head)
                .unwrap_or_else(|| b.head + b.rotation * Vec3::Y * scale * 0.08)
        })
        .collect()
}

fn distance_to_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() < 1e-12 { 0.0 } else { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) };
    (p - (a + ab * t)).length()
}

/// One vertex's unnormalised `(bone, weight)` pairs.
type RawWeights = Vec<(usize, f32)>;

/// Rig the body in `glb` from `markers` (model space).
pub fn build(glb: &mut Glb, markers: &BodyMarkers) -> Result<Applied, GlbError> {
    let centre_x = markers.groin.x;
    let tris = fingers::triangles(glb)?;
    let pos = positions(markers);
    let at = |n: &str| pos.iter().find(|(p, _)| *p == n).map(|(_, v)| *v);
    let mut found = Vec::new();
    for side in ["Left", "Right"] {
        if let (Some(elbow), Some(wrist)) = (at(&format!("{side}ForeArm")), at(&format!("{side}Hand"))) {
            found.extend(fingers::detect(&tris, side, elbow, wrist, centre_x));
        }
    }
    let bones = bones_with(markers, &found);
    let height = (markers.chin.y - markers.ankle.y).abs().max(1e-3);
    let tails = tails(&bones, height);
    let segments: Vec<weights::Segment> =
        bones.iter().zip(&tails).enumerate().map(|(i, (b, t))| weights::Segment { joint: i as u16, a: b.head, b: *t }).collect();
    // Worked out once per primitive, then read per vertex.
    let mut across: std::collections::HashMap<(usize, usize), Vec<RawWeights>> = std::collections::HashMap::new();
    skin::apply(glb, &bones, 0, |v| {
        let raw = match v.surface.filter(|s| !s.triangles.is_empty()) {
            Some(s) => across
                .entry((s.mesh, s.prim))
                .or_insert_with(|| {
                    let indices: Vec<u32> = s.triangles.iter().flatten().copied().collect();
                    weights::surface_raw(&s.positions, &indices, &segments, 6.0)
                })
                .get(v.index)
                .cloned()
                .unwrap_or_default(),
            None => envelope(&bones, &tails, height, v.position),
        };
        Some(sided(&bones, centre_x, height, v.position, raw))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t_pose() -> BodyMarkers {
        let t = |n: &str| template(n).expect("template bone").1;
        BodyMarkers {
            chin: Vec3::new(0.0, 1.55, 0.05),
            shoulder: t("LeftArm"),
            elbow: t("LeftForeArm"),
            wrist: t("LeftHand"),
            groin: t("Hips"),
            knee: t("LeftLeg"),
            ankle: t("LeftFoot"),
        }
    }

    #[test]
    fn every_template_bone_but_the_hips_has_a_parent() {
        for (name, ..) in all_templates() {
            assert_eq!(parent_of(name).is_none(), *name == "Hips", "{name}");
        }
        assert_eq!(parent_of("LeftHandIndex1"), Some("LeftHand"));
        assert_eq!(parent_of("RightHandThumb3"), Some("RightHandThumb2"));
    }

    /// Fingers hang off the hand in a chain, and a finger that bends is
    /// oriented down its own length, not the template's.
    #[test]
    fn found_fingers_join_the_skeleton() {
        let t = |n: &str| template(n).expect("template bone").1;
        let mut fingers: Vec<(&'static str, Vec3)> = Vec::new();
        for (name, _, p) in fingers::TEMPLATE.iter().filter(|f| f.0.starts_with("LeftHandIndex")) {
            fingers.push((name, Vec3::from_array(*p)));
        }
        // Curl the index's tip down.
        fingers[3].1 = t("LeftHandIndex3") + Vec3::new(0.0, -0.03, 0.0);
        let bones = bones_with(&t_pose(), &fingers);
        let index = |n: usize| bones.iter().position(|b| b.name == format!("{PREFIX}LeftHandIndex{n}")).expect("index bone");
        let hand = bones.iter().position(|b| b.name == format!("{PREFIX}LeftHand")).unwrap();
        assert_eq!(bones[index(1)].parent, Some(hand));
        assert_eq!(bones[index(3)].parent, Some(index(2)));
        let down = bones[index(3)].rotation * Vec3::Y;
        assert!(down.y < -0.9, "the curled joint points down, not along +X: {down:?}");
    }

    /// Rigging a body standing exactly in the template pose reproduces the
    /// template: the swing from template to body is no turn at all.
    #[test]
    fn a_template_body_gets_the_template_skeleton() {
        let bones = bones(&t_pose());
        assert_eq!(bones.len(), TEMPLATE.len());
        for b in &bones {
            let short = b.name.trim_start_matches(PREFIX);
            let (rot, pos) = template(short).expect("template");
            if matches!(short, "Hips" | "LeftArm" | "LeftForeArm" | "LeftHand" | "LeftLeg" | "LeftFoot") {
                assert!(b.head.distance(pos) < 1e-3, "{short} at {:?}, template {:?}", b.head, pos);
            }
            assert!(b.rotation.angle_between(rot) < 0.12, "{short} turned {}", b.rotation.angle_between(rot));
        }
    }

    #[test]
    fn the_unmarked_side_is_mirrored() {
        let bones = bones(&t_pose());
        let l = bones.iter().find(|b| b.name == "mixamorig:LeftForeArm").unwrap().head;
        let r = bones.iter().find(|b| b.name == "mixamorig:RightForeArm").unwrap().head;
        assert!((l.x + r.x).abs() < 1e-3 && (l.y - r.y).abs() < 1e-4);
    }
}
