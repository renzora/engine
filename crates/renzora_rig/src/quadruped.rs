//! A four-legged skeleton from thirteen points: dogs, cats, horses, deer,
//! wolves, anything with a spine, four legs and (usually) a tail.
//!
//! There is no Mixamo of quadrupeds, so the bones carry plain names (`pelvis`,
//! `spine_1`, `thigh_l`, `front_foot_r`, `tail_3`) and point along their own
//! length (+Y toward their child), the convention the chain and the
//! skeleton editor use, so bending a joint is a turn about its X or Z.
//!
//! The points, clicked on the side of the animal: the nose, the base of the
//! skull, the withers (the top of the shoulders), the pelvis (the top of the
//! hips), one front leg (shoulder, elbow, wrist, toes), one hind leg (hip,
//! knee, hock, toes), and the tip of the tail. The other legs are mirrored
//! across the animal's middle. The spine, the neck and the tail are spread
//! evenly between their end points.
//!
//! Weights are measured across the skin (`weights::surface_raw`), and each
//! leg's bones are kept to their own side, so a hind leg standing against
//! the other does not pull its skin.

use bevy::math::{Quat, Vec3};

use crate::glb::{Glb, GlbError};
use crate::skin::{self, Applied, Bone};
use crate::weights;

/// The points the user places, in the model's space.
#[derive(Clone, Copy, Debug)]
pub struct QuadrupedMarkers {
    pub nose: Vec3,
    pub skull: Vec3,
    pub withers: Vec3,
    pub front_shoulder: Vec3,
    pub front_elbow: Vec3,
    pub front_wrist: Vec3,
    pub front_toes: Vec3,
    pub pelvis: Vec3,
    pub hip: Vec3,
    pub knee: Vec3,
    pub hock: Vec3,
    pub hind_toes: Vec3,
    /// At the base of the spine for an animal with no tail: then no tail
    /// bones are made.
    pub tail_tip: Vec3,
}

/// How many bones share the tail.
const TAIL_BONES: usize = 4;

/// The skeleton, parents first. `centre_x` is the animal's middle, which
/// the legs are mirrored across.
pub fn bones(m: &QuadrupedMarkers, centre_x: f32) -> Vec<Bone> {
    let mut out: Vec<Bone> = Vec::new();
    let add = |out: &mut Vec<Bone>, name: &str, parent: Option<usize>, head: Vec3| {
        out.push(Bone { name: name.to_string(), parent, head, rotation: Quat::IDENTITY });
        out.len() - 1
    };
    let mid = |p: Vec3| Vec3::new(centre_x, p.y, p.z);
    let (pelvis_at, withers_at, skull_at, nose_at) = (mid(m.pelvis), mid(m.withers), mid(m.skull), mid(m.nose));

    let pelvis = add(&mut out, "pelvis", None, pelvis_at);
    let mut parent = pelvis;
    for (i, t) in [0.25, 0.5, 0.75].into_iter().enumerate() {
        parent = add(&mut out, &format!("spine_{}", i + 1), Some(parent), pelvis_at.lerp(withers_at, t));
    }
    let chest = add(&mut out, "chest", Some(parent), withers_at);
    let neck_1 = add(&mut out, "neck_1", Some(chest), withers_at.lerp(skull_at, 0.15));
    let neck_2 = add(&mut out, "neck_2", Some(neck_1), withers_at.lerp(skull_at, 0.55));
    let head = add(&mut out, "head", Some(neck_2), skull_at);
    add(&mut out, "head_end", Some(head), nose_at);

    // The marked side, and the other mirrored across the middle.
    let marked_left = m.front_shoulder.x >= centre_x;
    for left in [true, false] {
        let side = |p: Vec3| if left == marked_left { p } else { Vec3::new(2.0 * centre_x - p.x, p.y, p.z) };
        let s = if left { "l" } else { "r" };
        let shoulder = side(m.front_shoulder);
        // The shoulder blade runs from beside the withers down to the joint.
        let scapula = add(&mut out, &format!("scapula_{s}"), Some(chest), withers_at.lerp(shoulder, 0.35));
        let upper = add(&mut out, &format!("upper_arm_{s}"), Some(scapula), shoulder);
        let fore = add(&mut out, &format!("forearm_{s}"), Some(upper), side(m.front_elbow));
        let foot = add(&mut out, &format!("front_foot_{s}"), Some(fore), side(m.front_wrist));
        add(&mut out, &format!("front_toes_{s}"), Some(foot), side(m.front_toes));

        let thigh = add(&mut out, &format!("thigh_{s}"), Some(pelvis), side(m.hip));
        let shin = add(&mut out, &format!("shin_{s}"), Some(thigh), side(m.knee));
        let hind_foot = add(&mut out, &format!("hind_foot_{s}"), Some(shin), side(m.hock));
        add(&mut out, &format!("hind_toes_{s}"), Some(hind_foot), side(m.hind_toes));
    }

    // The tail, unless its tip was left at the base of the spine.
    let tail_tip = mid(m.tail_tip);
    if tail_tip.distance(pelvis_at) > 0.1 * pelvis_at.distance(withers_at) {
        let mut parent = pelvis;
        for i in 0..TAIL_BONES {
            let t = i as f32 / TAIL_BONES as f32;
            parent = add(&mut out, &format!("tail_{}", i + 1), Some(parent), pelvis_at.lerp(tail_tip, t));
        }
        add(&mut out, "tail_end", Some(parent), tail_tip);
    }

    aim(&mut out);
    out
}

