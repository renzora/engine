//! Architecture: walls, roofs, openings and the trim that dresses them.
//!
//! Sizes are real ones (a 2.2 m door frame, a 1 m railing) rather than unit
//! cells, because these are placed against each other and against a player
//! capsule, and a doorway that only fits after scaling is one more step on
//! every placement.
//!
//! Wherever an outline can be drawn as one simple polygon it is extruded as
//! one, rather than assembled from boxes: a battlement or an archway built
//! that way is a single closed surface with no internal faces and no seams
//! across its front.

use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

use super::builder::{Frame, LathePt, MeshBuilder, ProfilePt};

/// A classical column, 2.9 m: square plinth, round moulded base, a shaft with
/// a slight taper, a cushion capital and a square abacus.
pub fn create_column_mesh() -> Mesh {
    let mut b = MeshBuilder::new();
    b.cuboid(Vec3::new(-0.35, 0.0, -0.35), Vec3::new(0.35, 0.15, 0.35));
    b.cuboid(Vec3::new(-0.36, 2.75, -0.36), Vec3::new(0.36, 2.9, 0.36));

    let mut p = vec![
        LathePt::new(0.0, 0.15, Vec2::NEG_Y),
        LathePt::new(0.28, 0.15, Vec2::NEG_Y),
    ];
    // Torus moulding at the foot: a half round, normals straight out of its
    // centre. It stops short of the plinth's edge so the two never share a
    // silhouette line.
    let (mc, mr) = (Vec2::new(0.28, 0.2), 0.05);
    for i in 0..=8 {
        let t = -FRAC_PI_2 + PI * i as f32 / 8.0;
        let d = Vec2::new(t.cos(), t.sin());
        let q = mc + d * mr;
        p.push(LathePt::new(q.x, q.y, d));
    }
    // Step in to the shaft.
    p.push(LathePt::new(0.28, 0.25, Vec2::Y));
    p.push(LathePt::new(0.25, 0.25, Vec2::Y));
    // Shaft, tapering 3 cm over its height: entasis, so it does not read as
    // a pipe.
    let shaft_n = Vec2::new(2.25, 0.03);
    p.push(LathePt::new(0.25, 0.25, shaft_n));
    p.push(LathePt::new(0.22, 2.5, shaft_n));
    // Echinus: a quarter ellipse bulging out from the shaft to the abacus.
    let (ec, ea, eb) = (Vec2::new(0.22, 2.75), 0.1, 0.25);
    for i in 0..=8 {
        let t = FRAC_PI_2 * i as f32 / 8.0;
        let (s, c) = t.sin_cos();
        p.push(LathePt::new(ec.x + ea * s, ec.y - eb * c, Vec2::new(s / ea, -c / eb)));
    }
    p.push(LathePt::new(0.32, 2.75, Vec2::Y));
    p.push(LathePt::new(0.0, 2.75, Vec2::Y));
    b.lathe(&p, 32);
    b.center_bounds();
    b.build()
}

/// A gable roof with eaves: 2.3 m across, 2.4 m long, 0.12 m thick slabs at
/// a 37 degree pitch, open underneath like the real thing.
pub fn create_gable_roof_mesh() -> Mesh {
    let (half_span, rise, thick, eave) = (1.0, 0.75, 0.12, 0.15);
    let slope = rise / half_span;
    let x = half_span + eave;
    let under = |x: f32| rise - slope * x.abs();
    let profile = [
        ProfilePt::hard(-x, under(-x)),
        ProfilePt::hard(0.0, rise),
        ProfilePt::hard(x, under(x)),
        ProfilePt::hard(x, under(x) + thick),
        ProfilePt::hard(0.0, rise + thick),
        ProfilePt::hard(-x, under(-x) + thick),
    ];
    let mut b = MeshBuilder::new();
    b.extrude(&profile, Vec3::new(0.0, 0.0, -1.2), Vec3::new(0.0, 0.0, 1.2), Vec3::X, Vec3::Y);
    b.center_bounds();
    b.build()
}

