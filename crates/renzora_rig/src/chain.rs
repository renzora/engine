//! A chain of bones along a mesh's length, for anything that bends but is
//! not a body: a capsule or a cylinder from the shape library, a tail, a
//! rope, a tentacle, a plant stem, a worm.
//!
//! The humanoid and face rigs each need a body plan to place their bones;
//! a chain needs only two ends and how many joints. With no ends given they
//! are the ends of the mesh's longest axis (the direction its vertices spread
//! furthest along), starting from the lower end, so a capsule standing up
//! bends from its base the way a plant or a finger does.
//!
//! Each vertex is shared between the one or two bones whose stretch of the
//! chain it lies in, blended smoothly across each joint, so the mesh curves
//! rather than creasing where two bones meet.

use bevy::math::{Quat, Vec3};

use crate::glb::{Glb, GlbError};
use crate::skin::{self, smoothstep, Applied, Bone};

/// Every chain bone's name starts with this.
pub const PREFIX: &str = "chain_";

/// Where the chain runs and how many bones bend it.
#[derive(Clone, Copy, Debug)]
pub struct ChainSpec {
    /// The root end, in the model's space.
    pub start: Vec3,
    /// The tip.
    pub end: Vec3,
    /// How many bones share the length (at least one).
    pub segments: usize,
}

/// The two ends of the model's longest axis, lower end first: where a chain
/// with no ends given runs.
pub fn long_axis(glb: &Glb) -> Result<(Vec3, Vec3), GlbError> {
    let world = glb.world_matrices();
    let mut pts = Vec::new();
    for (node, n) in glb.nodes().iter().enumerate() {
        let Some(mesh) = n.get("mesh").and_then(|m| m.as_u64()).map(|m| m as usize) else { continue };
        let prims = glb.meshes()[mesh].get("primitives").and_then(|p| p.as_array()).map_or(0, Vec::len);
        for prim in 0..prims {
            pts.extend(glb.positions(mesh, prim)?.into_iter().map(|p| world[node].transform_point3(p)));
        }
    }
    axis_of(&pts).ok_or_else(|| GlbError::Unsupported("the model has no vertices to run a chain through".into()))
}

/// Mesh data to tessellate: positions with their normals and UVs (either
/// may be empty), and a triangle list.
pub struct Tessellate<'a> {
    pub positions: &'a mut Vec<Vec3>,
    pub normals: &'a mut Vec<Vec3>,
    pub uvs: &'a mut Vec<[f32; 2]>,
    pub indices: &'a mut Vec<u32>,
}

