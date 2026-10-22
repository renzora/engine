//! Primitives: the regular solids and simple round things a blockout reaches
//! for when a cube or sphere is not quite it.
//!
//! The polyhedra are flat shaded on purpose. Smoothing an icosahedron's
//! normals makes it read as a badly tessellated sphere; the faceted look is the
//! reason to pick one over the sphere.

use bevy::prelude::*;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

use super::builder::{LathePt, MeshBuilder, ProfilePt, hard_lathe};

/// The golden ratio, which is what places an icosahedron's corners.
const PHI: f32 = 1.618_034;

/// Regular tetrahedron, 1 m edge, resting on a face.
pub fn create_tetrahedron_mesh() -> Mesh {
    let r = 1.0 / 3f32.sqrt();
    let h = (2.0f32 / 3.0).sqrt();
    let base: Vec<Vec3> = (0..3)
        .map(|i| {
            let t = FRAC_PI_2 + TAU * i as f32 / 3.0;
            Vec3::new(r * t.cos(), 0.0, r * t.sin())
        })
        .collect();
    let apex = Vec3::new(0.0, h, 0.0);
    let mut b = MeshBuilder::new();
    b.polyhedron(&[
        vec![base[0], base[1], base[2]],
        vec![base[0], base[1], apex],
        vec![base[1], base[2], apex],
        vec![base[2], base[0], apex],
    ]);
    b.center_bounds();
    b.build()
}

/// Regular octahedron, corners 0.5 m out along each axis.
pub fn create_octahedron_mesh() -> Mesh {
    let mut faces = Vec::new();
    for sx in [-0.5, 0.5] {
        for sy in [-0.5, 0.5] {
            for sz in [-0.5, 0.5] {
                faces.push(vec![Vec3::new(sx, 0.0, 0.0), Vec3::new(0.0, sy, 0.0), Vec3::new(0.0, 0.0, sz)]);
            }
        }
    }
    let mut b = MeshBuilder::new();
    b.polyhedron(&faces);
    b.build()
}

/// The icosahedron's 12 corners on a sphere of radius 0.5, and its 20 faces
/// as corner indices.
///
/// The faces are found rather than listed: every triple of corners that are
/// mutually one edge apart. Twenty hand-typed index triples is exactly the
/// kind of table that hides one transposed digit.
fn icosahedron() -> (Vec<Vec3>, Vec<[usize; 3]>) {
    let mut v = Vec::new();
    for a in [-1.0, 1.0] {
        for b in [-PHI, PHI] {
            v.push(Vec3::new(0.0, a, b));
            v.push(Vec3::new(a, b, 0.0));
            v.push(Vec3::new(b, 0.0, a));
        }
    }
    let v: Vec<Vec3> = v.into_iter().map(|p| p.normalize() * 0.5).collect();
    let edge = v
        .iter()
        .skip(1)
        .map(|p| p.distance(v[0]))
        .fold(f32::MAX, f32::min);
    let near = |a: usize, b: usize| (v[a].distance(v[b]) - edge).abs() < 1e-3;
    let mut faces = Vec::new();
    for i in 0..v.len() {
        for j in i + 1..v.len() {
            for k in j + 1..v.len() {
                if near(i, j) && near(j, k) && near(i, k) {
                    faces.push([i, j, k]);
                }
            }
        }
    }
    (v, faces)
}

/// Regular icosahedron, circumradius 0.5 m.
pub fn create_icosahedron_mesh() -> Mesh {
    let (v, faces) = icosahedron();
    let faces: Vec<Vec<Vec3>> = faces.iter().map(|f| f.iter().map(|&i| v[i]).collect()).collect();
    let mut b = MeshBuilder::new();
    b.polyhedron(&faces);
    b.build()
}

