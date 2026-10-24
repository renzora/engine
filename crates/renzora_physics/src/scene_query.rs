//! Answers the contract crate's scene queries (`renzora::scene_query`) through
//! whichever backend simulates each world, so crates that must not link a
//! physics engine (the particle system) can still hit the level's real
//! colliders.
//!
//! A scene query is a [`Cast`] with the particle system's rules applied on top,
//! and those rules live here rather than in each backend:
//!
//! - **Sensors are skipped.** They are trigger volumes (a checkpoint, a kill
//!   plane), not surfaces, and a spark bouncing off an invisible trigger would
//!   look like a bug in the effect.
//! - **A planar ray moves in the plane only.** Depth in 2D is draw order, so a
//!   ray's `z` is dropped and its reach shrinks with it; the hit distance is
//!   converted back to one along the original direction.
//! - **A hit with no usable normal is a miss.** A ray that starts inside a solid
//!   has no "out" to push toward, so the particle carries on rather than
//!   sticking.

use bevy::prelude::*;
use renzora::physics::{Cast, CastBatch, CastShape, PhysicsBackends, PhysicsDim, ResolvedShape};
use renzora::scene_query::{SceneHit, SceneQuery, SceneQueryBatch};

/// The service `PhysicsPlugin` installs.
pub struct BackendSceneQuery;

impl SceneQuery for BackendSceneQuery {
    fn cast(&self, world: &mut World, batch: &mut SceneQueryBatch) {
        batch.hits.clear();
        batch.hits.resize(batch.rays.len(), None);

        // Rays that go nowhere are not asked at all; `index` maps each cast back.
        let mut casts = CastBatch::default();
        let mut index = Vec::with_capacity(batch.rays.len());
        for (i, ray) in batch.rays.iter().enumerate() {
            if !reaches(ray.max_distance) {
                continue;
            }
            let (dim, direction, reach) = if ray.planar {
                let flat = ray.direction.truncate();
                let Ok(dir) = Dir2::new(flat) else { continue };
                // A direction with depth covers less ground in the plane than
                // its length says.
                (PhysicsDim::D2, dir.extend(0.0), ray.max_distance * flat.length())
            } else {
                let Ok(dir) = Dir3::new(ray.direction) else { continue };
                (PhysicsDim::D3, *dir, ray.max_distance)
            };
            let shape = if ray.radius > 0.0 {
                CastShape::Shape(match dim {
                    PhysicsDim::D3 => ResolvedShape::Sphere { radius: ray.radius },
                    PhysicsDim::D2 => ResolvedShape::Circle { radius: ray.radius },
                })
            } else {
                CastShape::Ray
            };
            casts.casts.push(Cast::shape(dim, shape, ray.origin, Quat::IDENTITY, direction, reach));
            index.push(i);
        }
        if casts.casts.is_empty() {
            return;
        }

        PhysicsBackends::cast(&*world, &mut casts);

        for (hit, &i) in casts.hits.iter().zip(&index) {
            let Some(h) = hit else { continue };
            let ray = &batch.rays[i];
            let normal = h.normal.normalize_or_zero();
            if !usable(h.distance, normal) {
                continue;
            }
            let is_shape = ray.radius > 0.0;
            batch.hits[i] = Some(if ray.planar {
                // Back to a distance along the original direction, so the
                // consumer's `origin + direction * distance` lands on the hit.
                let distance = h.distance / ray.direction.truncate().length();
                let point = if is_shape {
                    h.point.truncate().extend(ray.origin.z)
                } else {
                    ray.origin + ray.direction * distance
                };
                SceneHit { distance, point, normal: normal.truncate().extend(0.0), entity: h.entity }
            } else {
                let point = if is_shape { h.point } else { ray.origin + ray.direction * h.distance };
                SceneHit { distance: h.distance, point, normal, entity: h.entity }
            });
        }
    }
}

/// A hit at the cast origin with no usable normal: the ray started inside a
/// solid.
fn usable(distance: f32, normal: Vec3) -> bool {
    distance.is_finite() && normal.is_finite() && normal.length_squared() > 1e-6
}

/// A cast that goes anywhere: a positive length, and not NaN.
fn reaches(max_distance: f32) -> bool {
    !max_distance.is_nan() && max_distance > 0.0
}
