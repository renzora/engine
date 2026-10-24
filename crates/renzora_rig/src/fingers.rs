//! Finger bones for the humanoid rig, found from the shape of each hand.
//!
//! The body's seven points stop at the wrist, and asking for five more clicks
//! per hand would make the hands the most tedious part of rigging. The hand
//! already says where its fingers are: past the knuckles a hand is not one
//! piece but five. So the hand's surface is filled in with samples and cut
//! across, square to the forearm, and what lies beyond the cut splits into
//! separate pieces. Scanning the cut from the fingertips back toward the wrist,
//! the most pieces appear just past the knuckles, before the palm joins them
//! again; that cut gives the fingers.
//!
//! The thumb usually leaves the palm before the knuckles, so it is looked for
//! off the side of the palm instead, and the side it is on says which end of
//! the row of fingers is the index. A hand modelled as a mitten has nothing to
//! split and gets no finger bones; the hand bone carries it as before.
//!
//! The bones are Mixamo's (`LeftHandIndex1` to `LeftHandIndex4` and so on,
//! four to a finger, base to tip), oriented from [`TEMPLATE`] the same way
//! [`crate::humanoid`] orients the rest, so a Mixamo clip's finger curls play
//! on them.

use std::collections::{HashMap, HashSet, VecDeque};

use bevy::math::Vec3;

use crate::glb::{Glb, GlbError};

