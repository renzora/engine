//! Props: set dressing at real-world sizes, for greyboxing a space that is
//! meant to be lived in rather than only walked through.
//!
//! Most are assembled from boxes. Parts are laid out so that no two of them
//! share a visible plane facing the same way over the same area (legs are
//! inset from table edges, rails sit proud of posts), because two coplanar
//! faces z-fight and draw as a flickering seam.

use bevy::prelude::*;
use std::f32::consts::PI;

use super::builder::{LathePt, MeshBuilder, hard_lathe};
use super::primitives::{hash01, icosphere};

/// A 1 m shipping crate: a recessed core framed by twelve 8 cm edge battens.
///
/// The battens along X run the full length and own all eight corners; the
/// Y and Z ones stop at them. Overlapping all three at every corner would
/// stack three coplanar faces there.
pub fn create_crate_mesh() -> Mesh {
    let (h, t, core) = (0.5, 0.08, 0.46);
    let a = h - t;
    let mut b = MeshBuilder::new();
    b.cuboid(Vec3::splat(-core), Vec3::splat(core));
    for sy in [-1.0, 1.0] {
        for sz in [-1.0, 1.0] {
            let (y0, y1) = if sy > 0.0 { (a, h) } else { (-h, -a) };
            let (z0, z1) = if sz > 0.0 { (a, h) } else { (-h, -a) };
            b.cuboid(Vec3::new(-h, y0, z0), Vec3::new(h, y1, z1));
        }
    }
    for sx in [-1.0, 1.0] {
        for sz in [-1.0, 1.0] {
            let (x0, x1) = if sx > 0.0 { (a, h) } else { (-h, -a) };
            let (z0, z1) = if sz > 0.0 { (a, h) } else { (-h, -a) };
            b.cuboid(Vec3::new(x0, -a, z0), Vec3::new(x1, a, z1));
        }
    }
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            let (x0, x1) = if sx > 0.0 { (a, h) } else { (-h, -a) };
            let (y0, y1) = if sy > 0.0 { (a, h) } else { (-h, -a) };
            b.cuboid(Vec3::new(x0, y0, -a), Vec3::new(x1, y1, a));
        }
    }
    b.build()
}

/// A 1.1 m barrel: bulging staves, heads set in below the chime, two hoops.
pub fn create_barrel_mesh() -> Mesh {
    let (height, end_r, bulge, head_r, inset) = (1.1, 0.4, 0.06, 0.36, 0.03);
    let stave = |y: f32| end_r + bulge * (PI * y / height).sin();
    let slope = |y: f32| bulge * PI / height * (PI * y / height).cos();
    let mut p = vec![
        LathePt::new(0.0, inset, Vec2::NEG_Y),
        LathePt::new(head_r, inset, Vec2::NEG_Y),
        LathePt::new(head_r, inset, Vec2::NEG_X),
        LathePt::new(head_r, 0.0, Vec2::NEG_X),
        LathePt::new(head_r, 0.0, Vec2::NEG_Y),
        LathePt::new(end_r, 0.0, Vec2::NEG_Y),
    ];
    let steps = 16;
    for i in 0..=steps {
        let y = height * i as f32 / steps as f32;
        p.push(LathePt::new(stave(y), y, Vec2::new(1.0, -slope(y))));
    }
    p.extend([
        LathePt::new(end_r, height, Vec2::Y),
        LathePt::new(head_r, height, Vec2::Y),
        LathePt::new(head_r, height, Vec2::NEG_X),
        LathePt::new(head_r, height - inset, Vec2::NEG_X),
        LathePt::new(head_r, height - inset, Vec2::Y),
        LathePt::new(0.0, height - inset, Vec2::Y),
    ]);
    let mut b = MeshBuilder::new();
    b.lathe(&p, 32);
    // Hoops: rings whose inner face is buried in the staves, so only their
    // outer band and edges show.
    for (y0, y1) in [(0.12, 0.18), (height - 0.18, height - 0.12)] {
        let inner = stave(y0).min(stave(y1)) - 0.01;
        let outer = stave(y0).max(stave(y1)) + 0.01;
        let ring = [Vec2::new(inner, y0), Vec2::new(outer, y0), Vec2::new(outer, y1), Vec2::new(inner, y1)];
        b.lathe(&hard_lathe(&ring, true), 32);
    }
    b.center_bounds();
    b.build()
}

/// A 3 m square platform, 0.3 m deep, with a raised 15 cm trim round its top.
pub fn create_platform_mesh() -> Mesh {
    let (h, t, trim_w, trim_h) = (1.5, 0.3, 0.15, 0.08);
    let mut b = MeshBuilder::new();
    b.cuboid(Vec3::new(-h, 0.0, -h), Vec3::new(h, t, h));
    let (y0, y1) = (t, t + trim_h);
    let i = h - trim_w;
    b.cuboid(Vec3::new(-h, y0, i), Vec3::new(h, y1, h));
    b.cuboid(Vec3::new(-h, y0, -h), Vec3::new(h, y1, -i));
    b.cuboid(Vec3::new(-h, y0, -i), Vec3::new(-i, y1, i));
    b.cuboid(Vec3::new(i, y0, -i), Vec3::new(h, y1, i));
    b.center_bounds();
    b.build()
}

