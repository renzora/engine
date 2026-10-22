//! Blocks: solid level-building pieces that tile with the 1 m cube and wedge.
//!
//! Each one fills (or exactly fills part of) a 1 m cell, so a corner wedge
//! drops in beside a wedge and the slopes line up without nudging.

use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, TAU};

use super::builder::{MeshBuilder, ProfilePt};

/// A 1 m cube with every edge chamfered by 8 cm.
///
/// A chamfer instead of a fillet: flat bevels catch light as a crisp line
/// along every edge, which is the look a beveled block is chosen for, and
/// cost 26 faces where a rounded cube costs hundreds.
pub fn create_beveled_block_mesh() -> Mesh {
    let (h, c) = (0.5, 0.08);
    let a = h - c;
    let mut faces: Vec<Vec<Vec3>> = Vec::new();
    // Main faces.
    for axis in 0..3 {
        for s in [-1.0, 1.0] {
            let corner = |u: f32, v: f32| {
                let mut p = [0.0; 3];
                p[axis] = s * h;
                p[(axis + 1) % 3] = u;
                p[(axis + 2) % 3] = v;
                Vec3::from(p)
            };
            faces.push(vec![corner(-a, -a), corner(a, -a), corner(a, a), corner(-a, a)]);
        }
    }
    // Edge chamfers: one per pair of signs on two axes.
    for axis in 0..3 {
        let (p, q) = ((axis + 1) % 3, (axis + 2) % 3);
        for sp in [-1.0, 1.0] {
            for sq in [-1.0, 1.0] {
                let pt = |along: f32, on_p: bool| {
                    let mut v = [0.0; 3];
                    v[axis] = along;
                    v[p] = sp * if on_p { h } else { a };
                    v[q] = sq * if on_p { a } else { h };
                    Vec3::from(v)
                };
                faces.push(vec![pt(-a, true), pt(a, true), pt(a, false), pt(-a, false)]);
            }
        }
    }
    // Corner triangles.
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                faces.push(vec![
                    Vec3::new(sx * h, sy * a, sz * a),
                    Vec3::new(sx * a, sy * h, sz * a),
                    Vec3::new(sx * a, sy * a, sz * h),
                ]);
            }
        }
    }
    let mut b = MeshBuilder::new();
    b.polyhedron(&faces);
    b.build()
}

/// A 1 m block with its front top quarter cut away: two 0.5 m steps.
pub fn create_step_block_mesh() -> Mesh {
    let profile = [
        ProfilePt::hard(0.0, 0.0),
        ProfilePt::hard(1.0, 0.0),
        ProfilePt::hard(1.0, 1.0),
        ProfilePt::hard(0.5, 1.0),
        ProfilePt::hard(0.5, 0.5),
        ProfilePt::hard(0.0, 0.5),
    ];
    let mut b = MeshBuilder::new();
    b.extrude(&profile, Vec3::ZERO, Vec3::X, Vec3::Z, Vec3::Y);
    b.center_bounds();
    b.build()
}

/// Where two wedges meet at an outside corner: a 1 m cell under
/// `y <= min(1 - x, 1 - z)`, peaking at one corner.
pub fn create_outer_corner_wedge_mesh() -> Mesh {
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    let apex = v(0.0, 1.0, 0.0);
    let mut b = MeshBuilder::new();
    b.polyhedron(&[
        vec![v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 0.0, 1.0), v(0.0, 0.0, 1.0)],
        vec![v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0), apex],
        vec![v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), apex],
        vec![v(1.0, 0.0, 0.0), v(1.0, 0.0, 1.0), apex],
        vec![v(0.0, 0.0, 1.0), v(1.0, 0.0, 1.0), apex],
    ]);
    b.center_bounds();
    b.build()
}

/// Where two wedges meet at an inside corner: a 1 m cell under
/// `y <= max(1 - x, 1 - z)`, with a valley running down the diagonal.
///
/// Not convex, so every face states which way it faces instead of trusting
/// the centroid.
pub fn create_inner_corner_wedge_mesh() -> Mesh {
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    let slope_x = Vec3::new(1.0, 1.0, 0.0);
    let slope_z = Vec3::new(0.0, 1.0, 1.0);
    let mut b = MeshBuilder::new();
    b.face(&[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 0.0, 1.0), v(0.0, 0.0, 1.0)], -Vec3::Y);
    b.face(&[v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0), v(0.0, 1.0, 1.0), v(0.0, 1.0, 0.0)], -Vec3::X);
    b.face(&[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0), v(0.0, 1.0, 0.0)], -Vec3::Z);
    b.face(&[v(0.0, 1.0, 0.0), v(0.0, 1.0, 1.0), v(1.0, 0.0, 1.0)], slope_x);
    b.face(&[v(0.0, 1.0, 0.0), v(1.0, 1.0, 0.0), v(1.0, 0.0, 1.0)], slope_z);
    b.face(&[v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0), v(1.0, 0.0, 1.0)], Vec3::X);
    b.face(&[v(0.0, 0.0, 1.0), v(0.0, 1.0, 1.0), v(1.0, 0.0, 1.0)], Vec3::Z);
    b.center_bounds();
    b.build()
}