/// A solid hip roof over a 2 m by 3 m footprint, every slope at 37 degrees.
pub fn create_hip_roof_mesh() -> Mesh {
    let (hx, hz, rise) = (1.0, 1.5, 0.75);
    // Equal pitch on all four sides puts the ridge ends one half-width in.
    let ridge = hz - hx;
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    let (r0, r1) = (v(0.0, rise, -ridge), v(0.0, rise, ridge));
    let mut b = MeshBuilder::new();
    b.polyhedron(&[
        vec![v(-hx, 0.0, -hz), v(hx, 0.0, -hz), v(hx, 0.0, hz), v(-hx, 0.0, hz)],
        vec![v(hx, 0.0, -hz), v(hx, 0.0, hz), r1, r0],
        vec![v(-hx, 0.0, -hz), v(-hx, 0.0, hz), r1, r0],
        vec![v(-hx, 0.0, hz), v(hx, 0.0, hz), r1],
        vec![v(-hx, 0.0, -hz), v(hx, 0.0, -hz), r0],
    ]);
    b.center_bounds();
    b.build()
}

/// A hemispherical shell dome, 2 m across with a 0.1 m wall, hollow inside.
pub fn create_dome_shell_mesh() -> Mesh {
    let (outer, inner, rings) = (1.0, 0.9, 16);
    let mut p = vec![
        LathePt::new(inner, 0.0, Vec2::NEG_Y),
        LathePt::new(outer, 0.0, Vec2::NEG_Y),
    ];
    for i in 0..=rings {
        let t = FRAC_PI_2 * i as f32 / rings as f32;
        let (s, c) = t.sin_cos();
        p.push(LathePt::new(outer * c, outer * s, Vec2::new(c, s)));
    }
    for i in 0..=rings {
        let t = FRAC_PI_2 * (rings - i) as f32 / rings as f32;
        let (s, c) = t.sin_cos();
        p.push(LathePt::new(inner * c, inner * s, Vec2::new(-c, -s)));
    }
    let mut b = MeshBuilder::new();
    b.lathe(&p, 48);
    b.center_bounds();
    b.build()
}

/// A 3 m by 3 m wall, 0.5 m thick, with a round-headed opening 1.6 m wide and
/// 2.4 m to the crown.
///
/// The soffit is part of the same outline as the jambs, smooth along the
/// curve and creased where it meets them, so the opening is one surface.
pub fn create_archway_mesh() -> Mesh {
    let (w, h, half_open, spring, steps) = (3.0, 3.0, 0.8, 1.6, 16);
    let (l, r) = (w / 2.0 - half_open, w / 2.0 + half_open);
    let centre = Vec2::new(w / 2.0, spring);
    let mut profile = vec![ProfilePt::hard(0.0, 0.0), ProfilePt::hard(l, 0.0)];
    for i in 0..=steps {
        let t = PI - PI * i as f32 / steps as f32;
        let d = Vec2::new(t.cos(), t.sin());
        let q = centre + d * half_open;
        // Into the opening, which is out of the wall.
        profile.push(ProfilePt::smooth(q.x, q.y, -d));
    }
    profile.extend([ProfilePt::hard(r, 0.0), ProfilePt::hard(w, 0.0), ProfilePt::hard(w, h), ProfilePt::hard(0.0, h)]);
    let mut b = MeshBuilder::new();
    b.extrude(&profile, Vec3::ZERO, Vec3::new(0.0, 0.0, 0.5), Vec3::X, Vec3::Y);
    b.center_bounds();
    b.build()
}

/// A castle wall: 3 m long, 2 m to the wall walk, 0.4 m thick, topped with
/// five 0.4 m merlons.
pub fn create_crenellated_wall_mesh() -> Mesh {
    let (len, height, merlon_h, merlon_w, count) = (3.0, 2.0, 0.4, 0.4, 5);
    let gap = (len - merlon_w * count as f32) / (count - 1) as f32;
    let top = height + merlon_h;
    let mut profile = vec![ProfilePt::hard(0.0, 0.0), ProfilePt::hard(len, 0.0)];
    for i in (0..count).rev() {
        let left = i as f32 * (merlon_w + gap);
        profile.push(ProfilePt::hard(left + merlon_w, top));
        profile.push(ProfilePt::hard(left, top));
        if i > 0 {
            profile.push(ProfilePt::hard(left, height));
            profile.push(ProfilePt::hard(left - gap, height));
        }
    }
    let mut b = MeshBuilder::new();
    b.extrude(&profile, Vec3::ZERO, Vec3::new(0.0, 0.0, 0.4), Vec3::X, Vec3::Y);
    b.center_bounds();
    b.build()
}

