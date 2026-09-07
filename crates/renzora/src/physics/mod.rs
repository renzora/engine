//! The physics contract: what an entity's body and collider are, what the
//! simulation reported about them, and how to ask the collider tree a question.
//!
//! `renzora_physics` still owns every system that does the work: the avian
//! backends, the auto-fit pass, the read-state updaters, the script actions.
//! What lives here is the vocabulary those systems read and write, so a plugin
//! reaching only `bevy`, `renzora` and `renzora_ember` can author a rigid body,
//! read whether it is grounded, and cast against the world.
//!
//! Three modules, and the third is unlike anything else in this crate:
//!
//! * [`data`] is the authored, serialized shape of a body and its collider.
//!   Backend-agnostic on purpose, because a scene saved by an avian build has
//!   to load into whatever the engine simulates with next.
//! * [`read_state`] is the mirror the simulation writes back each frame, which
//!   is what `get("PhysicsReadState.grounded")` reads.
//! * [`spatial`] is a *bridge*, not data. See its module doc for why casts
//!   cannot be expressed the way everything else here is.
//!
//! # Why the split is where it is
//!
//! A type belongs here when both sides of the dlopen boundary have to agree on
//! it, and stays in `renzora_physics` when only that crate ever names it.
//! `PendingAutoFit` is the clean example of the second: it is a one-frame
//! marker the auto-fit pass sets and clears within itself, and nothing outside
//! could act on it. [`SkipAutoFit`] is the first, because a tilemap layer
//! spawning exact colliders has to be able to say so.

/// The 3D simulation itself, re-exported.
///
/// This is the engine's own avian, not a second copy, and that distinction is
/// the entire reason it is re-exported rather than depended on: a plugin cannot
/// list `avian3d` in its own manifest, because
/// `renzora_native_build::deps::reject_engine_crates` refuses any dependency
/// whose graph contains a `bevy_*` crate. It would have to be refused, too — a
/// separately compiled avian has a different `Collider` and a different
/// `RigidBody` from the ones the solver reads, so the plugin would build, load,
/// and quietly write components nothing simulates.
///
/// Reaching it through here means there is one of each.
///
/// ```ignore
/// use renzora::physics::avian3d::prelude::*;
///
/// // A plugin can take the real system parameter, with no `&mut World` and no
/// // exclusive system.
/// fn probe(spatial: SpatialQuery, q: Query<&Transform, With<RigidBody>>) { }
/// ```
///
/// [`spatial`] is still worth using for anything that should survive a change
/// of backend, and it is the only option in a build where the dimension is not
/// known ahead of time. Everything else is better served by the real thing.
#[cfg(feature = "avian3d")]
pub use avian3d;

/// The 2D simulation, re-exported. See [`avian3d`] for why.
///
/// A separate crate rather than a feature of the 3D one, so both can coexist in
/// one app: their `RigidBody`, `Collider` and `LinearVelocity` are distinct
/// types, and an entity is routed to one or the other at init.
#[cfg(feature = "avian2d")]
pub use avian2d;

pub mod auto_fit;
pub mod backend;
pub mod data;
pub mod plugin;
pub mod properties;
pub mod read_state;
pub mod spatial;
/// The backend half of [`spatial`], separate so the vocabulary compiles in a
/// build with no simulation at all.
#[cfg(feature = "avian3d")]
pub mod spatial_impl;

pub use auto_fit::SkipAutoFit;
pub use data::*;
pub use plugin::PhysicsPlugin;
pub use properties::*;
pub use read_state::{CollisionReadState, PhysicsReadState};
pub use spatial::{
    RayCast, ShapeCast, Slide, SlideOutcome, SpatialFilter, SpatialHit, SpatialQueries, SweepShape,
};