/// `(name without the mixamorig: prefix, model-space rotation xyzw, position)`
/// for each finger bone of the Mixamo rig [`crate::humanoid::TEMPLATE`] comes
/// from, in its rest pose. Grouped by finger, base to tip, so a bone's parent
/// always comes before it.
pub const TEMPLATE: &[(&str, [f32; 4], [f32; 3])] = &[
    ("LeftHandThumb1", [-0.677212, -0.203432, 0.514739, -0.484812], [0.7757, 1.4141, -0.0317]),
    ("LeftHandThumb2", [-0.677215, -0.203428, 0.514740, -0.484808], [0.8124, 1.3928, -0.0105]),
    ("LeftHandThumb3", [-0.677215, -0.203428, 0.514740, -0.484808], [0.8464, 1.3732, 0.0091]),
    ("LeftHandThumb4", [-0.753812, -0.208667, 0.391127, -0.485021], [0.8732, 1.3578, 0.0246]),
    ("LeftHandIndex1", [-0.499981, -0.500019, 0.499976, -0.500024], [0.8604, 1.4334, -0.0335]),
    ("LeftHandIndex2", [-0.499978, -0.500022, 0.499979, -0.500021], [0.8994, 1.4334, -0.0335]),
    ("LeftHandIndex3", [-0.499978, -0.500022, 0.499979, -0.500021], [0.9335, 1.4334, -0.0335]),
    ("LeftHandIndex4", [-0.501986, -0.502007, 0.497963, -0.498027], [0.9643, 1.4334, -0.0335]),
    ("LeftHandMiddle1", [-0.499978, -0.500022, 0.499978, -0.500022], [0.8655, 1.4357, -0.0617]),
    ("LeftHandMiddle2", [-0.499978, -0.500022, 0.499978, -0.500022], [0.9017, 1.4357, -0.0617]),
    ("LeftHandMiddle3", [-0.499978, -0.500022, 0.499978, -0.500022], [0.9363, 1.4357, -0.0617]),
    ("LeftHandMiddle4", [-0.501935, -0.502634, 0.498009, -0.497400], [0.9731, 1.4357, -0.0617]),
    ("LeftHandRing1", [-0.499981, -0.500019, 0.499976, -0.500024], [0.8592, 1.4358, -0.0839]),
    ("LeftHandRing2", [-0.499975, -0.500024, 0.499979, -0.500021], [0.8953, 1.4358, -0.0839]),
    ("LeftHandRing3", [-0.499975, -0.500024, 0.499979, -0.500021], [0.9283, 1.4358, -0.0839]),
    ("LeftHandRing4", [-0.504049, -0.503319, 0.495885, -0.496690], [0.9649, 1.4358, -0.0839]),
    ("LeftHandPinky1", [-0.500050, -0.499950, 0.499909, -0.500091], [0.8469, 1.4335, -0.1090]),
    ("LeftHandPinky2", [-0.499863, -0.500137, 0.500090, -0.499910], [0.8882, 1.4335, -0.1090]),
    ("LeftHandPinky3", [-0.499980, -0.500020, 0.499977, -0.500023], [0.9142, 1.4335, -0.1090]),
    ("LeftHandPinky4", [-0.501737, -0.502354, 0.498209, -0.497682], [0.9434, 1.4335, -0.1090]),
    ("RightHandThumb1", [-0.677229, 0.203373, -0.514782, -0.484768], [-0.7757, 1.4139, -0.0317]),
    ("RightHandThumb2", [-0.677233, 0.203368, -0.514784, -0.484762], [-0.8125, 1.3927, -0.0105]),
    ("RightHandThumb3", [-0.677233, 0.203368, -0.514784, -0.484762], [-0.8464, 1.3731, 0.0091]),
    ("RightHandThumb4", [-0.754550, 0.209065, -0.389944, -0.484654], [-0.8732, 1.3576, 0.0246]),
    ("RightHandIndex1", [-0.500024, 0.499976, -0.500019, -0.499981], [-0.8605, 1.4333, -0.0335]),
    ("RightHandIndex2", [-0.500021, 0.499979, -0.500022, -0.499978], [-0.8994, 1.4333, -0.0335]),
    ("RightHandIndex3", [-0.500021, 0.499979, -0.500022, -0.499978], [-0.9336, 1.4333, -0.0335]),
    ("RightHandIndex4", [-0.502641, 0.504045, -0.497307, -0.495960], [-0.9643, 1.4333, -0.0335]),
    ("RightHandMiddle1", [-0.500022, 0.499978, -0.500022, -0.499978], [-0.8656, 1.4356, -0.0617]),
    ("RightHandMiddle2", [-0.500022, 0.499978, -0.500022, -0.499978], [-0.9017, 1.4356, -0.0617]),
    ("RightHandMiddle3", [-0.500022, 0.499978, -0.500022, -0.499978], [-0.9363, 1.4356, -0.0617]),
    ("RightHandMiddle4", [-0.504186, 0.502485, -0.495782, -0.497499], [-0.9731, 1.4356, -0.0617]),
    ("RightHandRing1", [-0.500024, 0.499976, -0.500019, -0.499981], [-0.8593, 1.4357, -0.0839]),
    ("RightHandRing2", [-0.500020, 0.499980, -0.500023, -0.499977], [-0.8953, 1.4357, -0.0839]),
    ("RightHandRing3", [-0.500020, 0.499980, -0.500023, -0.499977], [-0.9284, 1.4357, -0.0839]),
    ("RightHandRing4", [-0.503932, 0.503652, -0.496021, -0.496337], [-0.9650, 1.4357, -0.0839]),
    ("RightHandPinky1", [-0.500023, 0.499977, -0.500020, -0.499980], [-0.8469, 1.4333, -0.1090]),
    ("RightHandPinky2", [-0.500018, 0.499982, -0.500025, -0.499975], [-0.8883, 1.4333, -0.1090]),
    ("RightHandPinky3", [-0.500023, 0.499977, -0.500020, -0.499980], [-0.9142, 1.4333, -0.1090]),
    ("RightHandPinky4", [-0.503110, 0.505351, -0.496813, -0.494649], [-0.9435, 1.4333, -0.1090]),
];

/// The fingers in order across the hand, from the thumb.
const FINGERS: [&str; 5] = ["Thumb", "Index", "Middle", "Ring", "Pinky"];

/// Where along a finger, from its knuckle to its tip, joints 2 and 3 sit: the
/// Mixamo template's proportions.
const FINGER_JOINTS: [f32; 2] = [0.36, 0.68];
/// The same for the thumb, from the wrist: joints 1, 2 and 3.
const THUMB_JOINTS: [f32; 3] = [0.30, 0.57, 0.81];

