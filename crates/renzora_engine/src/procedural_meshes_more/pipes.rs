//! Pipes: fittings that join the existing Pipe shape, and two decorative
//! tubes (a spring and a knot).
//!
//! Every fitting uses the Pipe's own bore, 0.5 m outside and 0.35 m inside,
//! so an elbow or a tee mates with a placed Pipe face to face with no lip.
//! Their open ends are cut square to the pipe axis from analytic tangents,
//! not finite differences, for the same reason: a chord-estimated end would
//! be a couple of degrees off and leave a wedge-shaped gap at every joint.

use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use super::builder::{Frame, LathePt, MeshBuilder, ProfilePt, circle_profile, hard_lathe, transported_frames};

const OUTER: f32 = 0.5;
const INNER: f32 = 0.35;
/// Sides round every pipe here, matching the Pipe shape's 24.
const SIDES: u32 = 24;

/// A hollow tube along `frames`, outer and inner walls plus annular ends.
fn hollow_tube(b: &mut MeshBuilder, frames: &[Frame]) {
    let outer = circle_profile(OUTER, SIDES);
    let inner = circle_profile(INNER, SIDES);
    b.sweep(&outer, frames, false, false);
    b.sweep(&inner, frames, true, false);
    for (f, out) in [(frames[0], -frames[0].forward), (frames[frames.len() - 1], frames[frames.len() - 1].forward)] {
        let place = |p: &ProfilePt| f.origin + f.side * p.p.x + f.up * p.p.y;
        let o: Vec<Vec3> = outer.iter().map(place).collect();
        let i: Vec<Vec3> = inner.iter().map(place).collect();
        b.ring(&o, &i, out);
    }
}

/// A 90 degree elbow on a 1 m centreline radius: comes up out of the floor
/// and turns to run along +X.
pub fn create_pipe_elbow_mesh() -> Mesh {
    let (radius, steps) = (1.0, 24);
    let mut pts = Vec::new();
    let mut tans = Vec::new();
    for i in 0..=steps {
        let t = FRAC_PI_2 * i as f32 / steps as f32;
        let (s, c) = t.sin_cos();
        pts.push(Vec3::new(radius - radius * c, radius * s, 0.0));
        tans.push(Vec3::new(s, c, 0.0));
    }
    let frames = transported_frames(&pts, &tans, Vec3::Z, false);
    let mut b = MeshBuilder::new();
    hollow_tube(&mut b, &frames);
    b.center_bounds();
    b.build()
}

/// The outline of one square hub face, sampled at the pipe's own angles so
/// the square's corners land on samples and every face of the hub puts its
/// vertices at the same points along a shared edge (no T-junctions).
///
/// Written as `origin + u * x + w * y`, the same operation order as
/// [`Frame`] placement, so the hole's rim and the bore swept from it are the
/// same floats and not merely the same numbers.
fn square_samples(centre: Vec3, u: Vec3, w: Vec3, half: f32) -> Vec<Vec3> {
    (0..SIDES)
        .map(|i| {
            let t = TAU * i as f32 / SIDES as f32;
            let (s, c) = t.sin_cos();
            let scale = half / c.abs().max(s.abs());
            centre + u * (scale * c) + w * (scale * s)
        })
        .collect()
}

fn circle_samples(centre: Vec3, u: Vec3, w: Vec3, r: f32) -> Vec<Vec3> {
    circle_profile(r, SIDES)
        .iter()
        .map(|p| centre + u * p.p.x + w * p.p.y)
        .collect()
}

/// A pipe junction: a cube hub of half-size `hub` with a bore running out
/// along each of `ends` to a stub ending at `reach`.
///
/// Inside, the bores open into a small cubic chamber rather than meeting
/// each other. Real cylinders meeting at a tee intersect along a saddle
/// curve that has to be cut into both surfaces; the chamber keeps every
/// surface a cylinder or a flat face, so the join is exact by construction.
/// Seen down a pipe it reads as the inside of a cast fitting.
fn pipe_hub(ends: &[Vec3]) -> Mesh {
    let (hub, chamber, reach) = (0.6, 0.4, 1.0);
    let mut b = MeshBuilder::new();
    for axis in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z] {
        let u = if axis.y.abs() > 0.5 { Vec3::X } else { Vec3::Y };
        let w = axis.cross(u);
        let outside = square_samples(axis * hub, u, w, hub);
        let inside = square_samples(axis * chamber, u, w, chamber);
        if ends.contains(&axis) {
            let frame = |at: f32| Frame { origin: axis * at, side: u, up: w, forward: axis };
            b.ring(&outside, &circle_samples(axis * hub, u, w, OUTER), axis);
            b.ring(&inside, &circle_samples(axis * chamber, u, w, INNER), -axis);
            b.sweep(&circle_profile(OUTER, SIDES), &[frame(hub), frame(reach)], false, false);
            b.sweep(&circle_profile(INNER, SIDES), &[frame(chamber), frame(reach)], true, false);
            b.ring(
                &circle_samples(axis * reach, u, w, OUTER),
                &circle_samples(axis * reach, u, w, INNER),
                axis,
            );
        } else {
            // A closed face is a fan over the same boundary samples, not a
            // plain quad, for the same no-T-junction reason.
            b.ring(&outside, &vec![axis * hub; outside.len()], axis);
            b.ring(&inside, &vec![axis * chamber; inside.len()], -axis);
        }
    }
    b.build()
}

/// A tee: a straight run along X with a branch up +Y. The hub's centre is
/// the pivot, not the bounding box's, so every opening sits on an axis
/// through the origin and lines up with a Pipe placed at the same height.
pub fn create_pipe_tee_mesh() -> Mesh {
    pipe_hub(&[Vec3::X, -Vec3::X, Vec3::Y])
}

