//! Answers the contract crate's scene queries (`renzora::scene_query`) with
//! Avian's spatial query, so crates that must not link a physics engine (the
//! particle system) can still hit the level's real colliders.
//!
//! One cached system per world dimension runs a whole batch: building a
//! `SpatialQuery` costs a few query states, and doing that once per frame
//! rather than once per ray is the point of the batch. A ray for the other
//! dimension, or for a dimension this build has no physics for, is answered
//! `None`, which the contract allows.
//!
//! Sensors are skipped: they are trigger volumes (a checkpoint, a kill
//! plane), not surfaces, and a spark bouncing off an invisible trigger would
//! look like a bug in the effect.

use bevy::ecs::system::InMut;
use bevy::prelude::*;
use renzora::scene_query::{SceneHit, SceneQuery, SceneQueryBatch};

/// The service `PhysicsPlugin` installs.
pub struct AvianSceneQuery;

impl SceneQuery for AvianSceneQuery {
    fn cast(&self, world: &mut World, batch: &mut SceneQueryBatch) {
        batch.hits.clear();
        batch.hits.resize(batch.rays.len(), None);
        // A failure here is a system that could not run (its resources are
        // missing because the physics plugins were not added); the batch then
        // stays all misses, which is the same as an empty scene.
        #[cfg(feature = "avian3d")]
        if batch.rays.iter().any(|r| !r.planar) {
            let _ = world.run_system_cached_with(cast_3d, &mut *batch);
        }
        #[cfg(feature = "avian2d")]
        if batch.rays.iter().any(|r| r.planar) {
            let _ = world.run_system_cached_with(cast_2d, &mut *batch);
        }
    }
}

/// Hits at the cast origin with no usable normal: the ray started inside a
/// solid. There is no "out" to push toward from one ray, so the cast is
/// treated as a miss and the particle carries on rather than sticking.
fn usable(distance: f32, normal: Vec3) -> bool {
    distance.is_finite() && normal.is_finite() && normal.length_squared() > 1e-6
}

/// A cast that goes anywhere: a positive length, and not NaN.
fn reaches(max_distance: f32) -> bool {
    !max_distance.is_nan() && max_distance > 0.0
}

#[cfg(feature = "avian3d")]
fn cast_3d(
    InMut(batch): InMut<SceneQueryBatch>,
    spatial: avian3d::prelude::SpatialQuery,
    sensors: Query<(), With<avian3d::prelude::Sensor>>,
) {
    use avian3d::prelude::{Collider, ShapeCastConfig, SpatialQueryFilter};
    let filter = SpatialQueryFilter::default();
    let solid_only = |e: Entity| !sensors.contains(e);
    // Spheres are shared between rays of one radius, which a burst of one
    // emitter's particles nearly always is.
    let mut sphere: Option<(f32, Collider)> = None;
    for (ray, hit) in batch.rays.iter().zip(batch.hits.iter_mut()) {
        if ray.planar || !reaches(ray.max_distance) {
            continue;
        }
        let Ok(dir) = Dir3::new(ray.direction) else { continue };
        if ray.radius > 0.0 {
            if sphere.as_ref().is_none_or(|(r, _)| *r != ray.radius) {
                sphere = Some((ray.radius, Collider::sphere(ray.radius)));
            }
            let Some((_, shape)) = &sphere else { continue };
            let config = ShapeCastConfig::from_max_distance(ray.max_distance);
            if let Some(h) = spatial.cast_shape_predicate(shape, ray.origin, Quat::IDENTITY, dir, &config, &filter, &solid_only) {
                let normal = h.normal1.normalize_or_zero();
                if usable(h.distance, normal) {
                    *hit = Some(SceneHit { distance: h.distance, point: h.point1, normal, entity: h.entity });
                }
            }
        } else if let Some(h) = spatial.cast_ray_predicate(ray.origin, dir, ray.max_distance, true, &filter, &solid_only) {
            let normal = h.normal.normalize_or_zero();
            if usable(h.distance, normal) {
                *hit = Some(SceneHit { distance: h.distance, point: ray.origin + ray.direction * h.distance, normal, entity: h.entity });
            }
        }
    }
}

#[cfg(feature = "avian2d")]
fn cast_2d(
    InMut(batch): InMut<SceneQueryBatch>,
    spatial: avian2d::prelude::SpatialQuery,
    sensors: Query<(), With<avian2d::prelude::Sensor>>,
) {
    use avian2d::prelude::{Collider, ShapeCastConfig, SpatialQueryFilter};
    let filter = SpatialQueryFilter::default();
    let solid_only = |e: Entity| !sensors.contains(e);
    let mut circle: Option<(f32, Collider)> = None;
    for (ray, hit) in batch.rays.iter().zip(batch.hits.iter_mut()) {
        if !ray.planar || !reaches(ray.max_distance) {
            continue;
        }
        // The plane's own motion only: depth in 2D is draw order.
        let flat = ray.direction.truncate();
        let Ok(dir) = Dir2::new(flat) else { continue };
        // A direction with depth covers less ground in the plane than its
        // length says.
        let reach = ray.max_distance * flat.length();
        let origin = ray.origin.truncate();
        if ray.radius > 0.0 {
            if circle.as_ref().is_none_or(|(r, _)| *r != ray.radius) {
                circle = Some((ray.radius, Collider::circle(ray.radius)));
            }
            let Some((_, shape)) = &circle else { continue };
            let config = ShapeCastConfig::from_max_distance(reach);
            if let Some(h) = spatial.cast_shape_predicate(shape, origin, Rot2::IDENTITY, dir, &config, &filter, &solid_only) {
                let normal = h.normal1.normalize_or_zero().extend(0.0);
                if usable(h.distance, normal) {
                    let distance = h.distance / flat.length();
                    *hit = Some(SceneHit { distance, point: h.point1.extend(ray.origin.z), normal, entity: h.entity });
                }
            }
        } else if let Some(h) = spatial.cast_ray_predicate(origin, dir, reach, true, &filter, &solid_only) {
            let normal = h.normal.normalize_or_zero().extend(0.0);
            if usable(h.distance, normal) {
                // Back to a distance along the original direction, so the
                // consumer's `origin + direction * distance` lands on the hit.
                let distance = h.distance / flat.length();
                *hit = Some(SceneHit { distance, point: ray.origin + ray.direction * distance, normal, entity: h.entity });
            }
        }
    }
}
