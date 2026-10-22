//! Stairs and ramps beyond the straight flight: turning stairs, a curved
//! ramp, a ladder and a flight with a sloped underside.
//!
//! Flights use the same proportions as the original Stairs shape (six steps
//! per metre of rise and run), so a turning stair and a straight one placed
//! together climb at the same pitch and their treads line up.

use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

use super::builder::{Frame, MeshBuilder, ProfilePt, circle_profile};

/// Steps per flight, and per metre of rise.
const STEPS: u32 = 6;

/// The side outline of a flight, solid to the ground: `(run, height)` points,
/// counter-clockwise. `base` lifts the first tread (a flight leaving a
/// landing); `extra` stretches the top tread into a landing.
fn flight_profile(base: f32, extra: f32) -> Vec<ProfilePt> {
    let d = 1.0 / STEPS as f32;
    let h = 1.0 / STEPS as f32;
    let mut p = vec![
        ProfilePt::hard(0.0, 0.0),
        ProfilePt::hard(1.0 + extra, 0.0),
        ProfilePt::hard(1.0 + extra, base + 1.0),
    ];
    for i in (0..STEPS).rev() {
        if i < STEPS - 1 {
            p.push(ProfilePt::hard((i + 1) as f32 * d, base + (i + 1) as f32 * h));
        }
        p.push(ProfilePt::hard(i as f32 * d, base + (i + 1) as f32 * h));
    }
    p
}

/// Stairs that turn 90 degrees at a landing: two six-step flights, 1 m wide,
/// 2 m rise overall.
pub fn create_l_stairs_mesh() -> Mesh {
    let mut b = MeshBuilder::new();
    // First flight runs up +Z and carries the landing on its top tread.
    b.extrude(&flight_profile(0.0, 1.0), Vec3::ZERO, Vec3::X, Vec3::Z, Vec3::Y);
    // Second flight leaves the landing along +X.
    b.extrude(&flight_profile(1.0, 0.0), Vec3::new(1.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 2.0), Vec3::X, Vec3::Y);
    b.center_bounds();
    b.build()
}

/// Stairs that double back at a half landing: two six-step flights side by
/// side, 2 m wide overall, 2 m rise.
pub fn create_u_stairs_mesh() -> Mesh {
    let mut b = MeshBuilder::new();
    b.extrude(&flight_profile(0.0, 1.0), Vec3::ZERO, Vec3::X, Vec3::Z, Vec3::Y);
    // The landing's other half, under the second flight's start.
    b.cuboid(Vec3::new(1.0, 0.0, 1.0), Vec3::new(2.0, 1.0, 2.0));
    // Second flight runs back down -Z from the landing edge.
    b.extrude(&flight_profile(1.0, 0.0), Vec3::new(1.0, 0.0, 1.0), Vec3::new(2.0, 0.0, 1.0), -Vec3::Z, Vec3::Y);
    b.center_bounds();
    b.build()
}

/// A ramp that climbs 1 m while sweeping a quarter turn: inner radius 1 m,
/// outer 2.5 m, solid to the ground.
///
/// The running surface is a helicoid, not a plane, so it is tessellated
/// across its width as well as along it and shaded with its analytic normal;
/// two big triangles per step would show a crease down each one.
pub fn create_curved_ramp_mesh() -> Mesh {
    let (r0, r1, rise, sweep) = (1.0, 2.5, 1.0, FRAC_PI_2);
    let (along, across) = (32, 6);
    let k = rise / sweep;
    let at = |t: f32, r: f32| Vec3::new(r * t.cos(), k * t, r * t.sin());
    let mut b = MeshBuilder::new();

    b.grid(along, across, |i, j| {
        let t = sweep * i as f32 / along as f32;
        let r = r0 + (r1 - r0) * j as f32 / across as f32;
        let (s, c) = t.sin_cos();
        let n = Vec3::new(k * s, r, -k * c).normalize();
        (at(t, r), n, Vec2::new(t * r, r))
    });
    // Outer and inner walls, smooth round the curve.
    for (r, sign) in [(r1, 1.0), (r0, -1.0)] {
        b.grid(along, 1, |i, j| {
            let t = sweep * i as f32 / along as f32;
            let (s, c) = t.sin_cos();
            let y = if j == 0 { 0.0 } else { k * t };
            (Vec3::new(r * c, y, r * s), Vec3::new(c, 0.0, s) * sign, Vec2::new(t * r, y))
        });
    }
    // Underside.
    b.grid(along, 1, |i, j| {
        let t = sweep * i as f32 / along as f32;
        let r = if j == 0 { r0 } else { r1 };
        let p = Vec3::new(r * t.cos(), 0.0, r * t.sin());
        (p, -Vec3::Y, Vec2::new(p.x, p.z))
    });
    // The tall end.
    let (s, c) = sweep.sin_cos();
    b.face(
        &[
            Vec3::new(r0 * c, 0.0, r0 * s),
            Vec3::new(r1 * c, 0.0, r1 * s),
            Vec3::new(r1 * c, rise, r1 * s),
            Vec3::new(r0 * c, rise, r0 * s),
        ],
        Vec3::new(-s, 0.0, c),
    );
    b.center_bounds();
    b.build()
}

/// A 2.5 m ladder, 0.5 m between the rails, with round rungs every 0.3 m.
pub fn create_ladder_mesh() -> Mesh {
    let (height, half_w, rail_w, rail_d) = (2.5, 0.25, 0.03, 0.02);
    let mut b = MeshBuilder::new();
    for x in [-half_w, half_w] {
        b.cuboid(Vec3::new(x - rail_w, 0.0, -rail_d), Vec3::new(x + rail_w, height, rail_d));
    }
    // Rungs run into the rails, so their capped ends are hidden inside.
    let rung = circle_profile(0.018, 12);
    let mut y = 0.25;
    while y < height - 0.1 {
        let frames = [
            Frame { origin: Vec3::new(-half_w, y, 0.0), side: Vec3::Z, up: Vec3::Y, forward: Vec3::X },
            Frame { origin: Vec3::new(half_w, y, 0.0), side: Vec3::Z, up: Vec3::Y, forward: Vec3::X },
        ];
        b.sweep(&rung, &frames, false, true);
        y += 0.3;
    }
    b.center_bounds();
    b.build()
}

/// A six-step flight with a sloped soffit instead of a solid base: the
/// floating stair of a mezzanine, 1 m rise and run, 1 m wide.
pub fn create_slab_stairs_mesh() -> Mesh {
    let d = 1.0 / STEPS as f32;
    let thick = 0.25;
    let mut p = vec![
        ProfilePt::hard(0.0, 0.0),
        ProfilePt::hard(thick, 0.0),
        ProfilePt::hard(1.0, 1.0 - thick),
        ProfilePt::hard(1.0, 1.0),
    ];
    for i in (0..STEPS).rev() {
        if i < STEPS - 1 {
            p.push(ProfilePt::hard((i + 1) as f32 * d, (i + 1) as f32 * d));
        }
        p.push(ProfilePt::hard(i as f32 * d, (i + 1) as f32 * d));
    }
    let mut b = MeshBuilder::new();
    b.extrude(&p, Vec3::ZERO, Vec3::X, Vec3::Z, Vec3::Y);
    b.center_bounds();
    b.build()
}
