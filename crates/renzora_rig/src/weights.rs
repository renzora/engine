//! Skin weights on a model that already has a skeleton: generating them from
//! the bones, smoothing them, and the brush that paints them by hand.
//!
//! [`crate::skin`] weights a model while rigging it, from rules that know what
//! each bone is for. These work on any skeleton, a Mixamo one included, from
//! nothing but where its bones are, which is what the Skin mode needs:
//! "Auto weights" over a rig someone else made, then painting.
//!
//! Every vertex keeps four `(joint, weight)` slots summing to 1, the shape the
//! GPU skins with and the shape `JOINTS_0`/`WEIGHTS_0` store. A joint is an
//! index into the mesh's skin, never a bone name.

use bevy::math::Vec3;

use crate::skin::top_four;

/// One vertex's four influences.
pub type Influences = [(u16, f32); 4];

/// A bone as the generators see it: the joint it belongs to, and the line from
/// where it starts to where its child starts. A leaf bone is a point
/// (`a == b`).
#[derive(Clone, Copy, Debug)]
pub struct Segment {
    pub joint: u16,
    pub a: Vec3,
    pub b: Vec3,
}

fn distance_to_segment(p: Vec3, s: &Segment) -> f32 {
    let ab = s.b - s.a;
    let len2 = ab.length_squared();
    if len2 <= 1e-12 {
        return p.distance(s.a);
    }
    let t = ((p - s.a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(s.a + ab * t)
}

/// Weights by closeness to each bone: a vertex goes mostly to the bone it is
/// nearest, and blends into the next ones by how near they come.
/// `sharpness` is the power of the falloff; higher makes harder edges between
/// bones (6 suits bodies, it is what the humanoid auto-rig uses).
///
/// The distance is measured to the whole bone, not its joint, so a long thigh
/// owns the middle of the leg instead of handing it to the knee half way down.
pub fn envelope(positions: &[Vec3], bones: &[Segment], sharpness: f32) -> Vec<Influences> {
    let fallback = bones.first().map(|b| b.joint as usize).unwrap_or(0);
    positions
        .iter()
        .map(|&p| {
            let raw: Vec<(usize, f32)> = bones
                .iter()
                .map(|s| (s.joint as usize, 1.0 / (distance_to_segment(p, s) + 1e-4).powf(sharpness)))
                .collect();
            top_four(merge(raw), fallback)
        })
        .collect()
}

/// Weights by distance across the skin instead of through the air.
///
/// [`envelope`] measures straight lines, so a bone reaches any skin that is
/// near it in space: an arm resting against the ribs pulls the ribs with it,
/// a thumb pulls the side of the index finger. Here each bone starts from the
/// vertices it is (nearly) the nearest bone to, and its distance spreads out
/// over the mesh's surface from there. Skin that is close through the air but
/// far round the surface (the chest seen from the upper arm) is then far, and
/// the arm leaves it alone, while skin across a joint (the elbow) is close to
/// both bones either way and still blends.
///
/// Vertices at the same place are welded first: a UV seam splits a vertex in
/// two, and without the weld the surface would be cut along every seam. A
/// piece of the mesh no bone starts on (a separate eyeball, a button) is
/// reached only by straight-line distance, as [`envelope`] would.
pub fn surface(positions: &[Vec3], indices: &[u32], bones: &[Segment], sharpness: f32) -> Vec<Influences> {
    let fallback = bones.first().map(|b| b.joint as usize).unwrap_or(0);
    surface_raw(positions, indices, bones, sharpness).into_iter().map(|raw| top_four(merge(raw), fallback)).collect()
}

/// [`surface`] before it is cut to four influences: every `(joint, weight)`
/// for every vertex, unnormalised, for a rule that adjusts them first.
pub fn surface_raw(positions: &[Vec3], indices: &[u32], bones: &[Segment], sharpness: f32) -> Vec<Vec<(usize, f32)>> {
    use std::cmp::Reverse;
    use std::collections::{BinaryHeap, HashMap};

    let n = positions.len();
    if n == 0 || bones.is_empty() {
        return vec![Vec::new(); n];
    }
    // Weld by position, to a millionth of the model's size.
    let (lo, hi) = positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), p| (a.min(*p), b.max(*p)));
    let size = (hi - lo).length().max(1e-6);
    let q = size * 1e-6;
    let mut node_of = Vec::with_capacity(n);
    let mut nodes: Vec<Vec3> = Vec::new();
    let mut seen: HashMap<(i64, i64, i64), usize> = HashMap::new();
    for p in positions {
        let key = ((p.x / q).round() as i64, (p.y / q).round() as i64, (p.z / q).round() as i64);
        let id = *seen.entry(key).or_insert_with(|| {
            nodes.push(*p);
            nodes.len() - 1
        });
        node_of.push(id);
    }
    let m = nodes.len();
    let mut adj: Vec<Vec<(u32, f32)>> = vec![Vec::new(); m];
    for tri in indices.chunks_exact(3) {
        for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let (Some(&x), Some(&y)) = (node_of.get(a as usize), node_of.get(b as usize)) else { continue };
            if x == y || adj[x].iter().any(|(k, _)| *k as usize == y) {
                continue;
            }
            let d = nodes[x].distance(nodes[y]);
            adj[x].push((y as u32, d));
            adj[y].push((x as u32, d));
        }
    }
    // Straight-line distance from every node to every bone.
    let air: Vec<Vec<f32>> = nodes.iter().map(|p| bones.iter().map(|s| distance_to_segment(*p, s)).collect()).collect();
    let nearest: Vec<f32> = air.iter().map(|d| d.iter().copied().fold(f32::MAX, f32::min)).collect();

    // Distance across the surface from each bone's own skin. Positive floats
    // order the same as their bits, which is what the heap compares.
    let mut across = vec![vec![f32::INFINITY; bones.len()]; m];
    // A bone's weight a quarter of the model away is nothing next to the
    // bones there, so no search goes further: with sixty-odd bones over a
    // whole body, searching everywhere for each was seconds.
    let reach = 0.25 * size;
    for (b, _) in bones.iter().enumerate() {
        let mut dist = vec![f32::INFINITY; m];
        let mut heap = BinaryHeap::new();
        for v in 0..m {
            // Near-nearest, so a vertex at a joint starts both bones.
            if air[v][b] <= nearest[v] * 1.1 + 1e-6 * size {
                dist[v] = air[v][b];
                heap.push(Reverse((air[v][b].to_bits(), v as u32)));
            }
        }
        while let Some(Reverse((bits, v))) = heap.pop() {
            let d = f32::from_bits(bits);
            if d > dist[v as usize] || d > reach {
                continue;
            }
            for &(u, w) in &adj[v as usize] {
                let nd = d + w;
                if nd < dist[u as usize] {
                    dist[u as usize] = nd;
                    heap.push(Reverse((nd.to_bits(), u)));
                }
            }
        }
        for v in 0..m {
            across[v][b] = dist[v];
        }
    }
    let eps = 1e-4 * size;
    let per_node: Vec<Vec<(usize, f32)>> = (0..m)
        .map(|v| {
            bones
                .iter()
                .enumerate()
                .map(|(b, s)| {
                    // Not reached within the search's reach: no weight. Every
                    // vertex starts its own nearest bone, so none is left with
                    // nothing.
                    let d = across[v][b];
                    (s.joint as usize, if d.is_finite() { 1.0 / ((d / size) + eps / size).powf(sharpness) } else { 0.0 })
                })
                .collect()
        })
        .collect();
    node_of.iter().map(|&id| per_node[id].clone()).collect()
}