/// Regular dodecahedron, circumradius 0.5 m.
///
/// Built as the icosahedron's dual: each icosahedron face becomes a corner,
/// each icosahedron corner a pentagon. Pushing the face centres back out to
/// one radius keeps every pentagon planar, because the dual of a regular solid
/// is regular.
pub fn create_dodecahedron_mesh() -> Mesh {
    let (v, faces) = icosahedron();
    let corners: Vec<Vec3> = faces
        .iter()
        .map(|f| ((v[f[0]] + v[f[1]] + v[f[2]]) / 3.0).normalize() * 0.5)
        .collect();
    let mut pentagons = Vec::new();
    for (i, axis) in v.iter().enumerate() {
        let axis = axis.normalize();
        let u = axis.any_orthonormal_vector();
        let w = axis.cross(u);
        let mut ring: Vec<Vec3> = faces
            .iter()
            .zip(&corners)
            .filter(|(f, _)| f.contains(&i))
            .map(|(_, c)| *c)
            .collect();
        ring.sort_by(|a, b| {
            let ta = a.dot(w).atan2(a.dot(u));
            let tb = b.dot(w).atan2(b.dot(u));
            ta.total_cmp(&tb)
        });
        pentagons.push(ring);
    }
    let mut b = MeshBuilder::new();
    b.polyhedron(&pentagons);
    b.build()
}

/// The positions along one axis of a rounded box face: the flat middle, plus
/// `k` steps around each rounded edge spaced evenly in *angle*.
///
/// Spacing them evenly along the flat face instead (the usual quick version)
/// bunches the facets near the edge's midline and leaves the tangent ends
/// coarse, which shows as a faceted highlight on exactly the part of a bevel a
/// light catches.
fn rounded_axis_samples(h: f32, r: f32, k: u32) -> Vec<f32> {
    let a = h - r;
    let mut s = Vec::new();
    for j in 0..=k {
        let t = FRAC_PI_4 * (k - j) as f32 / k as f32;
        s.push(-a - r * t.tan());
    }
    for j in 0..=k {
        let t = FRAC_PI_4 * j as f32 / k as f32;
        s.push(a + r * t.tan());
    }
    s
}

/// A 1 m cube with every edge and corner rounded to a 0.12 m radius.
///
/// Each face is a grid whose border rows are pushed onto the rounding by
/// clamping to the inner box and stepping out by the radius. Two faces meet
/// on the 45 degree line of an edge, where both compute the same point and
/// the same normal from the same sample, so the join is exact.
pub fn create_rounded_cube_mesh() -> Mesh {
    let (h, r, k) = (0.5, 0.12, 4);
    let a = h - r;
    let s = rounded_axis_samples(h, r, k);
    let n = s.len() as u32 - 1;
    let mut b = MeshBuilder::new();
    for (normal, u, v) in [
        (Vec3::X, Vec3::Z, Vec3::Y),
        (-Vec3::X, -Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::X, Vec3::Z),
        (-Vec3::Y, Vec3::X, -Vec3::Z),
        (Vec3::Z, -Vec3::X, Vec3::Y),
        (-Vec3::Z, Vec3::X, Vec3::Y),
    ] {
        b.grid(n, n, |i, j| {
            let cube = normal * h + u * s[j as usize] + v * s[i as usize];
            let inner = cube.clamp(Vec3::splat(-a), Vec3::splat(a));
            let dir = (cube - inner).normalize();
            let p = inner + dir * r;
            (p, dir, Vec2::new(p.dot(u), p.dot(v)))
        });
    }
    b.build()
}

/// A flat disc, 1 m across and 5 cm thick.
pub fn create_disc_mesh() -> Mesh {
    let t = 0.05;
    let profile = hard_lathe(
        &[Vec2::new(0.0, 0.0), Vec2::new(0.5, 0.0), Vec2::new(0.5, t), Vec2::new(0.0, t)],
        false,
    );
    let mut b = MeshBuilder::new();
    b.lathe(&profile, 48);
    b.center_bounds();
    b.build()
}

/// A five-pointed star standing upright, 1 m tip to tip, 0.2 m thick.
pub fn create_star_prism_mesh() -> Mesh {
    let profile: Vec<ProfilePt> = (0..10)
        .map(|i| {
            let t = FRAC_PI_2 + TAU * i as f32 / 10.0;
            let r = if i % 2 == 0 { 0.5 } else { 0.2 };
            ProfilePt::hard(r * t.cos(), r * t.sin())
        })
        .collect();
    let mut b = MeshBuilder::new();
    b.extrude(&profile, Vec3::new(0.0, 0.0, -0.1), Vec3::new(0.0, 0.0, 0.1), Vec3::X, Vec3::Y);
    b.center_bounds();
    b.build()
}

