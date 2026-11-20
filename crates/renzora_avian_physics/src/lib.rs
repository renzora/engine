//! Avian as a physics backend, for both worlds: `avian3d` and `avian2d`.
//!
//! An engine crate, statically linked like the rest, behind `renzora_runtime`'s
//! `avian` feature so a game with no physics, or one whose Project Settings
//! pick another engine for every world, leaves it out. Avian is built on Bevy,
//! so it comes from `vendor/avian`, compiled against the engine's own `bevy/`;
//! that is also why this cannot be an installed plugin, which would compile a
//! second avian, and with it a second Bevy, of its own.
//!
//! Two backends, not one, because avian is two crates. `avian3d` and `avian2d`
//! are compiled from the same source but every type in them is distinct, so the
//! 3D and 2D worlds are separate simulations and each registers as its own
//! backend (`"avian3d"`, `"avian2d"`), claiming one dimension. A project can
//! therefore run avian for 3D and another engine for 2D.
//!
//! Each backend maps the engine's physics API onto avian's own components:
//!
//! | contract | avian |
//! |---|---|
//! | `RuntimePhysics` + `PhysicsBodyData` | `RigidBody`, `Mass`, `GravityScale`, damping, `LockedAxes` |
//! | `ResolvedCollider` | `Collider`, `Friction`, `Restitution`, `Sensor`, `CollisionLayers` |
//! | `PhysicsJoint` | `FixedJoint`, `SphericalJoint`, `RevoluteJoint`, `PrismaticJoint`, `DistanceJoint` |
//! | `PhysicsCommands` | `ConstantForce`, `ConstantTorque`, impulses, velocities |
//! | `PhysicsSettings` | `Gravity`, `Time<Physics>`, `SubstepCount` |
//! | `BodyVelocity`, `PhysicsContacts` | `LinearVelocity`, `AngularVelocity`, `Collisions` |

use bevy::prelude::*;

pub mod backend_2d;
pub mod backend_3d;

#[derive(Default)]
pub struct AvianPhysicsPlugin;

impl Plugin for AvianPhysicsPlugin {
    fn build(&self, app: &mut App) {
        info!("[plugin] AvianPhysicsPlugin");
        // Owned by the engine's physics plugin, initialised here too so a backend
        // system never fails validation whichever is added first.
        app.init_resource::<renzora::physics::PhysicsSettings>()
            .init_resource::<renzora::physics::PhysicsCommands>();
        // 3D first: the 2D backend leaves out the one avian plugin both crates
        // share, and relies on the 3D one having added it (see `backend_2d`).
        app.add_plugins(backend_3d::Avian3dPlugin);
        app.add_plugins(backend_2d::Avian2dPlugin);
    }
}

renzora::add!(AvianPhysicsPlugin, Runtime);

/// True if any ANCESTOR of `entity` carries `PhysicsBodyData`, i.e. this
/// entity's collider should attach to that body rather than get a body of
/// its own.
///
/// Both backends give a collider with no body, on itself or above it, an
/// explicit static one: avian puts a body-less collider in its "standalone"
/// tree, which stops nothing. In 3D that let the player fall straight through a
/// level made solid by a Mesh collider on its imported root, which has no body
/// because nothing in the editor asked for one.
pub(crate) fn has_body_ancestor(
    entity: Entity,
    bodies: &Query<(), With<renzora::physics::PhysicsBodyData>>,
    parents: &Query<&ChildOf>,
) -> bool {
    parents.iter_ancestors(entity).any(|e| bodies.contains(e))
}

/// Marker: the `ConstantForce` on this entity came from `PhysicsCommand::AddForce`
/// and is replaced by the next frame's commands. A force anything else put there
/// is left alone.
#[derive(Component)]
pub struct CommandForce;