/// Point every bone +Y at its first child; a leaf carries on its parent's
/// direction.
fn aim(bones: &mut [Bone]) {
    for i in 0..bones.len() {
        let child = bones.iter().position(|b| b.parent == Some(i));
        bones[i].rotation = match child.and_then(|c| (bones[c].head - bones[i].head).try_normalize()) {
            Some(dir) => Quat::from_rotation_arc(Vec3::Y, dir),
            None => bones[i].parent.map(|p| bones[p].rotation).unwrap_or(Quat::IDENTITY),
        };
    }
}

/// One vertex's unnormalised `(bone, weight)` pairs.
type RawWeights = Vec<(usize, f32)>;

/// Rig the animal in `glb` from `markers` (model space).
pub fn build(glb: &mut Glb, markers: &QuadrupedMarkers) -> Result<Applied, GlbError> {
    let centre_x = (markers.withers.x + markers.pelvis.x + markers.nose.x) / 3.0;
    let bones = bones(markers, centre_x);
    let length = markers.nose.distance(markers.pelvis).max(1e-3);
    let segments: Vec<weights::Segment> = bones
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let tail = bones.iter().find(|c| c.parent == Some(i)).map(|c| c.head).unwrap_or(b.head + b.rotation * Vec3::Y * length * 0.05);
            weights::Segment { joint: i as u16, a: b.head, b: tail }
        })
        .collect();
    let mut across: std::collections::HashMap<(usize, usize), Vec<RawWeights>> = std::collections::HashMap::new();
    skin::apply(glb, &bones, 0, |v| {
        let mut raw = match v.surface.filter(|s| !s.triangles.is_empty()) {
            Some(s) => across
                .entry((s.mesh, s.prim))
                .or_insert_with(|| {
                    let indices: Vec<u32> = s.triangles.iter().flatten().copied().collect();
                    weights::surface_raw(&s.positions, &indices, &segments, 6.0)
                })
                .get(v.index)
                .cloned()
                .unwrap_or_default(),
            None => weights::envelope(&[v.position], &segments, 6.0)[0].iter().map(|(j, w)| (*j as usize, *w)).collect(),
        };
        // A leg's bones keep to their own side of the middle.
        let side = v.position.x - centre_x;
        for (i, w) in raw.iter_mut() {
            let name = &bones[*i].name;
            let left = name.ends_with("_l");
            if (left || name.ends_with("_r")) && side.abs() > 0.02 * length && (side > 0.0) != left {
                *w *= 1e-4;
            }
        }
        Some(raw)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dog-ish animal facing +Z, its left side at +X.
    fn dog() -> QuadrupedMarkers {
        QuadrupedMarkers {
            nose: Vec3::new(0.0, 0.6, 0.8),
            skull: Vec3::new(0.0, 0.65, 0.6),
            withers: Vec3::new(0.0, 0.55, 0.35),
            front_shoulder: Vec3::new(0.1, 0.45, 0.4),
            front_elbow: Vec3::new(0.1, 0.3, 0.38),
            front_wrist: Vec3::new(0.1, 0.1, 0.4),
            front_toes: Vec3::new(0.1, 0.0, 0.45),
            pelvis: Vec3::new(0.0, 0.5, -0.3),
            hip: Vec3::new(0.1, 0.42, -0.32),
            knee: Vec3::new(0.1, 0.28, -0.25),
            hock: Vec3::new(0.1, 0.14, -0.38),
            hind_toes: Vec3::new(0.1, 0.0, -0.33),
            tail_tip: Vec3::new(0.0, 0.6, -0.8),
        }
    }

    #[test]
    fn a_dog_gets_four_legs_a_spine_and_a_tail() {
        let b = bones(&dog(), 0.0);
        let find = |n: &str| b.iter().position(|x| x.name == n).unwrap_or_else(|| panic!("{n}"));
        assert_eq!(b[0].name, "pelvis");
        for (i, x) in b.iter().enumerate() {
            assert!(x.parent.is_none_or(|p| p < i), "{} before its parent", x.name);
        }
        let (l, r) = (b[find("shin_l")].head, b[find("shin_r")].head);
        assert!((l.x + r.x).abs() < 1e-5 && l.x > 0.0, "mirrored: {l:?} {r:?}");
        assert_eq!(b[find("tail_end")].parent, Some(find("tail_4")));
        // The spine runs pelvis to withers, the head bone points at the nose.
        let dir = b[find("head")].rotation * Vec3::Y;
        assert!(dir.dot((dog().nose - dog().skull).normalize()) > 0.99);
    }

    #[test]
    fn no_tail_when_its_tip_is_at_the_spine() {
        let m = QuadrupedMarkers { tail_tip: dog().pelvis, ..dog() };
        assert!(!bones(&m, 0.0).iter().any(|b| b.name.starts_with("tail")));
    }
}