/// A four-way cross in the XY plane, pivoted on the hub like the tee.
pub fn create_pipe_cross_mesh() -> Mesh {
    pipe_hub(&[Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y])
}

/// A coil spring: five turns of 5 cm wire on a 0.35 m radius, 1 m tall.
pub fn create_spring_mesh() -> Mesh {
    let (radius, wire, turns, height, per_turn) = (0.35, 0.05, 5.0, 0.9, 40);
    let steps = (turns * per_turn as f32) as u32;
    let total = turns * TAU;
    let climb = height / total;
    let mut pts = Vec::new();
    let mut tans = Vec::new();
    for i in 0..=steps {
        let t = total * i as f32 / steps as f32;
        let (s, c) = t.sin_cos();
        pts.push(Vec3::new(radius * c, climb * t, radius * s));
        tans.push(Vec3::new(-radius * s, climb, radius * c));
    }
    let frames = transported_frames(&pts, &tans, Vec3::Y, false);
    let mut b = MeshBuilder::new();
    b.sweep(&circle_profile(wire, 12), &frames, false, true);
    b.center_bounds();
    b.build()
}

/// A (2, 3) torus knot (a trefoil) lying flat, about 1 m across.
///
/// The path is closed, so the tube's frames are transported once round and
/// the leftover twist is spread along the loop; the last ring then lands
/// exactly on the first and the join cannot be found.
pub fn create_torus_knot_mesh() -> Mesh {
    let (scale, tube, steps) = (0.14, 0.07, 240);
    let at = |t: f32| {
        let r = 2.0 + (3.0 * t).cos();
        Vec3::new(r * (2.0 * t).cos(), (3.0 * t).sin(), r * (2.0 * t).sin()) * scale
    };
    let d_at = |t: f32| {
        let r = 2.0 + (3.0 * t).cos();
        let dr = -3.0 * (3.0 * t).sin();
        Vec3::new(
            dr * (2.0 * t).cos() - 2.0 * r * (2.0 * t).sin(),
            3.0 * (3.0 * t).cos(),
            dr * (2.0 * t).sin() + 2.0 * r * (2.0 * t).cos(),
        )
    };
    let ts: Vec<f32> = (0..steps).map(|i| TAU * i as f32 / steps as f32).collect();
    let pts: Vec<Vec3> = ts.iter().map(|&t| at(t)).collect();
    let tans: Vec<Vec3> = ts.iter().map(|&t| d_at(t)).collect();
    let frames = transported_frames(&pts, &tans, Vec3::Y, true);
    let mut b = MeshBuilder::new();
    b.sweep(&circle_profile(tube, 16), &frames, false, false);
    b.center_bounds();
    b.build()
}

/// A Pipe-sized stub with a 1.6 m flange at its foot, for capping a run
/// against a wall or a tank.
pub fn create_pipe_flange_mesh() -> Mesh {
    let (flange_r, flange_t, len) = (0.8, 0.12, 0.6);
    let section = [
        Vec2::new(INNER, 0.0),
        Vec2::new(flange_r, 0.0),
        Vec2::new(flange_r, flange_t),
        Vec2::new(OUTER, flange_t),
        Vec2::new(OUTER, len),
        Vec2::new(INNER, len),
    ];
    let profile: Vec<LathePt> = hard_lathe(&section, true);
    let mut b = MeshBuilder::new();
    b.lathe(&profile, SIDES * 2);
    b.center_bounds();
    b.build()
}

/// An open half-round channel turning 90 degrees on a 1.25 m radius: a
/// gutter or a water slide corner. 1 m across, 5 cm wall.
pub fn create_channel_bend_mesh() -> Mesh {
    let (outer, inner, arc_steps, radius, path_steps) = (0.5, 0.45, 16, 1.25, 24);
    let arc = |r: f32, t: f32, facing: f32| {
        let d = Vec2::new(t.cos(), t.sin());
        ProfilePt::smooth(d.x * r, d.y * r, d * facing)
    };
    let mut profile = Vec::new();
    // Outer skin, left lip round the bottom to the right lip.
    for i in 0..=arc_steps {
        profile.push(arc(outer, PI + PI * i as f32 / arc_steps as f32, 1.0));
    }
    // Each lip is bracketed by hard copies of the skin points either side of
    // it: the zero-length step from a smooth point to its hard twin is a
    // crease, so the lip shades flat and the skins stay smooth right up to
    // their edges. Marking the skin's own end points hard instead would
    // flatten the last facet of each curve too.
    profile.push(ProfilePt::hard(outer, 0.0));
    profile.push(ProfilePt::hard(inner, 0.0));
    // Inner skin back the other way, facing into the channel.
    for i in 0..=arc_steps {
        profile.push(arc(inner, TAU - PI * i as f32 / arc_steps as f32, -1.0));
    }
    profile.push(ProfilePt::hard(-inner, 0.0));
    profile.push(ProfilePt::hard(-outer, 0.0));
    let mut pts = Vec::new();
    let mut tans = Vec::new();
    for i in 0..=path_steps {
        let t = FRAC_PI_2 * i as f32 / path_steps as f32;
        let (s, c) = t.sin_cos();
        pts.push(Vec3::new(radius * c, 0.0, radius * s));
        tans.push(Vec3::new(-s, 0.0, c));
    }
    let frames = transported_frames(&pts, &tans, Vec3::Y, false);
    let mut b = MeshBuilder::new();
    b.sweep(&profile, &frames, false, true);
    b.center_bounds();
    b.build()
}