/// Sum the weights of a joint that appears more than once (a bone with
/// several children has a segment to each).
fn merge(raw: Vec<(usize, f32)>) -> Vec<(usize, f32)> {
    let mut out: Vec<(usize, f32)> = Vec::new();
    for (j, w) in raw {
        match out.iter_mut().find(|(k, _)| *k == j) {
            Some(slot) => slot.1 += w,
            None => out.push((j, w)),
        }
    }
    out
}

/// Which vertices share an edge with which, from a triangle list.
pub fn adjacency(indices: &[u32], vertex_count: usize) -> Vec<Vec<u32>> {
    let mut adj: Vec<Vec<u32>> = vec![Vec::new(); vertex_count];
    for tri in indices.chunks_exact(3) {
        for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let (ai, bi) = (a as usize, b as usize);
            if ai >= vertex_count || bi >= vertex_count {
                continue;
            }
            if !adj[ai].contains(&b) {
                adj[ai].push(b);
            }
            if !adj[bi].contains(&a) {
                adj[bi].push(a);
            }
        }
    }
    adj
}

/// A joint's weight on one vertex (0 when it is not among the four).
pub fn weight_of(inf: &Influences, joint: u16) -> f32 {
    inf.iter().filter(|(j, _)| *j == joint).map(|(_, w)| w).sum()
}

/// Every influence of `inf` as a list, empty slots dropped.
fn spread(inf: &Influences) -> Vec<(usize, f32)> {
    merge(inf.iter().filter(|(_, w)| *w > 0.0).map(|(j, w)| (*j as usize, *w)).collect())
}