/// Every triangle of every mesh, in the model's space.
pub fn triangles(glb: &Glb) -> Result<Vec<[Vec3; 3]>, GlbError> {
    let world = glb.world_matrices();
    let mut out = Vec::new();
    for (node, n) in glb.nodes().iter().enumerate() {
        let Some(mesh) = n.get("mesh").and_then(|m| m.as_u64()).map(|m| m as usize) else { continue };
        let prims = glb.meshes()[mesh].get("primitives").and_then(|p| p.as_array()).map_or(0, Vec::len);
        for prim in 0..prims {
            let pts: Vec<Vec3> = glb.positions(mesh, prim)?.iter().map(|p| world[node].transform_point3(*p)).collect();
            let idx: Vec<u32> = match glb.indices(mesh, prim)? {
                Some(idx) => idx,
                None => (0..pts.len() as u32).collect(),
            };
            out.extend(idx.chunks_exact(3).filter_map(|t| Some([*pts.get(t[0] as usize)?, *pts.get(t[1] as usize)?, *pts.get(t[2] as usize)?])));
        }
    }
    Ok(out)
}

/// The finger joints of one hand, as `(template name, position)`, every
/// finger found complete from base to tip. Empty for a mitten, or when the
/// hand cannot be told from the rest of the body.
///
/// `side` is `"Left"` or `"Right"`; `elbow` and `wrist` are that arm's
/// joints; `centre_x` is the middle of the body, so the other hand and the
/// body's far side are left out.
pub fn detect(tris: &[[Vec3; 3]], side: &str, elbow: Vec3, wrist: Vec3, centre_x: f32) -> Vec<(&'static str, Vec3)> {
    let forearm = (wrist - elbow).length();
    let Some(along) = (wrist - elbow).try_normalize() else { return Vec::new() };
    let outward = (wrist.x - centre_x).signum();
    let in_hand = |p: Vec3| {
        let d = p - wrist;
        let t = d.dot(along);
        t > -0.05 * forearm && t < 1.2 * forearm && (d - along * t).length() < 0.6 * forearm && (p.x - centre_x) * outward > 0.0
    };

    // Small enough that the gap between two fingers is several cells wide,
    // so neighbouring fingers stay apart; the surface is sampled at half
    // that, so one finger's own cells always touch.
    let cell = 0.008 * forearm;
    let mut pts = Vec::new();
    for t in tris.iter().filter(|t| t.iter().any(|p| in_hand(*p))) {
        sample(t, cell * 0.5, &mut pts);
    }
    pts.retain(|p| in_hand(*p));
    if pts.len() < 50 {
        return Vec::new();
    }
    let ts: Vec<f32> = pts.iter().map(|p| (*p - wrist).dot(along)).collect();
    let grid = Grid::new(&pts, cell);
    let length = ts.iter().copied().fold(0.0, f32::max);
    if length <= 0.0 {
        return Vec::new();
    }

    // Across the hand: past the middle of it, a hand is wider than it is
    // thick, so the samples there spread most across the fingers.
    let distal: Vec<Vec3> = pts.iter().zip(&ts).filter(|(_, t)| **t > 0.5 * length).map(|(p, _)| *p).collect();
    let across = spread(&distal, along);
    let s: Vec<f32> = pts.iter().map(|p| (*p - wrist).dot(across)).collect();

    // The cut that finds the most fingers (four at most), the lowest of
    // those: just past the knuckles, where the fingers are at their longest.
    let mut best: Option<(f32, Vec<Vec<usize>>)> = None;
    for k in (12..=36).rev() {
        let cut = length * k as f32 / 40.0;
        let found = finger_lanes(&grid, &ts, &s, cut, length);
        if found.len() <= 4 && found.len() >= best.as_ref().map_or(2, |b| b.1.len()) {
            best = Some((cut, found));
        }
    }
    let Some((cut, mut pieces)) = best else { return Vec::new() };
    let slice = 0.04 * length;
    let mut bases: Vec<Vec3> = pieces.iter().map(|p| centroid(p.iter().filter(|&&i| ts[i] < cut + slice).map(|&i| pts[i]))).collect();
    let reach = |piece: &[usize]| piece.iter().map(|&i| ts[i]).fold(0.0, f32::max);
    let s_of = |p: Vec3| (p - wrist).dot(across);

    // The thumb, off the side of the palm: whichever side has more of the
    // hand beyond the row of fingers.
    let mut thumb: Option<Vec<Vec3>> = None;
    let mut thumb_first = None;
    {
        let (first_s, last_s) = (s_of(bases[0]), s_of(bases[bases.len() - 1]));
        let spacing = if bases.len() > 1 { (last_s - first_s) / (bases.len() - 1) as f32 } else { 0.1 * length };
        let (lo, hi) = (first_s - 0.5 * spacing, last_s + 0.5 * spacing);
        let low: Vec<usize> = (0..pts.len()).filter(|&i| ts[i] > 0.0 && s[i] < lo).collect();
        let high: Vec<usize> = (0..pts.len()).filter(|&i| ts[i] > 0.0 && s[i] > hi).collect();
        let (side_pts, first) = if low.len() >= high.len() { (low, true) } else { (high, false) };
        let biggest = grid.components(|i| side_pts.binary_search(&i).is_ok()).into_iter().max_by_key(Vec::len);
        if let Some(piece) = biggest {
            let out_of_palm = piece.iter().map(|&i| if first { lo - s[i] } else { s[i] - hi }).fold(0.0, f32::max);
            if piece.len() >= 30 && out_of_palm > 0.1 * length {
                thumb = Some(piece.iter().map(|&i| pts[i]).collect());
                thumb_first = Some(first);
            }
        }
    }

    // The index is beside the thumb; with no thumb found, it is the forward
    // end, as on the template (the body faces +Z).
    let index_first = thumb_first.unwrap_or_else(|| bases[0].z >= bases[bases.len() - 1].z);
    if !index_first {
        pieces.reverse();
        bases.reverse();
    }

    let mut out = Vec::new();
    let mut push = |finger: &str, joints: [Vec3; 4]| {
        for (i, p) in joints.into_iter().enumerate() {
            let name = format!("{side}Hand{finger}{}", i + 1);
            if let Some(t) = TEMPLATE.iter().find(|t| t.0 == name) {
                out.push((t.0, p));
            }
        }
    };
    if let Some(thumb) = thumb {
        let far = thumb.iter().map(|p| p.distance(wrist)).fold(0.0, f32::max);
        let tip = centroid(thumb.iter().copied().filter(|p| p.distance(wrist) > far - slice));
        let at = |f: f32| centre_near(&thumb, wrist.lerp(tip, f), 0.5 * slice);
        push("Thumb", [wrist.lerp(tip, THUMB_JOINTS[0]), at(THUMB_JOINTS[1]), at(THUMB_JOINTS[2]), tip]);
    }
    for (piece, (finger, base)) in pieces.iter().zip(FINGERS[1..].iter().zip(&bases)) {
        let points: Vec<Vec3> = piece.iter().map(|&i| pts[i]).collect();
        let far = reach(piece);
        let tip = centroid(piece.iter().filter(|&&i| ts[i] > far - slice).map(|&i| pts[i]));
        // The knuckle is inside the palm, a quarter of the visible finger
        // back from where the fingers part.
        let knuckle = *base - (tip - *base) * 0.25;
        let at = |f: f32| centre_near(&points, knuckle.lerp(tip, f), 0.5 * slice);
        push(finger, [knuckle, at(FINGER_JOINTS[0]), at(FINGER_JOINTS[1]), tip]);
    }
    out
}

