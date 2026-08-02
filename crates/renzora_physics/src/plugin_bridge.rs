//! The physics half of the standalone-plugin boundary.
//!
//! Same construction as `renzora_animation::plugin_bridge`, and for the same
//! reasons: `renzora_plugin::sys` is the frozen mechanism and does not know
//! physics exists; `renzora_plugin::physics` is the vocabulary, Bevy-free so
//! the same definitions compile into a plugin and into this crate; and this
//! module is the engine side that claims the service and turns bytes into work.
//!
//! ## Writes
//!
//! A plugin's `commands.entity(e).apply_impulse(v)` encodes a
//! [`PhysicsCommand`] tagged with [`renzora_plugin::physics::SERVICE`].
//! [`drain_plugin_physics_commands`] takes the calls bearing that tag — and only
//! those — and re-emits them as the same [`renzora::ScriptAction`] events a Lua
//! script or a blueprint node would have fired. Going through the existing
//! observer rather than around it means a plugin's impulse takes exactly the
//! path a script's does, including the 2D/3D dispatch, and there is one
//! implementation to keep correct rather than two.
//!
//! ## Reads
//!
//! [`PhysicsReadState`] and [`CollisionReadState`] cannot cross — `String` and
//! `HashSet` have no C layout. [`PluginPhysicsState`] is a numeric-only mirror
//! of both that can, delivered as an ordinary query cell, so a controller
//! polling `is_grounded()` every frame makes no calls into the engine.
//!
//! It is `#[repr(transparent)]` over the very
//! [`PhysicsState`](renzora_plugin::physics::PhysicsState) the plugin reads, so
//! the two layouts cannot drift.

use bevy::prelude::*;

use renzora_plugin::host::PluginServiceCalls;
use renzora_plugin::physics::{PhysicsCommand, PhysicsOp, PhysicsState};

use crate::read_state::{CollisionReadState, PhysicsReadState};

/// Numeric physics state, readable from a standalone plugin.
///
/// Plugins name this type by the string in `renzora_plugin::physics`'s
/// `host_component!` call, so **renaming this type or moving this module breaks
/// every plugin's physics reads.** Nothing fails to compile if the two stop
/// matching, which is why [`install`] asserts they agree at startup.
///
/// Reflected as **opaque**: [`PhysicsState`] lives in a Bevy-free module and
/// cannot derive `Reflect`. Scripts read [`PhysicsReadState`], which exists for
/// that; this only has to be *registered* so a plugin's host component resolves
/// through `AppTypeRegistry::get_with_type_path`.
#[derive(Component, Clone, Copy, Default, Reflect)]
#[reflect(Component, opaque)]
#[repr(transparent)]
pub struct PluginPhysicsState(pub PhysicsState);

/// Keep [`PluginPhysicsState`] in step with the two script-facing mirrors.
///
/// Hangs off `PhysicsReadState` so both describe the same frame — the updaters
/// populate it and this copies from it, in that order. `CollisionReadState` is
/// optional because a body can exist before its collision mirror is inserted.
pub fn sync_plugin_physics_state(
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &PhysicsReadState,
        Option<&CollisionReadState>,
        Option<&mut PluginPhysicsState>,
    )>,
) {
    for (entity, phys, collision, existing) in &mut q {
        let next = PhysicsState {
            velocity: renzora_plugin::sys::Vec3 {
                x: phys.velocity.x,
                y: phys.velocity.y,
                z: phys.velocity.z,
            },
            ground_normal: renzora_plugin::sys::Vec3 {
                x: phys.ground_normal.x,
                y: phys.ground_normal.y,
                z: phys.ground_normal.z,
            },
            speed: phys.speed,
            grounded: phys.grounded as u32,
            colliding: collision.is_some_and(|c| c.colliding) as u32,
            entered: collision.is_some_and(|c| c.entered) as u32,
            exited: collision.is_some_and(|c| c.exited) as u32,
            _reserved: 0,
        };
        match existing {
            Some(mut slot) => slot.0 = next,
            None => {
                commands.entity(entity).try_insert(PluginPhysicsState(next));
            }
        }
    }
}

