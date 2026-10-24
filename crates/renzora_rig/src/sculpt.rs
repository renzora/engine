//! Sculpting brushes: the math that moves a surface under a brush, kept apart
//! from any mesh or UI so each brush can be tested on a handful of points.
//!
//! The animation editor's Shapes mode sculpts blend shapes with these. It
//! works on **points**, not on a mesh's vertices: a model's vertices are split
//! wherever a UV seam, a hard edge or a material change runs, so two vertices
//! share a place without sharing an edge. Moving those independently tears the
//! seam open the first time Smooth or Inflate (which read neighbours and
//! normals) touches it, so [`weld`] first folds every vertex into the point
//! it sits on, and the brushes move points.
//!
//! The brushes follow the usual sculpting core, simplified: a per-point factor
//! (falloff times strength), one shared "area normal" per dab for Draw and
//! Flatten so a stroke over noisy normals still pushes one way, each point's
//! own normal for Inflate, and a stroke mirrored across X by running the dab
//! twice and blending the points both passes reach ([`merge_mirrored`]).

use std::collections::HashMap;

use bevy::math::Vec3;

use crate::weights::{falloff, Falloff};

/// What a dab does to the surface under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum Brush {
    /// Push along the surface (pull with invert).
    #[default]
    Draw,
    /// Relax toward the neighbours' average.
    Smooth,
    /// Drag the points under the brush with the cursor.
    Grab,
    /// Grow along each point's own normal (shrink with invert).
    Inflate,
    /// Pull onto the plane the points under the brush average to.
    Flatten,
    /// Draw toward the brush's centre (spread with invert).
    Pinch,
}

impl Brush {
    pub const ALL: [Brush; 6] = [Brush::Draw, Brush::Smooth, Brush::Grab, Brush::Inflate, Brush::Flatten, Brush::Pinch];

    pub fn label(self) -> &'static str {
        match self {
            Brush::Draw => "Draw",
            Brush::Smooth => "Smooth",
            Brush::Grab => "Grab",
            Brush::Inflate => "Inflate",
            Brush::Flatten => "Flatten",
            Brush::Pinch => "Pinch",
        }
    }
}

/// One brush application.
#[derive(Clone, Copy, Debug)]
pub struct Dab {
    pub centre: Vec3,
    pub radius: f32,
    /// 0 to 1.
    pub strength: f32,
    pub falloff: Falloff,
    pub invert: bool,
}

/// The points in reach of a dab, with how strongly each is moved: the falloff
/// at its distance times the strength. `allow` keeps a stroke to the mesh it
/// started on, so sculpting lips does not drag the teeth behind them.
pub fn reach(positions: &[Vec3], dab: &Dab, allow: impl Fn(usize) -> bool) -> Vec<(u32, f32)> {
    let r = dab.radius.max(1e-6);
    positions
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let d = p.distance(dab.centre);
            if d >= r || !allow(i) {
                return None;
            }
            let f = falloff(dab.falloff, d / r) * dab.strength;
            (f > 0.0).then_some((i as u32, f))
        })
        .collect()
}

/// How far each reached point moves for one dab of `brush`. Grab is not a dab
/// (it drags what it caught at the start of the stroke, [`grab`]) and moves
/// nothing here.
pub fn dab(brush: Brush, positions: &[Vec3], normals: &[Vec3], neighbours: &[Vec<u32>], d: &Dab, reached: &[(u32, f32)]) -> Vec<(u32, Vec3)> {
    if reached.is_empty() {
        return Vec::new();
    }
    let flip = if d.invert { -1.0 } else { 1.0 };
    let normal = |i: u32| normals.get(i as usize).copied().unwrap_or(Vec3::ZERO);
    let pos = |i: u32| positions[i as usize];
    // The factor-weighted average of the reached normals: one direction for
    // the whole dab, so a crease does not fold in on itself.
    let area = reached.iter().map(|(i, f)| normal(*i) * *f).sum::<Vec3>().normalize_or_zero();
    match brush {
        Brush::Draw => reached.iter().map(|(i, f)| (*i, area * d.radius * 0.25 * flip * *f)).collect(),
        Brush::Inflate => reached.iter().map(|(i, f)| (*i, normal(*i) * d.radius * 0.2 * flip * *f)).collect(),
        Brush::Pinch => reached.iter().map(|(i, f)| (*i, (d.centre - pos(*i)) * 0.5 * flip * *f)).collect(),
        Brush::Flatten => {
            let total: f32 = reached.iter().map(|(_, f)| *f).sum();
            let centroid = reached.iter().map(|(i, f)| pos(*i) * *f).sum::<Vec3>() / total.max(1e-6);
            reached.iter().map(|(i, f)| (*i, area * area.dot(centroid - pos(*i)) * *f)).collect()
        }
        Brush::Smooth => reached
            .iter()
            .filter_map(|(i, f)| {
                let ns = neighbours.get(*i as usize).filter(|n| !n.is_empty())?;
                let avg = ns.iter().map(|&n| positions[n as usize]).sum::<Vec3>() / ns.len() as f32;
                Some((*i, (avg - pos(*i)) * 0.5 * *f))
            })
            .collect(),
        Brush::Grab => Vec::new(),
    }
}