/// The fingers beyond `cut`, as sample indices, in order across the hand
/// (`s`), the thumb left out.
///
/// What lies beyond the cut is in pieces, but a piece is not always a
/// finger. A finger built from separate segments, as a jointed mannequin's
/// is, is several pieces in a row; two fingers modelled touching are one. So
/// pieces that cover the same stretch across the hand are one lane, a lane
/// about twice as wide as a typical one is split down the middle, and an end
/// lane reaching much less far than the rest is the thumb's tip and dropped.
fn finger_lanes(grid: &Grid, ts: &[f32], s: &[f32], cut: f32, length: f32) -> Vec<Vec<usize>> {
    let reach = |piece: &[usize]| piece.iter().map(|&i| ts[i]).fold(0.0, f32::max);
    let mut lanes: Vec<(f32, f32, Vec<usize>)> = grid
        .components(|i| ts[i] > cut)
        .into_iter()
        .filter(|c| c.len() >= 8 && reach(c) - cut > 0.08 * length)
        .map(|c| {
            let mut across: Vec<f32> = c.iter().map(|&i| s[i]).collect();
            across.sort_by(f32::total_cmp);
            // Inner percentiles: a stray sample must not widen a finger.
            let at = |q: f32| across[((across.len() - 1) as f32 * q) as usize];
            (at(0.05), at(0.95), c)
        })
        .collect();
    lanes.sort_by(|a, b| (a.0 + a.1).total_cmp(&(b.0 + b.1)));
    let mut merged: Vec<(f32, f32, Vec<usize>)> = Vec::new();
    for (lo, hi, piece) in lanes {
        if let Some(last) = merged.last_mut() {
            let overlap = hi.min(last.1) - lo.max(last.0);
            if overlap > 0.5 * (hi - lo).min(last.1 - last.0) {
                last.0 = last.0.min(lo);
                last.1 = last.1.max(hi);
                last.2.extend(piece);
                continue;
            }
        }
        merged.push((lo, hi, piece));
    }
    if merged.len() > 1 {
        let reaches: Vec<f32> = merged.iter().map(|l| reach(&l.2)).collect();
        let longest_but = |skip: usize| reaches.iter().enumerate().filter(|(i, _)| *i != skip).map(|(_, r)| *r).fold(0.0, f32::max);
        let last = merged.len() - 1;
        if reaches[last] < 0.75 * longest_but(last) {
            merged.pop();
        } else if reaches[0] < 0.75 * longest_but(0) {
            merged.remove(0);
        }
    }
    if merged.is_empty() {
        return Vec::new();
    }
    let mut widths: Vec<f32> = merged.iter().map(|l| l.1 - l.0).collect();
    widths.sort_by(f32::total_cmp);
    let typical = widths[widths.len() / 2].max(1e-6);
    let mut out = Vec::new();
    for (lo, hi, piece) in merged {
        let width = hi - lo;
        let n = if width > 1.6 * typical { (width / typical).round().max(1.0) as usize } else { 1 };
        if n == 1 {
            out.push(piece);
            continue;
        }
        let mut bands = vec![Vec::new(); n];
        for i in piece {
            let band = (((s[i] - lo) / width * n as f32).floor().max(0.0) as usize).min(n - 1);
            bands[band].push(i);
        }
        out.extend(bands.into_iter().filter(|b| b.len() >= 8));
    }
    out
}