/// A 4 m footbridge span, 2 m wide: a deck on two girders with post-and-rail
/// sides. Open at both ends so spans chain end to end.
pub fn create_bridge_mesh() -> Mesh {
    let (hx, hz, deck) = (1.0, 2.0, 0.2);
    let mut b = MeshBuilder::new();
    b.cuboid(Vec3::new(-hx, 0.0, -hz), Vec3::new(hx, deck, hz));
    for x in [-0.75, 0.75] {
        b.cuboid(Vec3::new(x - 0.1, -0.35, -hz), Vec3::new(x + 0.1, 0.0, hz));
    }
    for side in [-1.0, 1.0] {
        let x = side * (hx - 0.07);
        // End posts stand back from the span's ends, so the rails run past
        // them instead of finishing flush on the same plane as their faces.
        for k in 0..5 {
            let z = -hz + 0.1 + (2.0 * hz - 0.2) * k as f32 / 4.0;
            b.cuboid(Vec3::new(x - 0.05, deck, z - 0.05), Vec3::new(x + 0.05, 1.1, z + 0.05));
        }
        b.cuboid(Vec3::new(x - 0.06, 1.1, -hz), Vec3::new(x + 0.06, 1.16, hz));
        b.cuboid(Vec3::new(x - 0.025, 0.6, -hz), Vec3::new(x + 0.025, 0.66, hz));
    }
    b.center_bounds();
    b.build()
}

/// A 1.5 m bench, seat at 0.48 m, on two slab legs tied by a stretcher.
pub fn create_bench_mesh() -> Mesh {
    let mut b = MeshBuilder::new();
    b.cuboid(Vec3::new(-0.75, 0.42, -0.2), Vec3::new(0.75, 0.48, 0.2));
    for x in [-0.63, 0.63] {
        b.cuboid(Vec3::new(x - 0.03, 0.0, -0.17), Vec3::new(x + 0.03, 0.42, 0.17));
    }
    b.cuboid(Vec3::new(-0.6, 0.1, -0.03), Vec3::new(0.6, 0.16, 0.03));
    b.center_bounds();
    b.build()
}

/// A 1.6 m by 0.9 m table at the usual 0.77 m height.
pub fn create_table_mesh() -> Mesh {
    let mut b = MeshBuilder::new();
    b.cuboid(Vec3::new(-0.8, 0.72, -0.45), Vec3::new(0.8, 0.77, 0.45));
    for x in [-0.685, 0.685] {
        for z in [-0.355, 0.355] {
            b.cuboid(Vec3::new(x - 0.035, 0.0, z - 0.035), Vec3::new(x + 0.035, 0.72, z + 0.035));
        }
    }
    b.center_bounds();
    b.build()
}

/// A round 1 m display pedestal: stepped base, plain shaft, stepped top.
pub fn create_pedestal_mesh() -> Mesh {
    let outline = [
        Vec2::new(0.0, 0.0),
        Vec2::new(0.4, 0.0),
        Vec2::new(0.4, 0.1),
        Vec2::new(0.33, 0.1),
        Vec2::new(0.33, 0.15),
        Vec2::new(0.22, 0.15),
        Vec2::new(0.22, 0.85),
        Vec2::new(0.3, 0.85),
        Vec2::new(0.3, 0.9),
        Vec2::new(0.38, 0.9),
        Vec2::new(0.38, 1.0),
        Vec2::new(0.0, 1.0),
    ];
    let mut b = MeshBuilder::new();
    b.lathe(&hard_lathe(&outline, false), 40);
    b.center_bounds();
    b.build()
}

/// A low-poly boulder about 1 m across.
///
/// An icosphere whose corners are pushed in and out by a fixed hash and whose
/// underside is flattened, so it sits on the ground without rocking. The hash
/// is seeded by corner index, so the rock is identical every time it is
/// built: a scene saved with one reopens with the same one. Flat shaded,
/// because a smoothed rock reads as a lumpy ball.
pub fn create_rock_mesh() -> Mesh {
    let (v, faces) = icosphere(1);
    let floor = -0.28;
    let pts: Vec<Vec3> = v
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let r = 0.5 * (0.78 + 0.32 * hash01(i as u32, 0x5EED));
            let p = *d * r * Vec3::new(1.0, 0.72, 0.9);
            Vec3::new(p.x, p.y.max(floor), p.z)
        })
        .collect();
    let mut b = MeshBuilder::new();
    for f in &faces {
        let tri = [pts[f[0]], pts[f[1]], pts[f[2]]];
        // Every corner was pushed straight out from the centre, so the rock is
        // star-shaped about it and "away from the centre" is outward.
        let outward = (v[f[0]] + v[f[1]] + v[f[2]]).normalize();
        b.face(&tri, outward);
    }
    b.center_bounds();
    b.build()
}

/// A 1.75 m signpost with a 1 m by 0.5 m board.
pub fn create_sign_post_mesh() -> Mesh {
    let mut b = MeshBuilder::new();
    b.cuboid(Vec3::new(-0.05, 0.0, -0.05), Vec3::new(0.05, 1.75, 0.05));
    b.cuboid(Vec3::new(-0.5, 1.15, -0.06), Vec3::new(0.5, 1.65, 0.06));
    b.center_bounds();
    b.build()
}

/// A 3.6 m street lamp: flared foot, slim pole, lantern and cap, all one
/// surface of revolution.
pub fn create_lamp_post_mesh() -> Mesh {
    let outline = [
        Vec2::new(0.0, 0.0),
        Vec2::new(0.2, 0.0),
        Vec2::new(0.2, 0.12),
        Vec2::new(0.12, 0.3),
        Vec2::new(0.06, 0.4),
        Vec2::new(0.06, 3.0),
        Vec2::new(0.12, 3.05),
        Vec2::new(0.2, 3.4),
        Vec2::new(0.24, 3.4),
        Vec2::new(0.24, 3.43),
        Vec2::new(0.0, 3.6),
    ];
    let mut b = MeshBuilder::new();
    b.lathe(&hard_lathe(&outline, false), 32);
    b.center_bounds();
    b.build()
}
