//! Asking the collider tree a question, without linking the crate that owns it.
//!
//! Every other seam in this crate is data or an id. A plugin fills in a
//! [`CollisionShapeData`](super::CollisionShapeData) and the backend reads it;
//! it pushes a `CharacterCommand` and the controller acts on it; it registers a
//! `ShellActionItem` and waits to hear its own id back. Nothing has to be
//! *computed* for the plugin, so nothing has to call back into the engine.
//!
//! A cast is not like that. It is a question whose answer comes out of the
//! collider tree only `renzora_physics` can see, and it has to be answered
//! now, inside the caller's own logic, because the next cast usually depends on
//! what this one said. So this module is the one place in the contract crate
//! that passes function pointers instead of values: [`SpatialQueries`] is a
//! table the backend installs at startup, and a caller invokes it without ever
//! naming an avian type.
//!
//! # Why every entry point takes `&mut World`
//!
//! avian's `SpatialQuery` is two queries plus a resource, so it can only be
//! built from a world. There is no handle to hand out that does not borrow one,
//! and publishing a per-frame copy of the acceleration structure instead would
//! mean cloning a BVH every frame to save a borrow.
//!
//! The cost lands on the caller, and it is real: a system that casts has to be
//! exclusive, or hold its own `SystemState` and drop that borrow before it
//! calls. In exchange the answer is exact and same-frame. The alternative that
//! keeps callers ordinary — queue the casts, let a physics system resolve them,
//! read results next frame — puts a frame of lag in precisely the code that
//! moves a character, and a collide-and-slide that resolves against where the
//! world was last frame walks through corners.
//!
//! # Absent is a valid state
//!
//! [`SpatialQueries`] is installed only by a 3D backend. A 2D-only build, or a
//! lean export with physics stripped, has no such resource, and a caller should
//! do nothing rather than panic. [`SpatialQueries::get`] returns `Option` for
//! that reason, and is also how a caller copies the table out before it starts
//! handing `&mut World` around: holding a `Res` borrow across a call would
//! forbid the very access the call needs.

use bevy::math::bounding::Aabb3d;
use bevy::prelude::*;

/// What a cast found.
#[derive(Clone, Copy, Debug)]
pub struct SpatialHit {
    /// The **collider** entity that was hit.
    ///
    /// Usually a child of whatever the caller thinks of as "the thing that was
    /// hit": an imported model carries its collider on a child mesh rather than
    /// on the entity holding the gameplay components. A caller that cares about
    /// the owner walks `ChildOf` up from here.
    pub entity: Entity,
    /// Distance from the cast origin, along the cast direction.
    pub distance: f32,
    /// Surface normal at the hit, pointing back towards the origin.
    pub normal: Vec3,
    /// World-space point of contact.
    pub point: Vec3,
}

/// The shape a sweep is swept with.
///
/// A small closed set rather than a handle to an authored collider, because a
/// sweep shape is a query detail and not something the scene stores. A
/// character controller sweeps the capsule it derives from its own radius and
/// height, which is deliberately *not* whatever collider the artist put on the
/// model. Adding a variant here is additive and breaks nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SweepShape {
    Sphere {
        radius: f32,
    },
    /// A capsule standing on Y. `length` is the cylinder segment between the
    /// two cap centres, so the total height is `length + 2.0 * radius` — the
    /// same argument order the backend's own capsule constructor takes.
    Capsule {
        radius: f32,
        length: f32,
    },
    Cuboid {
        half_extents: Vec3,
    },
}

/// Which colliders a cast is allowed to see.
///
/// A struct rather than a bare slice so layers and sensor handling can be added
/// without changing four signatures.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpatialFilter<'a> {
    /// Entities the cast passes straight through.
    ///
    /// A character must exclude its **whole subtree**, not just itself:
    /// a model's collider hangs off a child mesh, and a capsule that collides
    /// with its own body cannot move at all. That failure is silent, which is
    /// worth knowing before debugging it.
    pub excluded: &'a [Entity],
}