/// Points over a triangle's surface, about `spacing` apart, capped so a
/// huge triangle from elsewhere on the body cannot flood the hand.
fn sample(t: &[Vec3; 3], spacing: f32, out: &mut Vec<Vec3>) {
    let longest = t[0].distance(t[1]).max(t[1].distance(t[2])).max(t[2].distance(t[0]));
    let n = ((longest / spacing.max(1e-6)).ceil() as usize).clamp(1, 64);
    for i in 0..=n {
        for j in 0..=n - i {
            let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
            out.push(t[0] + (t[1] - t[0]) * u + (t[2] - t[0]) * v);
        }
    }
}

/// The hand's samples sorted into cells once, with each cell's touching
/// neighbours (corners included), so the many cuts the scan tries are each a
/// walk over cells rather than a fresh hashing of every sample: a few
/// hundred thousand samples times twenty-five cuts was seconds per hand.
struct Grid {
    points: Vec<Vec<usize>>,
    neighbours: Vec<Vec<u32>>,
}

impl Grid {
    fn new(pts: &[Vec3], cell: f32) -> Grid {
        let key = |p: Vec3| ((p.x / cell).floor() as i32, (p.y / cell).floor() as i32, (p.z / cell).floor() as i32);
        let mut index: HashMap<(i32, i32, i32), usize> = HashMap::new();
        let mut points: Vec<Vec<usize>> = Vec::new();
        let mut keys = Vec::new();
        for (i, p) in pts.iter().enumerate() {
            let k = key(*p);
            let c = *index.entry(k).or_insert_with(|| {
                points.push(Vec::new());
                keys.push(k);
                points.len() - 1
            });
            points[c].push(i);
        }
        let neighbours = keys
            .iter()
            .map(|c| {
                let mut out = Vec::new();
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            if (dx, dy, dz) != (0, 0, 0) {
                                if let Some(&n) = index.get(&(c.0 + dx, c.1 + dy, c.2 + dz)) {
                                    out.push(n as u32);
                                }
                            }
                        }
                    }
                }
                out
            })
            .collect();
        Grid { points, neighbours }
    }

    /// Connected pieces of the samples `keep` admits, as sample indices.
    fn components(&self, keep: impl Fn(usize) -> bool) -> Vec<Vec<usize>> {
        let kept: Vec<Vec<usize>> = self.points.iter().map(|ps| ps.iter().copied().filter(|&i| keep(i)).collect()).collect();
        let mut seen: HashSet<usize> = HashSet::new();
        let mut out = Vec::new();
        for start in 0..kept.len() {
            if kept[start].is_empty() || !seen.insert(start) {
                continue;
            }
            let mut piece = Vec::new();
            let mut queue = VecDeque::from([start]);
            while let Some(c) = queue.pop_front() {
                piece.extend_from_slice(&kept[c]);
                for &n in &self.neighbours[c] {
                    let n = n as usize;
                    if !kept[n].is_empty() && seen.insert(n) {
                        queue.push_back(n);
                    }
                }
            }
            out.push(piece);
        }
        out
    }
}