/// An ellipsoid of revolution: 0.7 m across, 1 m tall.
pub fn create_ellipsoid_mesh() -> Mesh {
    let (a, c) = (0.35, 0.5);
    let rings = 24;
    let profile: Vec<LathePt> = (0..=rings)
        .map(|i| {
            let t = -FRAC_PI_2 + std::f32::consts::PI * i as f32 / rings as f32;
            let (s, co) = t.sin_cos();
            // The gradient of x^2/a^2 + y^2/c^2, which is the true normal; the
            // radial direction is only right on a sphere.
            LathePt::new(a * co, c * s, Vec2::new(co / a, s / c))
        })
        .collect();
    let mut b = MeshBuilder::new();
    b.lathe(&profile, 32);
    b.build()
}

/// A hexagonal prism standing on end, 1 m across the corners, 1 m tall.
pub fn create_hex_prism_mesh() -> Mesh {
    let profile: Vec<ProfilePt> = (0..6)
        .map(|i| {
            let t = TAU * i as f32 / 6.0;
            ProfilePt::hard(0.5 * t.cos(), 0.5 * t.sin())
        })
        .collect();
    let mut b = MeshBuilder::new();
    b.extrude(&profile, Vec3::new(0.0, -0.5, 0.0), Vec3::new(0.0, 0.5, 0.0), Vec3::X, Vec3::Z);
    b.build()
}

/// A cylinder with a hemispherical top: a silo, a bollard, a capsule that
/// stands flat. 1 m across and 1 m tall.
///
/// The side and the dome share their normal at the join, so the profile has
/// no crease there and the shading rolls over the shoulder.
pub fn create_domed_cylinder_mesh() -> Mesh {
    let r = 0.5;
    let wall = 0.5;
    let mut profile = vec![
        LathePt::new(0.0, 0.0, Vec2::NEG_Y),
        LathePt::new(r, 0.0, Vec2::NEG_Y),
        LathePt::new(r, 0.0, Vec2::X),
    ];
    let steps = 12;
    for i in 0..=steps {
        let t = FRAC_PI_2 * i as f32 / steps as f32;
        let (s, c) = t.sin_cos();
        profile.push(LathePt::new(r * c, wall + r * s, Vec2::new(c, s)));
    }
    let mut b = MeshBuilder::new();
    b.lathe(&profile, 32);
    b.center_bounds();
    b.build()
}

/// Deterministic per-index noise in `0..1`, so a rock is the same rock in
/// every session, every scene reload and every exported build.
pub(crate) fn hash01(i: u32, seed: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ seed.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x & 0x00FF_FFFF) as f32 / 0x00FF_FFFF as f32
}

/// Icosphere corners and faces after `levels` midpoint subdivisions, on the
/// unit sphere. Shared corners are shared indices, which is what lets a
/// per-corner displacement leave the surface closed.
pub(crate) fn icosphere(levels: u32) -> (Vec<Vec3>, Vec<[usize; 3]>) {
    let (v, faces) = icosahedron();
    let mut v: Vec<Vec3> = v.into_iter().map(|p| p.normalize()).collect();
    let mut faces = faces;
    for _ in 0..levels {
        let mut mid: HashMap<(usize, usize), usize> = HashMap::new();
        let mut next = Vec::new();
        for f in &faces {
            let mut m = [0usize; 3];
            for e in 0..3 {
                let (a, b) = (f[e], f[(e + 1) % 3]);
                let key = (a.min(b), a.max(b));
                m[e] = *mid.entry(key).or_insert_with(|| {
                    v.push(((v[a] + v[b]) * 0.5).normalize());
                    v.len() - 1
                });
            }
            next.push([f[0], m[0], m[2]]);
            next.push([f[1], m[1], m[0]]);
            next.push([f[2], m[2], m[1]]);
            next.push([m[0], m[1], m[2]]);
        }
        faces = next;
    }
    (v, faces)
}
