//! The physics contract.
//!
//! A layer of the contract hierarchy, beside `renzora` (it needs nothing from
//! it). Plugins reach these modules at the paths they always had
//! (`renzora::physics`, `renzora::vehicle`, `renzora::scene_query`) through
//! `renzora_sdk`; the engine's own crates depend on this crate directly.

// The components scenes save, the runtime state the engine and a backend
// exchange, and the `PhysicsBackend` trait a physics engine implements. In the
// contract because a backend plugin must name these types without linking
// `renzora_physics`. `PhysicsBackend` and `Caps` read better qualified, and
// `Caps` would collide with audio's, so callers name the module.
pub mod physics;

// Wheeled vehicles, run by `renzora_physics` on whichever backend simulates.
pub mod vehicle;

// Ray and sphere casts against the scene's physics colliders, for crates that
// must not link a physics engine (particles collide with the real level through
// it). The physics crate installs the service; a build without one simply has
// no `SceneQueries` resource. `SceneRay` and friends read better qualified at
// the call site.
pub mod scene_query;