/// A 2 m railing, 1 m to the top of the handrail: end posts, a bottom rail
/// and square balusters.
pub fn create_railing_mesh() -> Mesh {
    let mut b = MeshBuilder::new();
    let (half, post, top) = (1.0, 0.04, 0.94);
    for x in [-half + post, half - post] {
        b.cuboid(Vec3::new(x - post, 0.0, -post), Vec3::new(x + post, top, post));
    }
    b.cuboid(Vec3::new(-half, top, -0.05), Vec3::new(half, 1.0, 0.05));
    let inner = half - 2.0 * post;
    b.cuboid(Vec3::new(-inner, 0.1, -0.025), Vec3::new(inner, 0.15, 0.025));
    let count = 13;
    for i in 0..count {
        let x = -inner + 2.0 * inner * (i as f32 + 1.0) / (count as f32 + 1.0);
        b.cuboid(Vec3::new(x - 0.015, 0.15, -0.015), Vec3::new(x + 0.015, top, 0.015));
    }
    b.center_bounds();
    b.build()
}

/// A 2 m picket fence, 1.2 m posts, with pointed pickets nailed to two rails.
pub fn create_fence_mesh() -> Mesh {
    let mut b = MeshBuilder::new();
    let half = 1.0;
    for x in [-half + 0.05, half - 0.05] {
        b.cuboid(Vec3::new(x - 0.05, 0.0, -0.05), Vec3::new(x + 0.05, 1.2, 0.05));
    }
    for y in [0.3, 0.85] {
        b.cuboid(Vec3::new(-half, y, 0.05), Vec3::new(half, y + 0.08, 0.09));
    }
    let count = 8;
    let span = 2.0 * half - 0.2;
    for i in 0..count {
        let cx = -span / 2.0 + span * (i as f32 + 0.5) / count as f32;
        let (l, r) = (cx - 0.06, cx + 0.06);
        let picket = [
            ProfilePt::hard(l, 0.05),
            ProfilePt::hard(r, 0.05),
            ProfilePt::hard(r, 1.0),
            ProfilePt::hard(cx, 1.08),
            ProfilePt::hard(l, 1.0),
        ];
        b.extrude(&picket, Vec3::new(0.0, 0.0, 0.09), Vec3::new(0.0, 0.0, 0.11), Vec3::X, Vec3::Y);
    }
    b.center_bounds();
    b.build()
}

/// A rectangular ring extruded along Z: the body of a window or door casing.
fn rect_ring(b: &mut MeshBuilder, outer: (Vec2, Vec2), inner: (Vec2, Vec2), z0: f32, z1: f32) {
    let corners = |(lo, hi): (Vec2, Vec2)| {
        [Vec2::new(hi.x, hi.y), Vec2::new(lo.x, hi.y), Vec2::new(lo.x, lo.y), Vec2::new(hi.x, lo.y)]
    };
    let o: Vec<ProfilePt> = corners(outer).iter().map(|p| ProfilePt::hard(p.x, p.y)).collect();
    let i: Vec<ProfilePt> = corners(inner).iter().map(|p| ProfilePt::hard(p.x, p.y)).collect();
    let frames = [
        Frame { origin: Vec3::new(0.0, 0.0, z0), side: Vec3::X, up: Vec3::Y, forward: Vec3::Z },
        Frame { origin: Vec3::new(0.0, 0.0, z1), side: Vec3::X, up: Vec3::Y, forward: Vec3::Z },
    ];
    b.sweep(&o, &frames, false, false);
    b.sweep(&i, &frames, true, false);
    for (z, n) in [(z0, -Vec3::Z), (z1, Vec3::Z)] {
        let ov: Vec<Vec3> = o.iter().map(|p| p.p.extend(z)).collect();
        let iv: Vec<Vec3> = i.iter().map(|p| p.p.extend(z)).collect();
        b.ring(&ov, &iv, n);
    }
}