/// Split every edge that runs further than `max` along `axis` until none
/// does, so the mesh has rows of vertices to bend at.
///
/// A bone can only curve a mesh where it has vertices. The library shapes
/// were made to stand still: a capsule's straight middle is one span from
/// the bottom cap to the top one, and a chain's middle joint fell in that
/// empty stretch, so the top half swung over as a rigid block and folded
/// into the bottom instead of curving.
///
/// Edges are split at their middle, each once however many triangles share
/// it, so the mesh stays watertight; normals and UVs are taken halfway too.
pub fn tessellate_along(mesh: Tessellate, axis: Vec3, max: f32) {
    use std::collections::HashMap;
    let max = max.max(1e-5);
    let Tessellate { positions, normals, uvs, indices } = mesh;
    let long = |p: &[Vec3], a: u32, b: u32| (p[a as usize] - p[b as usize]).dot(axis).abs() > max;
    for _ in 0..12 {
        // Decided before any vertex is added, from the edges as they are.
        let flags: Vec<(bool, bool, bool)> =
            indices.chunks_exact(3).map(|t| (long(positions, t[0], t[1]), long(positions, t[1], t[2]), long(positions, t[2], t[0]))).collect();
        if !flags.iter().any(|f| f.0 || f.1 || f.2) {
            return;
        }
        let mut middle: HashMap<(u32, u32), u32> = HashMap::new();
        let mut split = |a: u32, b: u32, positions: &mut Vec<Vec3>, normals: &mut Vec<Vec3>, uvs: &mut Vec<[f32; 2]>| -> u32 {
            let key = (a.min(b), a.max(b));
            *middle.entry(key).or_insert_with(|| {
                let (ia, ib) = (a as usize, b as usize);
                positions.push((positions[ia] + positions[ib]) * 0.5);
                if normals.len() + 1 == positions.len() {
                    normals.push((normals[ia] + normals[ib]).normalize_or_zero());
                }
                if uvs.len() + 1 == positions.len() {
                    uvs.push([(uvs[ia][0] + uvs[ib][0]) * 0.5, (uvs[ia][1] + uvs[ib][1]) * 0.5]);
                }
                (positions.len() - 1) as u32
            })
        };
        let old = std::mem::take(indices);
        let mut out = Vec::with_capacity(old.len() * 2);
        for (t, &(ab, bc, ca)) in old.chunks_exact(3).zip(&flags) {
            let (a, b, c) = (t[0], t[1], t[2]);
            match (ab, bc, ca) {
                (false, false, false) => out.extend([a, b, c]),
                (true, true, true) => {
                    let (m, n, o) = (split(a, b, positions, normals, uvs), split(b, c, positions, normals, uvs), split(c, a, positions, normals, uvs));
                    out.extend([a, m, o, m, b, n, o, n, c, m, n, o]);
                }
                _ => {
                    // Turn the triangle so its first edge is split; the
                    // winding is kept by rotating, never by swapping.
                    let (a, b, c, bc, ca) = if ab {
                        (a, b, c, bc, ca)
                    } else if bc {
                        (b, c, a, ca, ab)
                    } else {
                        (c, a, b, ab, bc)
                    };
                    let m = split(a, b, positions, normals, uvs);
                    match (bc, ca) {
                        (false, false) => out.extend([a, m, c, m, b, c]),
                        (true, false) => {
                            let n = split(b, c, positions, normals, uvs);
                            out.extend([a, m, n, m, b, n, a, n, c]);
                        }
                        (false, true) => {
                            let o = split(c, a, positions, normals, uvs);
                            out.extend([a, m, o, m, b, c, m, c, o]);
                        }
                        (true, true) => unreachable!("all three split is handled above"),
                    }
                }
            }
        }
        *indices = out;
    }
}

/// The longest axis of `pts`, end to end, lower end first.
pub fn axis_of(pts: &[Vec3]) -> Option<(Vec3, Vec3)> {
    if pts.is_empty() {
        return None;
    }
    let mean = pts.iter().copied().sum::<Vec3>() / pts.len() as f32;
    // Power iteration on the covariance, seeded from the bounding box's
    // longest side so a box-like mesh settles on the obvious axis.
    let (lo, hi) = pts.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), p| (a.min(*p), b.max(*p)));
    let size = hi - lo;
    let mut axis = if size.y >= size.x && size.y >= size.z {
        Vec3::Y
    } else if size.x >= size.z {
        Vec3::X
    } else {
        Vec3::Z
    };
    for _ in 0..24 {
        let next = pts.iter().fold(Vec3::ZERO, |acc, p| {
            let d = *p - mean;
            acc + d * d.dot(axis)
        });
        match next.try_normalize() {
            Some(n) => axis = n,
            None => break,
        }
    }
    let (min_t, max_t) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| {
        let t = (*p - mean).dot(axis);
        (a.min(t), b.max(t))
    });
    let (a, b) = (mean + axis * min_t, mean + axis * max_t);
    // Lower end first; for a level chain, the end nearer -X, then -Z.
    let key = |p: Vec3| (p.y, p.x, p.z);
    Some(if key(a) <= key(b) { (a, b) } else { (b, a) })
}