/// Grab: where each caught point goes when the cursor has moved `offset` from
/// where the stroke started, relative to where the point was then.
pub fn grab(caught: &[(u32, f32)], offset: Vec3) -> Vec<(u32, Vec3)> {
    caught.iter().map(|(i, f)| (*i, offset * *f)).collect()
}

/// Mirror a point or a direction across the model's X = 0 plane.
pub fn mirror(v: Vec3) -> Vec3 {
    Vec3::new(-v.x, v.y, v.z)
}

/// Combine a dab's moves with its mirrored pass, given each pass's reach
/// ([`reach`]: the factor each point was moved with). A point only one pass
/// reaches takes that pass's move. A point both reach (the middle of the face,
/// under two big brushes) takes the average of the two weighted by their
/// factors, so on the centre line, where the factors are equal, its X
/// movement cancels and it is not pushed twice as far.
///
/// The weighting is what keeps the surface whole. A plain average halved a
/// point's move the moment the other pass's rim crossed it, where that pass's
/// factor is near zero: a cheek stroke with mirroring tore a step into the
/// face along the edge of the mirrored brush, around everything both brushes
/// covered. Weighted, a pass fading out at its rim fades out of the blend too.
pub fn merge_mirrored(a: Vec<(u32, Vec3)>, a_reach: &[(u32, f32)], b: Vec<(u32, Vec3)>, b_reach: &[(u32, f32)]) -> Vec<(u32, Vec3)> {
    let factors = |r: &[(u32, f32)]| r.iter().copied().collect::<HashMap<u32, f32>>();
    let (fa, fb) = (factors(a_reach), factors(b_reach));
    let mut out: HashMap<u32, (Vec3, f32)> = HashMap::with_capacity(a.len() + b.len());
    for (moves, f) in [(a, &fa), (b, &fb)] {
        for (i, m) in moves {
            let w = f.get(&i).copied().unwrap_or(1.0).max(1e-6);
            let e = out.entry(i).or_insert((Vec3::ZERO, 0.0));
            // Each move is already its factor times a full move, so the
            // weighted sum divides back by the total weight.
            e.0 += m * w;
            e.1 += w;
        }
    }
    let mut v: Vec<(u32, Vec3)> = out.into_iter().map(|(i, (m, w))| (i, m / w)).collect();
    v.sort_by_key(|(i, _)| *i);
    v
}

/// Fold vertices that sit on the same place into one point. `group` keeps
/// vertices of different meshes apart even where they touch (an eyelid and the
/// lashes on it). Returns each vertex's point and every point's position.
pub fn weld(positions: &[Vec3], group: &[u32], tolerance: f32) -> (Vec<u32>, Vec<Vec3>) {
    let cell = tolerance.max(1e-7);
    let mut seen: HashMap<(u32, i64, i64, i64), u32> = HashMap::with_capacity(positions.len());
    let mut map = Vec::with_capacity(positions.len());
    let mut points = Vec::new();
    for (i, p) in positions.iter().enumerate() {
        let key = (
            group.get(i).copied().unwrap_or(0),
            (p.x / cell).round() as i64,
            (p.y / cell).round() as i64,
            (p.z / cell).round() as i64,
        );
        let id = *seen.entry(key).or_insert_with(|| {
            points.push(*p);
            (points.len() - 1) as u32
        });
        map.push(id);
    }
    (map, points)
}

/// Area-weighted normals of a triangle list over `positions`. A point no
/// triangle uses gets a zero normal.
pub fn normals(positions: &[Vec3], triangles: &[[u32; 3]]) -> Vec<Vec3> {
    let mut out = vec![Vec3::ZERO; positions.len()];
    for t in triangles {
        let (Some(a), Some(b), Some(c)) = (positions.get(t[0] as usize), positions.get(t[1] as usize), positions.get(t[2] as usize)) else {
            continue;
        };
        // Not normalised: the cross product's length is twice the area, which
        // is the weighting wanted.
        let n = (*b - *a).cross(*c - *a);
        for &i in t {
            out[i as usize] += n;
        }
    }
    for n in &mut out {
        *n = n.normalize_or_zero();
    }
    out
}