/// Turn parked plugin service calls into the script actions physics already
/// understands.
pub fn drain_plugin_physics_commands(
    mut parked: ResMut<PluginServiceCalls>,
    mut commands: Commands,
) {
    use renzora::ScriptActionValue as V;

    for call in parked.take(renzora_plugin::physics::SERVICE) {
        // The payload is bytes the host never looked at, so every check happens
        // here — and it is exact, not a minimum. A `<` check passes a payload that is the right
        // size or larger, which sounds forgiving and is not: a plugin built from
        // a version of `renzora_plugin` that REORDERED this struct sends exactly
        // the right number of bytes and is misread silently, field for field. The
        // domain modules sit outside the ABI version deliberately, so nothing else
        // catches that — not the handshake, not the interface prefix hash, and not
        // cargo, since a plugin's `renzora_plugin = "0.1"` resolves to any 0.1.x.
        // An equality check turns a silent misread into a clean refusal.
        if call.payload.len() != size_of::<PhysicsCommand>() {
            warn!(
                "[physics] plugin sent {} bytes for a physics command; expected {}",
                call.payload.len(),
                size_of::<PhysicsCommand>()
            );
            continue;
        }
        // SAFETY: length checked, and `PhysicsCommand` is `#[repr(C)]`
        // plain-old-data.
        let cmd = unsafe { call.payload.as_ptr().cast::<PhysicsCommand>().read_unaligned() };

        let name = match cmd.op {
            PhysicsOp::ApplyForce => "apply_force",
            PhysicsOp::ApplyImpulse => "apply_impulse",
            PhysicsOp::SetVelocity => "set_velocity",
            PhysicsOp::KinematicSlide => "kinematic_slide",
            // From a plugin built against a newer vocabulary. Nothing upstream
            // validates this — the host carried opaque bytes — so this is the
            // only place it can be caught.
            other => {
                warn!(
                    "[physics] plugin used physics op {} ({}), which this build does not have",
                    other.0,
                    other.name()
                );
                continue;
            }
        };

        let mut args = std::collections::HashMap::new();
        args.insert("x".to_string(), V::Float(cmd.vec.x));
        args.insert("y".to_string(), V::Float(cmd.vec.y));
        args.insert("z".to_string(), V::Float(cmd.vec.z));
        if cmd.op == PhysicsOp::KinematicSlide {
            args.insert("max_slope".to_string(), V::Float(cmd.value));
        }

        commands.trigger(renzora::ScriptAction {
            name: name.to_string(),
            entity: call.entity,
            // `None`: the plugin already named the entity it meant, and the
            // handler falls back to `entity` when this is absent. Naming a
            // target by string is a scripting affordance the ABI has no need
            // for — a plugin holds real `Entity` values.
            target_entity: None,
            args,
        });
    }
}

/// Wires both directions up. Called from `PhysicsPlugin::build`.
pub fn install(app: &mut App) {
    // `register_type` makes the path resolvable — a plugin names this component
    // by string. `register_component` gives it a `ComponentId` now rather than
    // lazily on first query, which would be long after the C-ABI plugins have
    // initialised and asked for it.
    app.register_type::<PluginPhysicsState>();
    app.world_mut().register_component::<PluginPhysicsState>();

    // And say it may be read as *data*, not merely filtered on. Every host
    // component is filterable; handing over raw bytes is the restricted
    // direction, because a mirror is matched by name and its layout is the
    // plugin author's problem. This type is `#[repr(C)]` plain data built for
    // exactly that, which is what makes it safe to expose and why the exposing
    // call lives here rather than in a list the plugin crate maintains.
    renzora_plugin::host::expose_component_data::<PluginPhysicsState>(app);

    // The two sides agree by *string*, and a mismatch is silent on both: the
    // plugin asks for a path nothing registered, gets `INVALID`, and its physics
    // queries match nothing forever. Assert it here, where the failure is a
    // startup panic naming both halves.
    let ours = <PluginPhysicsState as bevy::reflect::TypePath>::type_path();
    let theirs = <PhysicsState as renzora_plugin::ecs::Component>::TYPE_PATH;
    assert_eq!(
        ours, theirs,
        "renzora_plugin::physics names the physics mirror `{theirs}` but it is registered here \
         as `{ours}` — renaming or moving `PluginPhysicsState` breaks every plugin's physics \
         reads, so update the `host_component!` call to match"
    );

    app.add_systems(Update, drain_plugin_physics_commands);
    app.add_systems(
        Update,
        sync_plugin_physics_state.after(crate::read_state::auto_init_physics_read_state),
    );
}
