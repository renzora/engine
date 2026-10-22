//! Ray and sphere casts against the scene's colliders, without a physics
//! engine in sight.
//!
//! The particle system wants its sparks to bounce off the real level, not just
//! off planes and boxes typed into a graph. The level's shape lives in the
//! physics engine's collider tree, and linking Avian into the particle crate
//! would drag a physics engine into every build that draws a spark, give a 2D
//! game a 3D solver it never asked for, and couple two crates that otherwise
//! know nothing of each other. So the question ("where does this segment first
//! hit something solid?") is asked through this contract, and whichever crate
//! owns the colliders answers it: `renzora_physics` installs a
//! [`SceneQueries`] resource backed by Avian's spatial query when it is in the
//! build. A build without physics has no resource, and a consumer treats that
//! as an empty scene.
//!
//! ## Batched, and asked with the world in hand
//!
//! A query needs the collider tree and every collider's pose, which a system
//! parameter reads; there is no snapshot to hand across. So the service takes
//! `&mut World` and a whole [`SceneQueryBatch`] at once: one call per frame per
//! consumer, however many rays, which is what lets the physics side run them
//! through one cached system instead of building its parameters per ray. The
//! consumer runs it from an exclusive system, after it has collected every
//! ray and before it applies a single hit.
//!
//! ## Units and spaces
//!
//! Everything is in world space and world units. A 2D query ([`SceneRay::planar`])
//! is the same ray with its `z` ignored, answered by the 2D physics world
//! (sprites, tiles), so a 2D effect collides with the 2D level and a 3D one
//! with the 3D level even when both exist in one app.

use std::sync::Arc;

use bevy::prelude::*;

/// One cast: from `origin` along `direction` (unit length) for up to
/// `max_distance`. A `radius` above zero sweeps a sphere of that radius
/// instead of a ray, so a fat particle stops with its surface on the wall
/// rather than its centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneRay {
    pub origin: Vec3,
    pub direction: Vec3,
    pub max_distance: f32,
    pub radius: f32,
    /// Ask the 2D physics world, in the XY plane, instead of the 3D one.
    pub planar: bool,
}

/// Where a cast first hit a collider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneHit {
    /// Along the ray, from its origin. For a sphere sweep, how far the
    /// sphere's centre travelled before it touched.
    pub distance: f32,
    /// The contact point on the collider's surface.
    pub point: Vec3,
    /// The surface normal at the contact, unit length, pointing back toward
    /// where the cast came from.
    pub normal: Vec3,
    /// The collider that was hit.
    pub entity: Entity,
}

/// One frame's casts and their answers, index for index. Reused between
/// frames by the consumer, so a steady state allocates nothing.
#[derive(Clone, Debug, Default)]
pub struct SceneQueryBatch {
    pub rays: Vec<SceneRay>,
    /// Filled by [`SceneQuery::cast`]: exactly one entry per ray.
    pub hits: Vec<Option<SceneHit>>,
}

impl SceneQueryBatch {
    pub fn clear(&mut self) {
        self.rays.clear();
        self.hits.clear();
    }
}

/// What a crate that owns colliders implements.
///
/// `cast` must leave `batch.hits` exactly as long as `batch.rays`, with `None`
/// for every ray it could not answer (a planar ray in a build with no 2D
/// physics, say). Sensors (trigger volumes) are not solid and must not be
/// reported: a spark does not bounce off a checkpoint.
pub trait SceneQuery: Send + Sync + 'static {
    fn cast(&self, world: &mut World, batch: &mut SceneQueryBatch);
}

/// The installed scene query service. Present only when some crate in the
/// build answers scene queries; see the module docs.
#[derive(Resource, Clone)]
pub struct SceneQueries(Arc<dyn SceneQuery>);

impl SceneQueries {
    pub fn new(query: impl SceneQuery) -> Self {
        Self(Arc::new(query))
    }

    /// Run every ray in `batch`, filling `batch.hits`. The result is padded
    /// or cut to one entry per ray whatever the service did, so a consumer
    /// can index it without checking.
    pub fn cast(&self, world: &mut World, batch: &mut SceneQueryBatch) {
        batch.hits.clear();
        if batch.rays.is_empty() {
            return;
        }
        self.0.cast(world, batch);
        batch.hits.resize(batch.rays.len(), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A floor at y = 0, answering every downward ray that crosses it.
    struct Floor;

    impl SceneQuery for Floor {
        fn cast(&self, _world: &mut World, batch: &mut SceneQueryBatch) {
            // Deliberately one short, to prove the wrapper pads.
            for ray in batch.rays.iter().take(batch.rays.len() - 1) {
                let hit = (ray.direction.y < 0.0 && ray.origin.y >= 0.0).then(|| {
                    let distance = ray.origin.y / -ray.direction.y;
                    SceneHit { distance, point: ray.origin + ray.direction * distance, normal: Vec3::Y, entity: Entity::PLACEHOLDER }
                });
                batch.hits.push(hit.filter(|h| h.distance <= ray.max_distance));
            }
        }
    }

    #[test]
    fn the_service_answers_one_entry_per_ray() {
        let mut world = World::new();
        let service = SceneQueries::new(Floor);
        let down = SceneRay { origin: Vec3::Y, direction: Vec3::NEG_Y, max_distance: 5.0, radius: 0.0, planar: false };
        let mut batch = SceneQueryBatch { rays: vec![down, SceneRay { max_distance: 0.5, ..down }, down], hits: Vec::new() };
        service.cast(&mut world, &mut batch);
        assert_eq!(batch.hits.len(), 3);
        assert_eq!(batch.hits[0].map(|h| h.point), Some(Vec3::ZERO));
        assert!(batch.hits[1].is_none(), "out of reach");
        assert!(batch.hits[2].is_none(), "unanswered, so padded");
        batch.clear();
        service.cast(&mut world, &mut batch);
        assert!(batch.hits.is_empty());
    }
}