/// Blur the weights across the mesh's edges: each pass moves every vertex
/// `amount` of the way towards the average of its neighbours. Takes the
/// stair-steps out of generated weights, which is what makes a bent elbow
/// fold instead of crease.
pub fn smooth(weights: &mut [Influences], adjacency: &[Vec<u32>], iterations: usize, amount: f32) {
    for _ in 0..iterations {
        let before = weights.to_vec();
        for (v, inf) in weights.iter_mut().enumerate() {
            let Some(neighbours) = adjacency.get(v).filter(|n| !n.is_empty()) else { continue };
            let mut avg: Vec<(usize, f32)> = Vec::new();
            for &n in neighbours {
                for (j, w) in spread(&before[n as usize]) {
                    avg.push((j, w / neighbours.len() as f32));
                }
            }
            let mut mixed: Vec<(usize, f32)> = spread(&before[v]).into_iter().map(|(j, w)| (j, w * (1.0 - amount))).collect();
            mixed.extend(merge(avg).into_iter().map(|(j, w)| (j, w * amount)));
            *inf = top_four(merge(mixed), inf[0].0 as usize);
        }
    }
}

/// Give `joint` exactly `value` on this vertex and share what is left among
/// the others in the proportions they had. A joint not yet among the four
/// takes the slot of the weakest. A vertex with nothing else on it keeps all
/// of its weight on `joint` whatever `value` asks, since the weights must
/// still sum to 1.
pub fn set_weight(inf: &mut Influences, joint: u16, value: f32) {
    let value = value.clamp(0.0, 1.0);
    let mut others: Vec<(usize, f32)> = spread(inf).into_iter().filter(|(j, _)| *j != joint as usize).collect();
    others.sort_by(|a, b| b.1.total_cmp(&a.1));
    others.truncate(3);
    let rest: f32 = others.iter().map(|(_, w)| w).sum();
    let mut out: Vec<(usize, f32)> = Vec::with_capacity(4);
    if rest <= 1e-6 {
        out.push((joint as usize, 1.0));
    } else {
        out.push((joint as usize, value));
        out.extend(others.into_iter().map(|(j, w)| (j, w / rest * (1.0 - value))));
    }
    *inf = top_four(out, joint as usize);
}

/// What a brush stroke does to the painted joint's weight.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Brush {
    /// Towards 1.
    Add,
    /// Towards 0.
    Remove,
    /// Towards the neighbours' average.
    Smooth,
    /// Towards the brush's strength, exactly.
    Set,
}

/// The joint's new weight after one dab: `amount` is the brush's strength
/// times its falloff at this vertex, `strength` the brush's own setting (the
/// target of [`Brush::Set`]), and `neighbours` the average weight around the
/// vertex (for [`Brush::Smooth`]).
pub fn dab(brush: Brush, current: f32, amount: f32, strength: f32, neighbours: f32) -> f32 {
    let target = match brush {
        Brush::Add => 1.0,
        Brush::Remove => 0.0,
        Brush::Smooth => neighbours,
        Brush::Set => strength,
    };
    current + (target - current) * amount.clamp(0.0, 1.0)
}

/// How hard the brush presses at `t` (0 at its centre, 1 at its rim).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Falloff {
    Smooth,
    Sharp,
    Flat,
}

pub fn falloff(kind: Falloff, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match kind {
        Falloff::Smooth => 1.0 - crate::skin::smoothstep(0.0, 1.0, t),
        Falloff::Sharp => (1.0 - t) * (1.0 - t),
        Falloff::Flat => 1.0,
    }
}

/// What the Checks list reports: vertices no bone moves, and vertices whose
/// weights do not add up to 1 (which the GPU would scale the vertex by).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Health {
    pub unweighted: usize,
    pub not_normalised: usize,
}

pub fn health(weights: &[Influences]) -> Health {
    let mut h = Health::default();
    for inf in weights {
        let sum: f32 = inf.iter().map(|(_, w)| w).sum();
        if sum <= 1e-4 {
            h.unweighted += 1;
        } else if (sum - 1.0).abs() > 1e-3 {
            h.not_normalised += 1;
        }
    }
    h
}