/// Which points share an edge with which, from triangles over points. A
/// triangle that welding collapsed (two corners on one point) adds no edge
/// from a point to itself.
pub fn neighbours(triangles: &[[u32; 3]], count: usize) -> Vec<Vec<u32>> {
    let mut adj: Vec<Vec<u32>> = vec![Vec::new(); count];
    for t in triangles {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            if a == b || a as usize >= count || b as usize >= count {
                continue;
            }
            if !adj[a as usize].contains(&b) {
                adj[a as usize].push(b);
            }
            if !adj[b as usize].contains(&a) {
                adj[b as usize].push(a);
            }
        }
    }
    adj
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat 5 x 5 grid in the XY plane facing +Z, one unit apart, centred.
    fn grid() -> (Vec<Vec3>, Vec<[u32; 3]>) {
        let mut p = Vec::new();
        for y in 0..5 {
            for x in 0..5 {
                p.push(Vec3::new(x as f32 - 2.0, y as f32 - 2.0, 0.0));
            }
        }
        let mut t = Vec::new();
        for y in 0..4u32 {
            for x in 0..4u32 {
                let i = y * 5 + x;
                t.push([i, i + 1, i + 6]);
                t.push([i, i + 6, i + 5]);
            }
        }
        (p, t)
    }

    fn dab_at(centre: Vec3, radius: f32) -> Dab {
        Dab { centre, radius, strength: 1.0, falloff: Falloff::Smooth, invert: false }
    }

    #[test]
    fn draw_pushes_along_the_surface_and_invert_pulls() {
        let (p, t) = grid();
        let n = normals(&p, &t);
        let d = dab_at(Vec3::ZERO, 1.5);
        let reached = reach(&p, &d, |_| true);
        let centre = 12;
        assert!(reached.iter().any(|(i, f)| *i == centre && (*f - 1.0).abs() < 1e-5));
        let moves = dab(Brush::Draw, &p, &n, &[], &d, &reached);
        let m = moves.iter().find(|(i, _)| *i == centre).unwrap().1;
        assert!(m.z > 0.0 && m.x.abs() < 1e-6 && m.y.abs() < 1e-6);
        let inv = dab(Brush::Draw, &p, &n, &[], &Dab { invert: true, ..d }, &reached);
        assert!(inv.iter().find(|(i, _)| *i == centre).unwrap().1.z < 0.0);
        // Nothing past the radius moves.
        assert!(reached.iter().all(|(i, _)| p[*i as usize].length() < 1.5));
    }

    #[test]
    fn falloff_is_full_at_the_centre_and_gone_at_the_rim() {
        let p = vec![Vec3::ZERO, Vec3::X * 0.5, Vec3::X * 0.999, Vec3::X * 2.0];
        let r = reach(&p, &dab_at(Vec3::ZERO, 1.0), |_| true);
        let f = |i: u32| r.iter().find(|(j, _)| *j == i).map(|(_, f)| *f);
        assert!((f(0).unwrap() - 1.0).abs() < 1e-6);
        assert!((f(1).unwrap() - 0.5).abs() < 1e-6, "smoothstep is a half at half the radius");
        assert!(f(2).unwrap() < 1e-3);
        assert!(f(3).is_none());
        // And `allow` keeps points out.
        assert!(reach(&p, &dab_at(Vec3::ZERO, 1.0), |i| i != 0).iter().all(|(i, _)| *i != 0));
    }

    #[test]
    fn smooth_pulls_a_spike_back_toward_its_neighbours() {
        let (mut p, t) = grid();
        p[12].z = 1.0;
        let n = normals(&p, &t);
        let adj = neighbours(&t, p.len());
        let d = dab_at(p[12], 0.5);
        let reached = reach(&p, &d, |_| true);
        let moves = dab(Brush::Smooth, &p, &n, &adj, &d, &reached);
        let m = moves.iter().find(|(i, _)| *i == 12).unwrap().1;
        assert!(m.z < -0.3, "the spike comes down, got {m}");
    }

    #[test]
    fn flatten_brings_points_onto_their_average_plane() {
        let (mut p, t) = grid();
        p[12].z = 0.4;
        p[7].z = -0.4;
        let n = normals(&p, &t);
        let d = Dab { falloff: Falloff::Flat, ..dab_at(Vec3::ZERO, 10.0) };
        let reached = reach(&p, &d, |_| true);
        let moves = dab(Brush::Flatten, &p, &n, &[], &d, &reached);
        let after: Vec<Vec3> = p.iter().enumerate().map(|(i, q)| *q + moves.iter().find(|(j, _)| *j as usize == i).map(|m| m.1).unwrap_or(Vec3::ZERO)).collect();
        let spread = after.iter().map(|q| q.z).fold(f32::MIN, f32::max) - after.iter().map(|q| q.z).fold(f32::MAX, f32::min);
        assert!(spread < 0.05, "flat after one full-strength dab, spread {spread}");
    }

    #[test]
    fn pinch_draws_in_and_inflate_grows_along_each_normal() {
        let (p, t) = grid();
        let n = normals(&p, &t);
        let d = dab_at(Vec3::ZERO, 2.5);
        let reached = reach(&p, &d, |_| true);
        let pinch = dab(Brush::Pinch, &p, &n, &[], &d, &reached);
        let m = pinch.iter().find(|(i, _)| *i == 13).unwrap().1;
        assert!(m.x < 0.0, "a point right of centre moves left");
        let inflate = dab(Brush::Inflate, &p, &n, &[], &d, &reached);
        assert!(inflate.iter().all(|(_, m)| m.z >= 0.0 && m.x.abs() < 1e-6));
    }

    #[test]
    fn grab_moves_by_the_offset_times_the_factor() {
        let moves = grab(&[(0, 1.0), (1, 0.25)], Vec3::new(0.0, 2.0, 0.0));
        assert_eq!(moves, vec![(0, Vec3::new(0.0, 2.0, 0.0)), (1, Vec3::new(0.0, 0.5, 0.0))]);
    }

    #[test]
    fn a_mirrored_stroke_keeps_the_centre_line_on_it() {
        let a = vec![(0, Vec3::new(0.3, 1.0, 0.0)), (1, Vec3::X)];
        let b = vec![(0, mirror(Vec3::new(0.3, 1.0, 0.0))), (2, -Vec3::X)];
        let m = merge_mirrored(a, &[(0, 0.5), (1, 0.5)], b, &[(0, 0.5), (2, 0.5)]);
        assert_eq!(m.len(), 3);
        assert_eq!(m[0], (0, Vec3::new(0.0, 1.0, 0.0)));
        assert_eq!(m[1], (1, Vec3::X));
        assert_eq!(m[2], (2, -Vec3::X));
    }

    /// The tear found sculpting a cheek with mirroring on: two big brushes
    /// either side of the centre overlap across the middle of the face, and
    /// the move must fade smoothly across the edge of each, not halve where
    /// the other brush's rim crosses.
    #[test]
    fn a_mirrored_stroke_has_no_step_where_the_brushes_overlap() {
        // A dense strip along X, facing +Z.
        let n = 801;
        let p: Vec<Vec3> = (0..n).map(|i| Vec3::new(i as f32 / 200.0 - 2.0, 0.0, 0.0)).collect();
        let normals = vec![Vec3::Z; n];
        for brush in [Brush::Inflate, Brush::Draw, Brush::Pinch] {
            let d = dab_at(Vec3::new(0.6, 0.0, 0.0), 1.0);
            let m = Dab { centre: mirror(d.centre), ..d };
            let (ra, rb) = (reach(&p, &d, |_| true), reach(&p, &m, |_| true));
            let moves = merge_mirrored(dab(brush, &p, &normals, &[], &d, &ra), &ra, dab(brush, &p, &normals, &[], &m, &rb), &rb);
            let mut out = vec![Vec3::ZERO; n];
            for (i, v) in moves {
                out[i as usize] = v;
            }
            // Neighbours 5 mm apart: a smooth falloff moves them nearly alike;
            // the old plain average jumped by half a full move at each rim.
            let step = out.windows(2).map(|w| (w[1] - w[0]).length()).fold(0.0f32, f32::max);
            let full = out.iter().map(|v| v.length()).fold(0.0f32, f32::max);
            assert!(step < full * 0.05, "{brush:?}: step {step} against a largest move of {full}");
            // And it stays mirrored: the middle keeps to the centre line.
            assert!(out[400].x.abs() < 1e-6, "{brush:?}");
        }
    }

    #[test]
    fn welding_joins_a_seam_but_not_two_meshes() {
        let p = vec![Vec3::ZERO, Vec3::X, Vec3::new(1e-6, 0.0, 0.0), Vec3::ZERO];
        let (map, points) = weld(&p, &[0, 0, 0, 1], 1e-4);
        assert_eq!(map[0], map[2], "a split vertex is one point");
        assert_ne!(map[0], map[3], "the same place on another mesh is not");
        assert_eq!(points.len(), 3);
        let adj = neighbours(&[[0, 0, 1]], 2);
        assert_eq!(adj[0], vec![1], "no edge from a point to itself");
    }
}