/// The chain's bones, root first, then a tip bone at the end that bends
/// nothing but gives the last bone a length and a target for IK or springs.
pub fn bones(spec: &ChainSpec) -> Vec<Bone> {
    let segments = spec.segments.max(1);
    let dir = (spec.end - spec.start).try_normalize().unwrap_or(Vec3::Y);
    // Every bone points down the chain (+Y along it), as Mixamo's do along
    // a limb, so bending one is a turn about its own X or Z.
    let rotation = Quat::from_rotation_arc(Vec3::Y, dir);
    let mut out = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let name = if i == segments { format!("{PREFIX}end") } else { format!("{PREFIX}{}", i + 1) };
        let head = spec.start.lerp(spec.end, i as f32 / segments as f32);
        out.push(Bone { name, parent: i.checked_sub(1), head, rotation });
    }
    out
}

/// How far each bone's weight reaches along the chain, in bone lengths from
/// its middle. At 1 a vertex was shared only between a bone and its
/// neighbour across one joint, and a bend showed a lump at every joint; past
/// about 1.5 the turns of neighbouring joints overlap and the mesh curves
/// evenly along its length, the way a tube bent in other engines does.
const BLEND: f32 = 1.75;

/// Weights for a point: the bones whose stretch it lies in, blended across
/// each joint. The tip bone is never weighted.
fn weigh(spec: &ChainSpec, p: Vec3) -> Vec<(usize, f32)> {
    let segments = spec.segments.max(1);
    let d = spec.end - spec.start;
    let len2 = d.length_squared().max(1e-12);
    // Position along the chain in bone lengths: 0 at the root, `segments`
    // at the tip.
    let u = ((p - spec.start).dot(d) / len2 * segments as f32).clamp(0.0, segments as f32);
    (0..segments)
        .map(|i| (i, smoothstep(BLEND, 0.0, (u - (i as f32 + 0.5)).abs())))
        .filter(|(_, w)| *w > 0.0)
        .collect()
}