/// A 2:1 slope: 2 m long, 1 m tall, 1 m wide. Steeper than the ramp, gentler
/// than the wedge, and walkable at most character controllers' default limit.
pub fn create_slope_2_1_mesh() -> Mesh {
    let profile = [ProfilePt::hard(0.0, 0.0), ProfilePt::hard(2.0, 0.0), ProfilePt::hard(0.0, 1.0)];
    let mut b = MeshBuilder::new();
    b.extrude(&profile, Vec3::ZERO, Vec3::Z, Vec3::X, Vec3::Y);
    b.center_bounds();
    b.build()
}

/// A 1 m block with its four vertical edges rounded to 0.15 m, top and bottom
/// left sharp: the footprint of a planter, a kerb end, a bollard base.
pub fn create_rounded_edge_block_mesh() -> Mesh {
    let (h, r, steps) = (0.5, 0.15, 8);
    let a = h - r;
    let mut profile = Vec::new();
    for (k, (cx, cy)) in [(a, a), (-a, a), (-a, -a), (a, -a)].into_iter().enumerate() {
        let start = FRAC_PI_2 * k as f32;
        for i in 0..=steps {
            let t = start + FRAC_PI_2 * i as f32 / steps as f32;
            let (s, c) = t.sin_cos();
            profile.push(ProfilePt::smooth(cx + r * c, cy + r * s, Vec2::new(c, s)));
        }
    }
    let mut b = MeshBuilder::new();
    b.extrude(&profile, Vec3::new(0.0, -h, 0.0), Vec3::new(0.0, h, 0.0), Vec3::X, Vec3::Z);
    b.build()
}

/// A 12-tooth gear lying flat, 1 m across the tips, 0.2 m thick, with a bore.
///
/// The bore is sampled at the same angles as the teeth, so the two faces are
/// a single band of quads between matching points rather than a polygon with
/// a hole, which ear clipping cannot take.
pub fn create_gear_mesh() -> Mesh {
    let (tip, root, bore, half_t, teeth) = (0.5, 0.4, 0.12, 0.1, 12);
    let pitch = TAU / teeth as f32;
    let mut angles_radii = Vec::new();
    for k in 0..teeth {
        let a0 = pitch * k as f32;
        angles_radii.push((a0, root));
        angles_radii.push((a0 + 0.15 * pitch, tip));
        angles_radii.push((a0 + 0.45 * pitch, tip));
        angles_radii.push((a0 + 0.6 * pitch, root));
    }
    let outer: Vec<ProfilePt> = angles_radii
        .iter()
        .map(|&(t, r)| ProfilePt::hard(r * t.cos(), r * t.sin()))
        .collect();
    let inner: Vec<ProfilePt> = angles_radii
        .iter()
        .map(|&(t, _)| ProfilePt::smooth(bore * t.cos(), bore * t.sin(), Vec2::new(t.cos(), t.sin())))
        .collect();
    let bottom = Vec3::new(0.0, -half_t, 0.0);
    let top = Vec3::new(0.0, half_t, 0.0);
    let frames = [
        super::builder::Frame { origin: bottom, side: Vec3::X, up: Vec3::Z, forward: Vec3::Y },
        super::builder::Frame { origin: top, side: Vec3::X, up: Vec3::Z, forward: Vec3::Y },
    ];
    let mut b = MeshBuilder::new();
    b.sweep(&outer, &frames, false, false);
    b.sweep(&inner, &frames, true, false);
    let lay = |p: &ProfilePt, y: f32| Vec3::new(p.p.x, y, p.p.y);
    for (y, n) in [(-half_t, -Vec3::Y), (half_t, Vec3::Y)] {
        let o: Vec<Vec3> = outer.iter().map(|p| lay(p, y)).collect();
        let i: Vec<Vec3> = inner.iter().map(|p| lay(p, y)).collect();
        b.ring(&o, &i, n);
    }
    b.build()
}

/// A square frame standing upright: 1 m outside, 0.7 m opening, 0.15 m deep.
pub fn create_frame_mesh() -> Mesh {
    let (o, i, half_d) = (0.5, 0.35, 0.075);
    let square = |s: f32| [Vec2::new(s, s), Vec2::new(-s, s), Vec2::new(-s, -s), Vec2::new(s, -s)];
    let outer: Vec<ProfilePt> = square(o).iter().map(|p| ProfilePt::hard(p.x, p.y)).collect();
    let inner: Vec<ProfilePt> = square(i).iter().map(|p| ProfilePt::hard(p.x, p.y)).collect();
    let mut b = MeshBuilder::new();
    let frames = [
        super::builder::Frame { origin: Vec3::new(0.0, 0.0, -half_d), side: Vec3::X, up: Vec3::Y, forward: Vec3::Z },
        super::builder::Frame { origin: Vec3::new(0.0, 0.0, half_d), side: Vec3::X, up: Vec3::Y, forward: Vec3::Z },
    ];
    b.sweep(&outer, &frames, false, false);
    b.sweep(&inner, &frames, true, false);
    for (z, n) in [(-half_d, -Vec3::Z), (half_d, Vec3::Z)] {
        let ov: Vec<Vec3> = outer.iter().map(|p| Vec3::new(p.p.x, p.p.y, z)).collect();
        let iv: Vec<Vec3> = inner.iter().map(|p| Vec3::new(p.p.x, p.p.y, z)).collect();
        b.ring(&ov, &iv, n);
    }
    b.build()
}