/// Scale every vertex's weights to sum to 1.
pub fn normalise(weights: &mut [Influences]) {
    for inf in weights.iter_mut() {
        *inf = top_four(spread(inf), inf[0].0 as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sum(inf: &Influences) -> f32 {
        inf.iter().map(|(_, w)| w).sum()
    }

    /// Two fingers side by side, joined only at the bottom: the inner edge of
    /// the left one is nearer the right bone through the air than it looks,
    /// but far from it across the skin.
    #[test]
    fn surface_weights_do_not_jump_the_gap_between_fingers() {
        let xs = [-0.04, 0.04, 0.06, 0.14];
        let rows = 11;
        let mut positions = Vec::new();
        for r in 0..rows {
            for x in xs {
                positions.push(Vec3::new(x, r as f32 * 0.1, 0.0));
            }
        }
        let at = |r: usize, c: usize| (r * 4 + c) as u32;
        let mut indices = Vec::new();
        for r in 0..rows - 1 {
            // Each finger; the gap between them only bridged in the first row.
            for c in [0, 2] {
                indices.extend([at(r, c), at(r, c + 1), at(r + 1, c), at(r, c + 1), at(r + 1, c + 1), at(r + 1, c)]);
            }
            if r == 0 {
                indices.extend([at(0, 1), at(0, 2), at(1, 1), at(0, 2), at(1, 2), at(1, 1)]);
            }
        }
        let bones = [
            Segment { joint: 0, a: Vec3::new(0.0, 0.1, 0.0), b: Vec3::new(0.0, 1.0, 0.0) },
            Segment { joint: 1, a: Vec3::new(0.1, 0.1, 0.0), b: Vec3::new(0.1, 1.0, 0.0) },
        ];
        let inner_edge = at(9, 1) as usize;
        let through_air = envelope(&positions, &bones, 6.0);
        let across_skin = surface(&positions, &indices, &bones, 6.0);
        assert!(weight_of(&through_air[inner_edge], 1) > 0.05, "the envelope bleeds: {:?}", through_air[inner_edge]);
        assert!(weight_of(&across_skin[inner_edge], 1) < 0.001, "across the skin it does not: {:?}", across_skin[inner_edge]);
        assert!((sum(&across_skin[inner_edge]) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn envelope_goes_to_the_nearest_bone_and_sums_to_one() {
        let bones = [
            Segment { joint: 0, a: Vec3::ZERO, b: Vec3::Y },
            Segment { joint: 1, a: Vec3::Y, b: Vec3::Y * 2.0 },
        ];
        let w = envelope(&[Vec3::new(0.1, 0.3, 0.0), Vec3::new(0.1, 1.7, 0.0)], &bones, 6.0);
        assert!(weight_of(&w[0], 0) > 0.9 && weight_of(&w[1], 1) > 0.9);
        assert!(w.iter().all(|i| (sum(i) - 1.0).abs() < 1e-4));
    }

    #[test]
    fn set_weight_keeps_the_rest_in_proportion() {
        let mut inf: Influences = [(0, 0.5), (1, 0.3), (2, 0.2), (0, 0.0)];
        set_weight(&mut inf, 0, 0.8);
        assert!((weight_of(&inf, 0) - 0.8).abs() < 1e-4);
        assert!((weight_of(&inf, 1) / weight_of(&inf, 2) - 1.5).abs() < 1e-3);
        assert!((sum(&inf) - 1.0).abs() < 1e-4);
        // A new joint takes a slot.
        set_weight(&mut inf, 7, 0.4);
        assert!((weight_of(&inf, 7) - 0.4).abs() < 1e-4 && (sum(&inf) - 1.0).abs() < 1e-4);
        // Nothing else to give the weight to: it stays whole.
        let mut lone: Influences = [(3, 1.0), (0, 0.0), (0, 0.0), (0, 0.0)];
        set_weight(&mut lone, 3, 0.2);
        assert!((weight_of(&lone, 3) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn smoothing_blends_a_hard_edge() {
        // A strip of four vertices, two on each bone.
        let mut w: Vec<Influences> = vec![[(0, 1.0), (0, 0.0), (0, 0.0), (0, 0.0)]; 2];
        w.extend(vec![[(1, 1.0), (0, 0.0), (0, 0.0), (0, 0.0)]; 2]);
        let adj = adjacency(&[0, 1, 2, 1, 2, 3], 4);
        smooth(&mut w, &adj, 1, 0.5);
        assert!(weight_of(&w[1], 1) > 0.0 && weight_of(&w[2], 0) > 0.0);
        assert!(w.iter().all(|i| (sum(i) - 1.0).abs() < 1e-4));
    }

    #[test]
    fn brushes_move_towards_their_targets() {
        assert!((dab(Brush::Add, 0.2, 0.5, 0.9, 0.0) - 0.6).abs() < 1e-5);
        assert!((dab(Brush::Remove, 0.8, 0.5, 0.9, 0.0) - 0.4).abs() < 1e-5);
        assert!((dab(Brush::Set, 0.0, 1.0, 0.3, 0.0) - 0.3).abs() < 1e-5);
        assert!((dab(Brush::Smooth, 1.0, 1.0, 0.0, 0.25) - 0.25).abs() < 1e-5);
        assert!(falloff(Falloff::Smooth, 0.0) > 0.99 && falloff(Falloff::Smooth, 1.0) < 0.01);
    }

    #[test]
    fn health_counts_problems() {
        let w = vec![[(0, 1.0), (0, 0.0), (0, 0.0), (0, 0.0)], [(0, 0.0); 4], [(0, 0.5), (1, 0.2), (0, 0.0), (0, 0.0)]];
        assert_eq!(health(&w), Health { unweighted: 1, not_normalised: 1 });
    }
}
