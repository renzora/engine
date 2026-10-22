//! Collision response for the `collide/*` nodes.
//!
//! A collider acts after a particle has moved: it sees the step's start and
//! end positions and the velocity, and pushes the particle back to the
//! surface, reflects the velocity into the surface by the bounce, and takes
//! tangential speed off by Coulomb friction. Everything is per particle and
//! stateless, so a replay (`simulate_to`) collides exactly as play did.
//!
//! ## Tunnelling
//!
//! The solid shapes test the whole segment a particle moved along, not just
//! where it ended, so a spark crossing a thin box in one step still hits it.
//! A plane is a half-space, so where the particle ended is already enough; a
//! containing shape likewise only has to look at the end of the step.
//!
//! ## Resting contact
//!
//! A particle lying on the ground hits it every step, because gravity pulls it
//! a hair below the surface each time. Bouncing that hair back would make a
//! resting particle jitter forever, so an impact no faster than what the
//! step's own acceleration added is treated as resting: it loses its normal
//! speed instead of bouncing. Friction is Coulomb's, proportional to the
//! normal speed the hit removed, which is what makes a resting particle slide
//! to a stop at a rate that does not depend on the frame rate.

use bevy::math::{Vec3, Vec4};

use crate::ir::ColliderShape;

/// What a collision did to a particle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    None,
    /// It touched the surface and was moved back onto it.
    Bounced,
    /// It touched the surface of a `kill` collider.
    Killed,
}

/// One collider's inputs for one particle, all in the collider's space.
#[derive(Clone, Copy, Debug)]
pub struct ColliderInput {
    pub shape: ColliderShape,
    pub contain: bool,
    pub kill: bool,
    pub a: Vec4,
    pub b: Vec4,
    pub bounce: f32,
    pub friction: f32,
}

/// Collide one particle that moved from `p0` to `p1` with velocity `v` under
/// the step's acceleration `accel_dt` (acceleration times the step), moving
/// `p1` and `v` as the hit demands.
pub fn collide(c: &ColliderInput, p0: Vec3, p1: &mut Vec3, v: &mut Vec3, accel_dt: Vec3) -> Hit {
    let contact = match c.shape {
        ColliderShape::Plane => plane(c.a.truncate(), c.b.truncate(), *p1),
        ColliderShape::Sphere if c.contain => sphere_inside(c.a.truncate(), c.b.x.abs(), *p1),
        ColliderShape::Sphere => sphere_solid(c.a.truncate(), c.b.x.abs(), p0, *p1),
        ColliderShape::Box if c.contain => return box_inside(c, p1, v, accel_dt),
        ColliderShape::Box => box_solid(c.a.truncate(), c.b.truncate().abs() * 0.5, p0, *p1),
        // Answered after the step, in one batch against the scene
        // (`crate::scene`): nothing a single particle can test here.
        ColliderShape::Scene => return Hit::None,
    };
    let Some((at, normal)) = contact else { return Hit::None };
    if c.kill {
        *p1 = at;
        return Hit::Killed;
    }
    *p1 = at;
    respond(v, normal, c.bounce, c.friction, accel_dt);
    Hit::Bounced
}

/// Reflect the part of `v` going into the surface (normal `n`, pointing to
/// the side the particle belongs on) by `bounce`, and brake the tangential
/// part by `friction` times the normal speed the hit removed.
pub fn respond(v: &mut Vec3, n: Vec3, bounce: f32, friction: f32, accel_dt: Vec3) {
    let vn = v.dot(n);
    if vn >= 0.0 {
        // Already leaving the surface: placing it back on it is all a hit
        // needs to do.
        return;
    }
    let bounce = if bounce.is_finite() { bounce.clamp(0.0, 1.0) } else { 0.0 };
    let friction = if friction.is_finite() { friction.max(0.0) } else { 0.0 };
    let tangent = *v - n * vn;
    // Resting: the impact is no more than this step's own pull into the
    // surface (see the module docs). The margin keeps a resting particle
    // resting when its frame steps vary a little.
    let rest = accel_dt.dot(n).abs() * 1.5;
    let out = if -vn <= rest { 0.0 } else { -vn * bounce };
    let removed = out - vn;
    let t_len = tangent.length();
    let tangent = if t_len > 1e-9 { tangent * ((t_len - friction * removed).max(0.0) / t_len) } else { tangent };
    *v = tangent + n * out;
}

/// A half-space: `point` on the plane, `normal` toward the allowed side.
fn plane(point: Vec3, normal: Vec3, p1: Vec3) -> Option<(Vec3, Vec3)> {
    let n = normal.normalize_or_zero();
    if n == Vec3::ZERO {
        return None;
    }
    let d = (p1 - point).dot(n);
    (d < 0.0).then(|| (p1 - n * d, n))
}

