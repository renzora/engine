//! Skeletal ragdoll physics distribution plugin.
//!
//! Walks a GLTF skeleton (the `SkinnedMesh` joint list reachable from the
//! `Ragdoll` entity) and auto-builds one Avian rigid body + collider per bone
//! plus a `SphericalJoint` between every parent/child bone pair, mirroring
//! the bind pose so the ragdoll doesn't pop on activation.
//!
//! Bones start `Kinematic` — driven by the `AnimationPlayer` exactly as if
//! this plugin weren't present. `enable_ragdoll(entity)` flips them to
//! `Dynamic` (the joints then take over) and, crucially, **detaches each bone
//! from its animation player** (severs `AnimatedBy`) so Bevy's `animate_targets`
//! stops stamping the clip pose back over the solver every frame — pausing the
//! player alone does not stop that per-frame write, so the skeleton would
//! otherwise stay pinned in place. The animator is paused too, for tidiness.
//! `disable_ragdoll(entity)` reverses both (reconnect + resume).
//!
//! Ragdoll is a *simulation* feature: it only does anything while physics is
//! running, i.e. in **Play mode**. Toggling `Ragdoll.active` in the editor (where
//! the simulation is paused) just freezes the pose.

mod generate;
mod script_extension;
mod toggle;

use bevy::prelude::*;
#[cfg(feature = "editor")]
use renzora::AppEditorExt;

/// Marker placed on a skeleton root (an ancestor of the `SkinnedMesh` entity,
/// typically the same entity as `renzora_animation::AnimatorComponent`).
/// Adding it triggers one-time bone -> physics-body generation; `active`
/// tracks whether the skeleton is currently ragdolling — flip it directly
/// (e.g. from the Inspector) or via `enable_ragdoll()`/`disable_ragdoll()`,
/// both converge on `toggle::apply_ragdoll_state`.
// `renzora::Inspectable` is itself gated behind `renzora`'s `editor` feature,
// so the derive (and its `#[inspectable(...)]` attribute) must be applied
// through `cfg_attr` rather than directly — a plain `#[derive(renzora::Inspectable)]`
// would fail to resolve in a lean build with `editor` off. Going through
// `renzora_macros` (always present, feature-independent) rather than the
// gated `renzora::Inspectable` re-export keeps the macro *name* resolvable
// even when the attribute itself is compiled out.
#[derive(Component, Reflect, Default, Clone, Copy)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
#[reflect(Component)]
#[cfg_attr(
    feature = "editor",
    inspectable(name = "Ragdoll", icon = "BONE", category = "physics")
)]
pub struct Ragdoll {
    pub active: bool,
}

/// Marker on every bone entity this plugin generated a rigid body for, so
/// activation can walk just the ragdoll's own bones instead of every
/// descendant of the root.
#[derive(Component, Reflect, Default, Clone, Copy)]
#[reflect(Component)]
pub struct RagdollBone;

#[derive(Default)]
pub struct RagdollPlugin;

impl Plugin for RagdollPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] RagdollPlugin (avian skeletal ragdoll)");

        app.register_type::<RagdollBone>()
            .add_systems(Update, (generate::build_ragdolls, toggle::apply_ragdoll_state))
            .add_observer(toggle::handle_ragdoll_script_actions);

        #[cfg(feature = "editor")]
        app.register_inspectable::<Ragdoll>();
        #[cfg(not(feature = "editor"))]
        app.register_type::<Ragdoll>();

        let mut extensions = app.world_mut().get_resource_or_insert_with(
            renzora_scripting::extension::ScriptExtensions::default,
        );
        extensions.register(script_extension::RagdollScriptExtension);
    }
}

renzora::add!(RagdollPlugin);
