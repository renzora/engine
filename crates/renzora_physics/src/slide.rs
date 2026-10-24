//! Collide-and-slide, written once against shape casts so every backend gets
//! `move_controller` without implementing a character controller.
//!
//! [`collide_and_slide`] is the pure algorithm: it takes the sweep as a
//! closure, which is how parkour drives it from its own spatial queries and how
//! [`run_kinematic_slides`] drives it through the selected backend.

use bevy::prelude::*;
use renzora::physics::{Cast, CastBatch, CastShape, PhysicsBackends, PhysicsDim, ResolvedCollider};

use crate::data::RuntimePhysics2d;
use crate::read_state::PhysicsReadState;

/// Result of a [`collide_and_slide`] call.
#[derive(Clone, Copy, Debug, Default)]
pub struct SlideResult {
    /// The movement that was actually applied after collide-and-slide.
    pub actual_delta: Vec3,
    /// True if a downward probe found ground at the end of the slide.
    pub grounded: bool,
    /// True if the slide hit a wall (non-walkable surface) during iteration.
    pub hit_wall: bool,
    /// Normal of the last ground hit (or `Vec3::Y` if airborne).
    pub ground_normal: Vec3,
}

/// What one sweep reports: how far the shape travelled before touching, and
/// the hit surface's normal.
#[derive(Clone, Copy, Debug)]
pub struct SweepHit {
    pub distance: f32,
    pub normal: Vec3,
}

/// Collide-and-slide: move a shape by `desired_delta`, clipping against hits
/// and retrying along the remaining tangent up to a fixed iteration budget.
/// Returns the effective delta and a grounded flag.
///
/// `sweep(origin, direction, max_distance)` casts the moving shape. It must
/// ignore the shape's own body and anything it already overlaps at `origin`,
/// or a body resting on the floor could never move along it.
///
/// The caller is responsible for applying `result.actual_delta` to the
/// `Transform` (this function only does the query math).
pub fn collide_and_slide(
    mut sweep: impl FnMut(Vec3, Dir3, f32) -> Option<SweepHit>,
    origin: Vec3,
    desired_delta: Vec3,
    max_slope_deg: f32,
) -> SlideResult {
    const MAX_ITERS: usize = 4;
    const SKIN: f32 = 0.002;

    let mut remaining = desired_delta;
    let mut origin = origin;
    let mut actual = Vec3::ZERO;
    let mut hit_wall = false;
    let max_slope_rad = max_slope_deg.to_radians();

    for _ in 0..MAX_ITERS {
        let dist = remaining.length();
        if dist < 1e-5 {
            break;
        }
        let dir = remaining / dist;
        let Ok(dir3) = Dir3::new(dir) else { break };

        match sweep(origin, dir3, dist + SKIN) {
            None => {
                actual += remaining;
                origin += remaining;
                break;
            }
            Some(h) => {
                let travel = (h.distance - SKIN).max(0.0);
                let moved = dir * travel;
                actual += moved;
                origin += moved;

                let angle = h.normal.angle_between(Vec3::Y);
                if angle > max_slope_rad {
                    hit_wall = true;
                }

                let consumed = travel;
                let leftover_mag = (dist - consumed).max(0.0);
                let tangent = remaining - h.normal * remaining.dot(h.normal);
                if tangent.length_squared() < 1e-8 {
                    break;
                }
                remaining = tangent.normalize() * leftover_mag;
            }
        }
    }

    // Downward probe to decide grounded at the final position.
    let mut grounded = false;
    let mut ground_normal = Vec3::Y;
    if let Some(h) = sweep(origin, Dir3::NEG_Y, 0.15) {
        let angle = h.normal.angle_between(Vec3::Y);
        if angle <= max_slope_rad {
            grounded = true;
            ground_normal = h.normal;
        }
    }

    SlideResult {
        actual_delta: actual,
        grounded,
        hit_wall,
        ground_normal,
    }
}

/// One pending kinematic slide request.
#[derive(Clone, Copy, Debug)]
pub struct PendingSlide {
    pub entity: Entity,
    pub delta: Vec3,
    pub max_slope: f32,
}

/// Queue of slide requests produced by the `kinematic_slide` script action
/// and drained each frame by [`run_kinematic_slides`].
#[derive(Resource, Default)]
pub struct PendingKinematicSlides(pub Vec<PendingSlide>);

/// Applies pending kinematic slides with full collision response, sweeping
/// each body's own collider through the backend simulating the 3D world.
///
/// Exclusive because a backend's cast reads its state out of the world. A 2D
/// body, or one whose collider is not built yet (a mesh still loading), is
/// skipped and does not move, rather than moving through everything.
pub fn run_kinematic_slides(world: &mut World) {
    let slides = match world.get_resource_mut::<PendingKinematicSlides>() {
        Some(mut queue) if !queue.0.is_empty() => std::mem::take(&mut queue.0),
        _ => return,
    };
    let mut batch = CastBatch::default();
    for slide in slides {
        let Ok(entity) = world.get_entity(slide.entity) else {
            continue;
        };
        if entity.contains::<RuntimePhysics2d>() || !entity.contains::<ResolvedCollider>() {
            continue;
        }
        let Some(transform) = entity.get::<Transform>().copied() else {
            continue;
        };
        // A `Collider` cast is placed by the entity's own pose; the backend
        // adds the collider's offset and turn, exactly as it built them.
        let layers = entity.get::<ResolvedCollider>().map(|c| c.layers.filters).unwrap_or(u32::MAX);

        let result = collide_and_slide(
            |origin, direction, max_distance| {
                batch.casts.clear();
                batch.casts.push(
                    Cast::shape(
                        PhysicsDim::D3,
                        CastShape::Collider(slide.entity),
                        origin,
                        transform.rotation,
                        *direction,
                        max_distance,
                    )
                    .excluding([slide.entity])
                    .with_sensors(true)
                    .ignoring_origin_penetration(true)
                    .colliding_with(layers),
                );
                PhysicsBackends::cast(world, &mut batch);
                batch.hits[0].map(|h| SweepHit { distance: h.distance, normal: h.normal })
            },
            transform.translation,
            slide.delta,
            slide.max_slope,
        );

        let Ok(mut entity) = world.get_entity_mut(slide.entity) else {
            continue;
        };
        if let Some(mut t) = entity.get_mut::<Transform>() {
            t.translation = transform.translation + result.actual_delta;
        }
        if let Some(mut rs) = entity.get_mut::<PhysicsReadState>() {
            rs.grounded = result.grounded;
            rs.ground_normal = result.ground_normal;
        }
    }
}