/// A window frame 1.2 m by 1.4 m with a cross mullion and a projecting sill.
///
/// The mullions sit back from the frame's faces and the horizontal one is
/// split either side of the vertical, so no two parts share a visible plane
/// and nothing z-fights.
pub fn create_window_frame_mesh() -> Mesh {
    let mut b = MeshBuilder::new();
    let (ox, oy, ix, iy, d) = (0.6, 0.7, 0.45, 0.55, 0.075);
    rect_ring(&mut b, (Vec2::new(-ox, -oy), Vec2::new(ox, oy)), (Vec2::new(-ix, -iy), Vec2::new(ix, iy)), -d, d);
    b.cuboid(Vec3::new(-0.7, -oy - 0.08, -d), Vec3::new(0.7, -oy, 0.15));
    let m = 0.025;
    b.cuboid(Vec3::new(-m, -iy, -0.03), Vec3::new(m, iy, 0.03));
    b.cuboid(Vec3::new(-ix, -m, -0.03), Vec3::new(-m, m, 0.03));
    b.cuboid(Vec3::new(m, -m, -0.03), Vec3::new(ix, m, 0.03));
    b.center_bounds();
    b.build()
}

/// A door casing: 1.2 m by 2.2 m outside, a 0.9 m by 2.05 m opening, 0.2 m
/// deep. Open at the floor, so it is one U-shaped outline.
pub fn create_door_frame_mesh() -> Mesh {
    let profile = [
        ProfilePt::hard(-0.6, 0.0),
        ProfilePt::hard(-0.45, 0.0),
        ProfilePt::hard(-0.45, 2.05),
        ProfilePt::hard(0.45, 2.05),
        ProfilePt::hard(0.45, 0.0),
        ProfilePt::hard(0.6, 0.0),
        ProfilePt::hard(0.6, 2.2),
        ProfilePt::hard(-0.6, 2.2),
    ];
    let mut b = MeshBuilder::new();
    b.extrude(&profile, Vec3::new(0.0, 0.0, -0.1), Vec3::new(0.0, 0.0, 0.1), Vec3::X, Vec3::Y);
    b.center_bounds();
    b.build()
}

/// An L-shaped wall: two 2 m arms, 0.2 m thick, 2 m tall, so a room corner
/// is one piece with no overlap to hide.
pub fn create_wall_corner_mesh() -> Mesh {
    let t = 0.2;
    let profile = [
        ProfilePt::hard(0.0, 0.0),
        ProfilePt::hard(2.0, 0.0),
        ProfilePt::hard(2.0, t),
        ProfilePt::hard(t, t),
        ProfilePt::hard(t, 2.0),
        ProfilePt::hard(0.0, 2.0),
    ];
    let mut b = MeshBuilder::new();
    b.extrude(&profile, Vec3::ZERO, Vec3::new(0.0, 2.0, 0.0), Vec3::X, Vec3::Z);
    b.center_bounds();
    b.build()
}

/// A 1 m floor tile, 0.1 m thick, with a 2 cm chamfer round its top edge, so
/// a floor of them reads as tiles instead of one slab.
pub fn create_floor_tile_mesh() -> Mesh {
    let (h, t, c) = (0.5, 0.1, 0.02);
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    let ring = |s: f32, y: f32| [v(-s, y, -s), v(s, y, -s), v(s, y, s), v(-s, y, s)];
    let low = ring(h, 0.0);
    let mid = ring(h, t - c);
    let top = ring(h - c, t);
    let mut faces = vec![low.to_vec(), top.to_vec()];
    for i in 0..4 {
        let j = (i + 1) % 4;
        faces.push(vec![low[i], low[j], mid[j], mid[i]]);
        faces.push(vec![mid[i], mid[j], top[j], top[i]]);
    }
    let mut b = MeshBuilder::new();
    b.polyhedron(&faces);
    b.center_bounds();
    b.build()
}

/// A 2 m by 1.2 m balcony slab with a 0.15 m lip on its three open sides; the
/// back edge is left bare to butt against a wall.
pub fn create_balcony_mesh() -> Mesh {
    let (hx, d, t, lip_h, lip_w) = (1.0, 1.2, 0.2, 0.15, 0.08);
    let mut b = MeshBuilder::new();
    b.cuboid(Vec3::new(-hx, 0.0, 0.0), Vec3::new(hx, t, d));
    let top = t + lip_h;
    b.cuboid(Vec3::new(-hx, t, d - lip_w), Vec3::new(hx, top, d));
    b.cuboid(Vec3::new(-hx, t, 0.0), Vec3::new(-hx + lip_w, top, d - lip_w));
    b.cuboid(Vec3::new(hx - lip_w, t, 0.0), Vec3::new(hx, top, d - lip_w));
    b.center_bounds();
    b.build()
}