/// A ray cast.
#[derive(Clone, Copy, Debug)]
pub struct RayCast {
    pub origin: Vec3,
    pub direction: Dir3,
    pub max_distance: f32,
    /// When true, a ray starting inside a collider reports the origin itself
    /// rather than punching out through the far side and reporting the geometry
    /// behind it. Probes almost always want this.
    pub solid: bool,
}

/// A shape sweep.
#[derive(Clone, Copy, Debug)]
pub struct ShapeCast {
    pub shape: SweepShape,
    pub origin: Vec3,
    pub rotation: Quat,
    pub direction: Dir3,
    pub max_distance: f32,
    /// When true, a sweep that starts already overlapping something reports the
    /// hit at distance zero instead of refusing to move. A character resting on
    /// the floor is always slightly overlapping it, so this is on in practice.
    pub ignore_origin_penetration: bool,
}

/// A collide-and-slide move.
#[derive(Clone, Copy, Debug)]
pub struct Slide {
    pub shape: SweepShape,
    pub origin: Vec3,
    pub rotation: Quat,
    /// The movement to attempt. What actually happened comes back as
    /// [`SlideOutcome::actual_delta`].
    pub delta: Vec3,
    /// Surfaces steeper than this count as walls rather than floors, in
    /// degrees from horizontal.
    pub max_slope_deg: f32,
}

/// What a [`Slide`] resolved to.
///
/// The caller applies `actual_delta` to its own `Transform`; the backend only
/// does the query math, and deliberately writes nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct SlideOutcome {
    /// The movement actually applied after clipping and sliding.
    pub actual_delta: Vec3,
    /// True if a downward probe found ground at the end of the slide.
    pub grounded: bool,
    /// True if the slide hit a non-walkable surface during iteration.
    pub hit_wall: bool,
    /// Normal of the last ground hit, or `Vec3::Y` when airborne.
    pub ground_normal: Vec3,
}

/// The backend's query entry points.
///
/// Installed by `renzora_physics` at startup and absent when no 3D backend is
/// running. `Copy`, so a caller lifts it out of the world once and then spends
/// the rest of its system handing `&mut World` to it.
#[derive(Resource, Clone, Copy)]
pub struct SpatialQueries {
    pub cast_ray: fn(&mut World, &RayCast, &SpatialFilter) -> Option<SpatialHit>,
    pub cast_shape: fn(&mut World, &ShapeCast, &SpatialFilter) -> Option<SpatialHit>,
    pub slide: fn(&mut World, &Slide, &SpatialFilter) -> SlideOutcome,
    /// The world-space bounds of an entity's collider, if it has one.
    ///
    /// Here because the alternative for a plugin is deriving bounds from the
    /// render mesh, which is a different box: a ladder's collider is what the
    /// character has to line up with, not its handrail geometry.
    pub collider_aabb: fn(&mut World, Entity) -> Option<Aabb3d>,
}

impl SpatialQueries {
    /// Copy the installed table out of the world.
    ///
    /// `None` when no 3D backend is running. Copying rather than borrowing is
    /// the point: every entry point below wants `&mut World`, and a live `Res`
    /// borrow would make all of them uncallable.
    pub fn get(world: &World) -> Option<Self> {
        world.get_resource::<Self>().copied()
    }

    /// Cast a ray. See [`RayCast`].
    pub fn ray(
        &self,
        world: &mut World,
        cast: &RayCast,
        filter: &SpatialFilter,
    ) -> Option<SpatialHit> {
        (self.cast_ray)(world, cast, filter)
    }

    /// Sweep a shape. See [`ShapeCast`].
    pub fn shape(
        &self,
        world: &mut World,
        cast: &ShapeCast,
        filter: &SpatialFilter,
    ) -> Option<SpatialHit> {
        (self.cast_shape)(world, cast, filter)
    }

    /// Move a shape with collide-and-slide. See [`Slide`].
    pub fn slide_shape(
        &self,
        world: &mut World,
        slide: &Slide,
        filter: &SpatialFilter,
    ) -> SlideOutcome {
        (self.slide)(world, slide, filter)
    }

    /// World-space bounds of an entity's collider.
    pub fn aabb(&self, world: &mut World, entity: Entity) -> Option<Aabb3d> {
        (self.collider_aabb)(world, entity)
    }
}
