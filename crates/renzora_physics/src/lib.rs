//! What is left of the physics crate once the subsystem itself moved.
//!
//! The simulation, the backends, the authored data, the read-state mirrors, the
//! spatial-query bridge and `PhysicsPlugin` all live in
//! [`renzora::physics`](renzora::physics) now. A native plugin links `bevy`,
//! `renzora` and `renzora_ember` and nothing else, so putting them there is
//! what makes physics reachable from one — not just describable through a
//! handful of function pointers, but the real `SpatialQuery`, the real
//! `RigidBody`, the real contact list.
//!
//! # Why this crate still exists
//!
//! One thing could not come with the rest: [`script_extension`]. It is defined
//! over `renzora_scripting`'s `ScriptExtension` trait, and `renzora_scripting`
//! depends on `renzora`. Moving it would close a dependency cycle, and the
//! contract crate's dependency list is kept to `bevy` + serialization + the
//! plugin ABI precisely so that can never happen.
//!
//! So this crate is the declaration of the physics script verbs, plus a
//! re-export of everything that moved. The re-exports are not deprecation
//! scaffolding: seven crates already write `renzora_physics::CollisionShapeData`
//! and there is nothing wrong with that path.

/// The physics script verbs (`apply_force`, `apply_impulse`, `set_velocity`,
/// `kinematic_slide`), declared rather than written.
///
/// The one part of this crate that is not a re-export, and the reason it is not
/// in `renzora`: see the module doc.
#[cfg(feature = "scripting")]
pub mod script_extension;

use bevy::prelude::*;

// Everything that moved, from its old path. `pub use` rather than `pub mod`
// aliases so `renzora_physics::backend::avian_character::shape_cast_slide` and
// `renzora_physics::data::CollisionShapeData` both still resolve.
/// The engine side of the C-ABI physics surface, for *standalone* plugins.
///
/// Did not move with the rest, for two reasons. It needs
/// `renzora_plugin/host`, which pulls `libloading` and a file watcher — neither
/// belongs in the crate every other crate depends on. And it is the wrong
/// boundary for the move anyway: a native plugin reaches the real avian through
/// [`renzora::physics`], and never speaks this protocol.
pub mod plugin_bridge;

pub use renzora::physics::{auto_fit, backend, data, properties, read_state, spatial};
pub use renzora::physics::{
    CollisionReadState, CollisionShapeData, CollisionShapeType, GravityPreset, Physics2d,
    PhysicsBodyData, PhysicsBodyType, PhysicsPropertiesState, PhysicsPropertyCommand,
    PhysicsReadState, RuntimePhysics, RuntimePhysics2d, SkipAutoFit,
};
pub use renzora::physics::plugin::{
    despawn_physics_components, pause, spawn_entity_physics, unpause, ColliderEditMode,
    PendingKinematicSlides, PendingSlide,
};

/// The physics plugin, plus this crate's script declarations.
///
/// A thin wrapper over [`renzora::physics::PhysicsPlugin`] rather than a second
/// plugin: the simulation is entirely that one's, and what this adds is the
/// `ScriptExtension` registration that could not move. A build without
/// scripting adds nothing at all here.
#[derive(Default)]
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(renzora::physics::PhysicsPlugin);

        // The C-ABI surface: standalone plugins driving and reading bodies.
        // Installed here rather than inside the plugin above, because it is the
        // half that could not move — see [`plugin_bridge`].
        plugin_bridge::install(app);

        // Register script functions owned by the physics crate. Braced so the
        // `cfg` has a single item to sit on.
        #[cfg(feature = "scripting")]
        {
            let mut extensions = app.world_mut().get_resource_or_insert_with(
                renzora_scripting::extension::ScriptExtensions::default,
            );
            extensions.register(script_extension::PhysicsScriptExtension);
        }
    }
}

// Deliberately NO `renzora::add!`. Physics is a *foundation* plugin, added by
// hand from `renzora_runtime` (see its `lib.rs`) so it is installed before the
// plugins that depend on a simulation existing. Declaring it here as well would
// have the generator write a second `add_plugins` into the runtime's list, and
// Bevy would build the whole subsystem twice.
