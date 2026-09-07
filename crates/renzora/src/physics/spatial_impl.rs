//! The backend side of [`spatial`](crate::physics::spatial).
//!
//! Four functions, installed into [`SpatialQueries`] at startup, that turn a
//! backend-agnostic cast request into an avian one. Everything avian-shaped
//! stops here: the caller passes a [`SweepShape`] and gets a [`SpatialHit`],
//! and never links this crate.
//!
//! # Why the `SystemState` is cached in a resource
//!
//! Each entry point takes `&mut World` and has to rebuild avian's
//! `SpatialQuery` from it. Constructing a fresh `SystemState` per call would
//! re-resolve the component ids and re-scan archetypes every time, and a
//! character controller casts ten times per character per frame. Holding one in
//! [`SpatialQueryState`] means a call costs an archetype-delta update and two
//! query fetches.
//!
//! # Why the collider is cached too
//!
//! `Collider::capsule` allocates a parry shape. The overwhelmingly common
//! pattern is one controller sweeping the same capsule for every cast in a
//! frame, so a one-entry cache keyed on the [`SweepShape`] turns that back into
//! a comparison. It is a cache and not a map on purpose: two controllers with
//! different capsules would thrash a one-entry cache, but they would each
//! rebuild a shape they were rebuilding anyway, so the miss costs nothing that
//! was not already being paid.

use avian3d::prelude::*;
use bevy::ecs::system::SystemState;
use bevy::math::bounding::Aabb3d;
use bevy::prelude::*;

use crate::physics::spatial::{
    RayCast, ShapeCast, Slide, SlideOutcome, SpatialFilter, SpatialHit, SpatialQueries, SweepShape,
};

use crate::physics::backend::avian_character::shape_cast_slide;

/// Cached machinery for the query entry points below.
///
/// A resource rather than a `Local` because the entry points are plain
/// functions, not systems: there is nowhere for a `Local` to live.
#[derive(Resource)]
struct SpatialQueryState {
    query: SystemState<SpatialQuery<'static, 'static>>,
    /// The last shape built, kept to avoid re-allocating a parry shape for
    /// every cast in a frame. See the module doc.
    shape: Option<(SweepShape, Collider)>,
}

impl SpatialQueryState {
    fn collider(&mut self, shape: SweepShape) -> Collider {
        if let Some((cached, collider)) = &self.shape {
            if *cached == shape {
                return collider.clone();
            }
        }
        let collider = match shape {
            SweepShape::Sphere { radius } => Collider::sphere(radius),
            SweepShape::Capsule { radius, length } => Collider::capsule(radius, length),
            SweepShape::Cuboid { half_extents } => Collider::cuboid(
                half_extents.x * 2.0,
                half_extents.y * 2.0,
                half_extents.z * 2.0,
            ),
        };
        self.shape = Some((shape, collider.clone()));
        collider
    }
}

/// Translate the contract's filter into avian's.
fn filter_of(filter: &SpatialFilter) -> SpatialQueryFilter {
    SpatialQueryFilter::from_excluded_entities(filter.excluded.iter().copied())
}

fn cast_ray(world: &mut World, cast: &RayCast, filter: &SpatialFilter) -> Option<SpatialHit> {
    world.resource_scope(|world, mut state: Mut<SpatialQueryState>| {
        // `get` validates the params, and the one that can fail here is avian's
        // `ColliderTrees`: it does not exist until the backend has finished its
        // own setup. A cast on that frame finds nothing, which is the truth.
        let Ok(spatial) = state.query.get(world) else {
            return None;
        };
        let hit = spatial.cast_ray(
            cast.origin,
            cast.direction,
            cast.max_distance,
            cast.solid,
            &filter_of(filter),
        )?;
        Some(SpatialHit {
            entity: hit.entity,
            distance: hit.distance,
            normal: hit.normal,
            // avian reports a distance along the ray rather than a point, and
            // every caller wants the point. Computing it here means one
            // definition of "where did it hit" instead of one per call site.
            point: cast.origin + cast.direction.as_vec3() * hit.distance,
        })
    })
}

fn cast_shape(world: &mut World, cast: &ShapeCast, filter: &SpatialFilter) -> Option<SpatialHit> {
    world.resource_scope(|world, mut state: Mut<SpatialQueryState>| {
        let collider = state.collider(cast.shape);
        let Ok(spatial) = state.query.get(world) else {
            return None;
        };
        let hit = spatial.cast_shape(
            &collider,
            cast.origin,
            cast.rotation,
            cast.direction,
            &ShapeCastConfig {
                max_distance: cast.max_distance,
                ignore_origin_penetration: cast.ignore_origin_penetration,
                ..Default::default()
            },
            &filter_of(filter),
        )?;
        Some(SpatialHit {
            entity: hit.entity,
            distance: hit.distance,
            // `normal1` is the normal on the *cast* shape's surface, which is
            // the one pointing back at the caster. `normal2` is the hit
            // collider's own and points the other way; mixing them up inverts
            // every slope test that uses this.
            normal: hit.normal1,
            point: hit.point1,
        })
    })
}

fn slide(world: &mut World, slide: &Slide, filter: &SpatialFilter) -> SlideOutcome {
    world.resource_scope(|world, mut state: Mut<SpatialQueryState>| {
        let collider = state.collider(slide.shape);
        let Ok(spatial) = state.query.get(world) else {
            // Stay put rather than apply the whole delta: an unvalidated frame
            // is one where nothing can be collided against, and moving the full
            // distance would walk a character through the first wall of the
            // level on the frame the backend came up.
            return SlideOutcome {
                ground_normal: Vec3::Y,
                ..Default::default()
            };
        };
        let result = shape_cast_slide(
            &spatial,
            &collider,
            slide.origin,
            slide.rotation,
            slide.delta,
            slide.max_slope_deg,
            &filter_of(filter),
        );
        SlideOutcome {
            actual_delta: result.actual_delta,
            grounded: result.grounded,
            hit_wall: result.hit_wall,
            ground_normal: result.ground_normal,
        }
    })
}

fn collider_aabb(world: &mut World, entity: Entity) -> Option<Aabb3d> {
    // A direct component read rather than a `SystemState`: there is one
    // component to fetch and no filtering to do.
    let aabb = world.get::<ColliderAabb>(entity)?;
    // `ColliderAabb::INVALID` is the empty box avian gives a collider whose
    // bounds have not been computed yet, with min/max at opposite infinities.
    // Returning it would hand the caller a box containing everything.
    if aabb.min.cmpgt(aabb.max).any() {
        return None;
    }
    Some(Aabb3d {
        min: aabb.min.into(),
        max: aabb.max.into(),
    })
}

/// Install the query table.
///
/// Called only from the 3D backend's setup, which is what makes a 2D-only build
/// leave the resource absent. `SpatialQueries::get` reports that as `None` and
/// a caller does nothing, rather than casting into a simulation that has no
/// third axis.
pub fn install(app: &mut App) {
    // Built before the insert: `SystemState::new` needs the world, and
    // `insert_resource` would already have borrowed the app to get it.
    let state = SpatialQueryState {
        query: SystemState::new(app.world_mut()),
        shape: None,
    };
    app.insert_resource(state);
    app.insert_resource(SpatialQueries {
        cast_ray,
        cast_shape,
        slide,
        collider_aabb,
    });
}