/// A solid ball: the entry point along the segment, or a push straight out
/// for a particle that started inside (born there, or the ball moved).
fn sphere_solid(center: Vec3, r: f32, p0: Vec3, p1: Vec3) -> Option<(Vec3, Vec3)> {
    if r.is_nan() || r <= 0.0 {
        return None;
    }
    let rel0 = p0 - center;
    if rel0.length_squared() < r * r {
        let rel1 = p1 - center;
        if rel1.length_squared() >= r * r {
            return None;
        }
        let n = rel1.try_normalize().unwrap_or(Vec3::Y);
        return Some((center + n * r, n));
    }
    // |rel0 + t d|^2 = r^2, the smaller root, if it lies on the segment.
    let d = p1 - p0;
    let a = d.length_squared();
    if a < 1e-20 {
        return None;
    }
    let b = rel0.dot(d);
    let c = rel0.length_squared() - r * r;
    let disc = b * b - a * c;
    if disc < 0.0 {
        return None;
    }
    let t = (-b - disc.sqrt()) / a;
    if !(0.0..=1.0).contains(&t) {
        return None;
    }
    let at = p0 + d * t;
    let n = (at - center).try_normalize().unwrap_or(Vec3::Y);
    Some((center + n * r, n))
}

/// A bowl: a particle past the wall is put back on it, the normal pointing
/// in.
fn sphere_inside(center: Vec3, r: f32, p1: Vec3) -> Option<(Vec3, Vec3)> {
    let rel = p1 - center;
    let len = rel.length();
    if r.is_nan() || r <= 0.0 || len <= r {
        return None;
    }
    let out = rel / len;
    Some((center + out * r, -out))
}

/// A solid box (centre, half extents): the entry face along the segment, or
/// for a particle that started inside, out through the nearest face.
fn box_solid(center: Vec3, half: Vec3, p0: Vec3, p1: Vec3) -> Option<(Vec3, Vec3)> {
    if half.min_element() <= 0.0 || !half.is_finite() {
        return None;
    }
    let (min, max) = (center - half, center + half);
    let inside = |p: Vec3| p.cmpgt(min).all() && p.cmplt(max).all();
    if inside(p0) {
        if !inside(p1) {
            return None;
        }
        // Least penetration: the face the particle is nearest to.
        let to_min = p1 - min;
        let to_max = max - p1;
        let mut best = (f32::INFINITY, Vec3::ZERO, p1);
        for axis in 0..3 {
            let unit = Vec3::AXES[axis];
            if to_min[axis] < best.0 {
                let mut at = p1;
                at[axis] = min[axis];
                best = (to_min[axis], -unit, at);
            }
            if to_max[axis] < best.0 {
                let mut at = p1;
                at[axis] = max[axis];
                best = (to_max[axis], unit, at);
            }
        }
        return Some((best.2, best.1));
    }
    // Slab test along the segment; the entry face is the last slab entered.
    let d = p1 - p0;
    let (mut t0, mut t1) = (0.0_f32, 1.0_f32);
    let mut normal = Vec3::ZERO;
    for axis in 0..3 {
        if d[axis].abs() < 1e-12 {
            if p0[axis] < min[axis] || p0[axis] > max[axis] {
                return None;
            }
            continue;
        }
        let inv = 1.0 / d[axis];
        let (mut ta, mut tb) = ((min[axis] - p0[axis]) * inv, (max[axis] - p0[axis]) * inv);
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        if ta > t0 {
            t0 = ta;
            // Moving toward +axis enters through the min face, whose outward
            // normal is -axis, and the other way round.
            normal = Vec3::AXES[axis] * -d[axis].signum();
        }
        t1 = t1.min(tb);
        if t0 > t1 {
            return None;
        }
    }
    (normal != Vec3::ZERO).then(|| (p0 + d * t0, normal))
}

/// A room: each axis the particle left through is clamped back and
/// responded to on its own, so a particle in a corner bounces off both walls.
fn box_inside(c: &ColliderInput, p1: &mut Vec3, v: &mut Vec3, accel_dt: Vec3) -> Hit {
    let center = c.a.truncate();
    let half = c.b.truncate().abs() * 0.5;
    if !half.is_finite() {
        return Hit::None;
    }
    let (min, max) = (center - half, center + half);
    let mut hit = false;
    for axis in 0..3 {
        let unit = Vec3::AXES[axis];
        let n = if p1[axis] > max[axis] {
            p1[axis] = max[axis];
            -unit
        } else if p1[axis] < min[axis] {
            p1[axis] = min[axis];
            unit
        } else {
            continue;
        };
        hit = true;
        if !c.kill {
            respond(v, n, c.bounce, c.friction, accel_dt);
        }
    }
    match (hit, c.kill) {
        (false, _) => Hit::None,
        (true, true) => Hit::Killed,
        (true, false) => Hit::Bounced,
    }
}
