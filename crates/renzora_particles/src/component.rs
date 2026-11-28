//! The components a scene holds, and the runtime handle built from them.
//!
//! The authored components ([`ParticleSystem`], [`ParticlePlayback`],
//! [`ParticleParams`], [`ParticleStats`]) live in the `renzora_particles_api`
//! contract layer, so a script or a plugin can spawn and drive an effect; they
//! are re-exported here at the paths this crate always used.
//!
//! [`ParticleSystem`] persists the effect by **path**: a `Handle` means nothing
//! in a saved scene, so the loaded handle is this crate's own runtime
//! component, [`ParticleAsset`], which the runtime fills from the path on every
//! scene load (see [`crate::systems::resolve_handles`]). Code that builds an
//! effect in memory hands its handle over with [`from_handle`] and leaves the
//! path empty.

use bevy::prelude::*;

use crate::asset::VfxAsset;

pub use renzora_particles_api::{
    OneShot, ParticleParams, ParticlePlayback, ParticleStats, ParticleSystem, PlayState,
    DEFAULT_EFFECT,
};

/// The loaded effect for a [`ParticleSystem`]. Runtime-only and never saved:
/// required by `ParticleSystem` (registered in the plugin), so it is on every
/// effect, and filled from the path by `resolve_handles`.
#[derive(Component, Clone, Debug, Default)]
pub struct ParticleAsset(pub Handle<VfxAsset>);

/// An effect built in code: no path, the handle given directly.
pub fn from_handle(asset: Handle<VfxAsset>) -> (ParticleSystem, ParticleAsset) {
    (ParticleSystem::from_path(""), ParticleAsset(asset))
}