/// Rig the model in `glb` with a chain from `spec`.
pub fn build(glb: &mut Glb, spec: &ChainSpec) -> Result<Applied, GlbError> {
    let bones = bones(spec);
    skin::apply(glb, &bones, 0, |v| Some(weigh(spec, v.position)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> ChainSpec {
        ChainSpec { start: Vec3::ZERO, end: Vec3::new(0.0, 2.0, 0.0), segments: 4 }
    }

    #[test]
    fn bones_run_root_to_tip_along_the_chain() {
        let b = bones(&spec());
        assert_eq!(b.len(), 5);
        assert_eq!(b[0].parent, None);
        assert_eq!(b[3].parent, Some(2));
        assert_eq!(b[4].name, "chain_end");
        assert!((b[2].head.y - 1.0).abs() < 1e-5);
        assert!((b[1].rotation * Vec3::Y).abs_diff_eq(Vec3::Y, 1e-5));
    }

    /// Halfway along a bone it has most of the vertex; at a joint the two
    /// bones share it equally, so the mesh curves there instead of creasing.
    #[test]
    fn joints_are_shared_and_middles_are_mostly_their_bone() {
        let w = |y: f32| {
            let mut w = weigh(&spec(), Vec3::new(0.3, y, 0.0));
            let sum: f32 = w.iter().map(|(_, w)| w).sum();
            w.iter_mut().for_each(|(_, x)| *x /= sum);
            w
        };
        let heaviest = |w: &[(usize, f32)]| w.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1)).expect("weighted");
        let mid = w(0.75);
        assert_eq!(heaviest(&mid).0, 1);
        assert!(heaviest(&mid).1 > 0.5, "{mid:?}");
        let joint = w(1.0);
        let (j1, j2) = (joint.iter().find(|x| x.0 == 1).unwrap().1, joint.iter().find(|x| x.0 == 2).unwrap().1);
        assert!((j1 - j2).abs() < 1e-4, "{joint:?}");
        // Past either end, the end bone has the most of it.
        assert_eq!(heaviest(&w(-0.5)).0, 0);
        assert_eq!(heaviest(&w(2.5)).0, 3);
    }

    /// A tube with no rows between its ends, like the library capsule's
    /// middle, gets rows every `max`, stays one piece, and keeps its winding.
    #[test]
    fn a_long_span_is_given_rows_to_bend_at() {
        let mut positions = Vec::new();
        let sides = 8;
        for y in [-1.0f32, 1.0] {
            for i in 0..sides {
                let a = i as f32 / sides as f32 * std::f32::consts::TAU;
                positions.push(Vec3::new(a.cos(), y, a.sin()));
            }
        }
        let mut indices = Vec::new();
        for i in 0..sides as u32 {
            let j = (i + 1) % sides as u32;
            indices.extend([i, j, i + sides as u32, j, j + sides as u32, i + sides as u32]);
        }
        let normal_of = |p: &[Vec3], t: &[u32]| (p[t[1] as usize] - p[t[0] as usize]).cross(p[t[2] as usize] - p[t[0] as usize]).normalize();
        let before = normal_of(&positions, &indices[0..3]);
        let (mut normals, mut uvs) = (Vec::new(), Vec::new());
        tessellate_along(Tessellate { positions: &mut positions, normals: &mut normals, uvs: &mut uvs, indices: &mut indices }, Vec3::Y, 0.25);
        for t in indices.chunks_exact(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                assert!((positions[a as usize].y - positions[b as usize].y).abs() <= 0.25 + 1e-5);
            }
        }
        let rows: std::collections::BTreeSet<i32> = positions.iter().map(|p| (p.y * 1000.0).round() as i32).collect();
        assert!(rows.len() >= 9, "{} rows", rows.len());
        // The first triangle's replacement still faces the same way.
        assert!(normal_of(&positions, &indices[0..3]).dot(before) > 0.9);
        // Watertight: every edge is shared by exactly two triangles.
        let mut edges: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
        for t in indices.chunks_exact(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *edges.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let open = edges.values().filter(|n| **n != 2).count();
        // The tube's two rims are open edges, 8 each, and nothing else.
        assert_eq!(open, 2 * sides, "{open} open edges");
    }

    #[test]
    fn the_long_axis_of_a_lying_box_is_its_length() {
        let mut pts = Vec::new();
        for x in [-2.0, 2.0] {
            for y in [-0.3, 0.3] {
                for z in [-0.5, 0.5] {
                    pts.push(Vec3::new(x, y + 1.0, z));
                }
            }
        }
        let (a, b) = axis_of(&pts).expect("axis");
        assert!((a.x + 2.0).abs() < 1e-3 && (b.x - 2.0).abs() < 1e-3, "{a:?} {b:?}");
        assert!((a.y - 1.0).abs() < 1e-3);
    }

    /// A model written from bare geometry rigs as a chain end to end.
    #[test]
    fn a_written_shape_takes_a_chain() {
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for i in 0..=8 {
            let y = i as f32 * 0.25;
            positions.extend([Vec3::new(-0.2, y, 0.0), Vec3::new(0.2, y, 0.0)]);
            if i < 8 {
                let b = (i * 2) as u32;
                indices.extend([b, b + 1, b + 2, b + 1, b + 3, b + 2]);
            }
        }
        let geometry = crate::glb::Geometry { name: "strip", positions: &positions, normals: &[], uvs: &[], indices: &indices, base_color: [1.0; 4] };
        let mut glb = Glb::from_geometry(&geometry).expect("written");
        let (start, end) = long_axis(&glb).expect("axis");
        assert!(start.y < 0.01 && end.y > 1.99);
        let applied = build(&mut glb, &ChainSpec { start, end, segments: 3 }).expect("rigged");
        assert_eq!((applied.bones, applied.meshes, applied.vertices), (4, 1, 18));
        let reread = Glb::parse(&glb.to_bytes()).expect("still a glb");
        assert_eq!(reread.skins().len(), 1);
    }
}