fn centroid(pts: impl Iterator<Item = Vec3>) -> Vec3 {
    let (sum, n) = pts.fold((Vec3::ZERO, 0u32), |(s, n), p| (s + p, n + 1));
    if n == 0 {
        Vec3::ZERO
    } else {
        sum / n as f32
    }
}

/// The middle of the surface around `p`: a joint placed on a straight line
/// from knuckle to tip lies off-centre in a curled finger.
fn centre_near(pts: &[Vec3], p: Vec3, radius: f32) -> Vec3 {
    let near: Vec<Vec3> = pts.iter().copied().filter(|q| q.distance(p) < radius).collect();
    if near.len() < 4 {
        p
    } else {
        centroid(near.into_iter())
    }
}

/// The direction `points` are most spread along, square to `along`.
fn spread(points: &[Vec3], along: Vec3) -> Vec3 {
    let flat: Vec<Vec3> = points.iter().map(|p| *p - along * p.dot(along)).collect();
    let mean = centroid(flat.iter().copied());
    let mut v = along.any_orthonormal_vector();
    for _ in 0..32 {
        let next = flat.iter().fold(Vec3::ZERO, |acc, p| {
            let d = *p - mean;
            acc + d * d.dot(v)
        });
        match (next - along * next.dot(along)).try_normalize() {
            Some(n) => v = n,
            None => break,
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box's twelve triangles.
    fn cuboid(min: Vec3, max: Vec3, out: &mut Vec<[Vec3; 3]>) {
        let c = |x: bool, y: bool, z: bool| Vec3::new(if x { max.x } else { min.x }, if y { max.y } else { min.y }, if z { max.z } else { min.z });
        let faces = [
            [c(false, false, false), c(true, false, false), c(true, true, false), c(false, true, false)],
            [c(false, false, true), c(true, false, true), c(true, true, true), c(false, true, true)],
            [c(false, false, false), c(true, false, false), c(true, false, true), c(false, false, true)],
            [c(false, true, false), c(true, true, false), c(true, true, true), c(false, true, true)],
            [c(false, false, false), c(false, true, false), c(false, true, true), c(false, false, true)],
            [c(true, false, false), c(true, true, false), c(true, true, true), c(true, false, true)],
        ];
        for f in faces {
            out.push([f[0], f[1], f[2]]);
            out.push([f[0], f[2], f[3]]);
        }
    }

    /// A left hand like the template's: palm down along +X, fingers spread
    /// along Z with the index forward, the thumb off the front of the palm.
    fn hand(with_thumb: bool) -> Vec<[Vec3; 3]> {
        let mut t = Vec::new();
        cuboid(Vec3::new(0.74, 1.424, -0.115), Vec3::new(0.84, 1.448, -0.025), &mut t);
        for z in [-0.033, -0.057, -0.081, -0.105] {
            cuboid(Vec3::new(0.84, 1.428, z - 0.008), Vec3::new(0.93, 1.444, z + 0.008), &mut t);
        }
        if with_thumb {
            cuboid(Vec3::new(0.76, 1.416, -0.025), Vec3::new(0.80, 1.432, 0.03), &mut t);
        }
        t
    }

    fn joint(found: &[(&str, Vec3)], name: &str) -> Vec3 {
        found.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("{name} not found")).1
    }

    const ELBOW: Vec3 = Vec3::new(0.46, 1.436, -0.06);
    const WRIST: Vec3 = Vec3::new(0.74, 1.436, -0.06);

    #[test]
    fn a_hand_gets_five_fingers_in_order() {
        let found = detect(&hand(true), "Left", ELBOW, WRIST, 0.0);
        assert_eq!(found.len(), 20, "{:?}", found.iter().map(|f| f.0).collect::<Vec<_>>());
        for (finger, z) in [("Index", -0.033), ("Middle", -0.057), ("Ring", -0.081), ("Pinky", -0.105)] {
            let tip = joint(&found, &format!("LeftHand{finger}4"));
            assert!((tip.z - z).abs() < 0.004 && tip.x > 0.91, "{finger} tip at {tip:?}");
            let knuckle = joint(&found, &format!("LeftHand{finger}1"));
            assert!(knuckle.x < 0.84 && knuckle.x > 0.80, "{finger} knuckle at {knuckle:?}");
        }
        assert!(joint(&found, "LeftHandThumb4").z > 0.0, "the thumb reaches forward");
    }

    /// Without a thumb to go by, the forward end of the row is the index.
    #[test]
    fn with_no_thumb_the_index_is_forward() {
        let found = detect(&hand(false), "Left", ELBOW, WRIST, 0.0);
        assert_eq!(found.len(), 16);
        assert!((joint(&found, "LeftHandIndex4").z + 0.033).abs() < 0.004);
        assert!(!found.iter().any(|f| f.0.contains("Thumb")));
    }

    /// Two fingers modelled touching are one piece, and a jointed finger is
    /// several: both still come out as four fingers.
    #[test]
    fn touching_and_jointed_fingers_are_still_four() {
        let mut t = Vec::new();
        cuboid(Vec3::new(0.74, 1.424, -0.115), Vec3::new(0.84, 1.448, -0.025), &mut t);
        // The index in two segments with a gap at the joint.
        cuboid(Vec3::new(0.84, 1.428, -0.041), Vec3::new(0.88, 1.444, -0.025), &mut t);
        cuboid(Vec3::new(0.886, 1.428, -0.041), Vec3::new(0.93, 1.444, -0.025), &mut t);
        // Middle and ring as one block.
        cuboid(Vec3::new(0.84, 1.428, -0.089), Vec3::new(0.93, 1.444, -0.049), &mut t);
        cuboid(Vec3::new(0.84, 1.428, -0.113), Vec3::new(0.92, 1.444, -0.097), &mut t);
        let found = detect(&t, "Left", ELBOW, WRIST, 0.0);
        assert_eq!(found.len(), 16, "{:?}", found.iter().map(|f| f.0).collect::<Vec<_>>());
        assert!((joint(&found, "LeftHandMiddle4").z + 0.059).abs() < 0.006);
        assert!((joint(&found, "LeftHandRing4").z + 0.079).abs() < 0.006);
    }

    /// A mitten has nothing to split: no fingers rather than wrong ones.
    #[test]
    fn a_mitten_gets_no_fingers() {
        let mut t = Vec::new();
        cuboid(Vec3::new(0.74, 1.424, -0.115), Vec3::new(0.93, 1.448, -0.025), &mut t);
        assert!(detect(&t, "Left", ELBOW, WRIST, 0.0).is_empty());
    }
}
