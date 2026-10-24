//! Skeletal ragdoll physics.
//!
//! Walks a GLTF skeleton (the `SkinnedMesh` joint list reachable from the
//! `Ragdoll` entity) and auto-builds one rigid body + collider per bone plus a
//! spherical `PhysicsJoint` between every parent/child bone pair, mirroring
//! the bind pose so the ragdoll doesn't pop on activation. All of it is
//! `renzora::physics` data, so it runs on whichever backend the project picks,
//! provided that backend simulates joints.
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
//! That is the *full* ragdoll. `Ragdoll.bone` narrows it to one bone's subtree
//! (an arm hit by a shot hangs off a body that keeps animating), and
//! `Ragdoll.blend` below `1` mixes the simulated bones with the still-playing
//! clip instead of handing them over. Switching off eases back to the
//! animation over `get_up_time`. Both need the animation link kept, so they go
//! through a pose split described in `blend`.
//!
//! Ragdoll is a *simulation* feature: it only does anything while physics is
//! running, i.e. in **Play mode**. Toggling `Ragdoll.active` in the editor (where
//! the simulation is paused) just freezes the pose.

mod blend;
mod generate;
#[cfg(feature = "scripting")]
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
///
/// The tuning fields (`stiffness` and below) are read once, by
/// `generate::build_ragdolls`, when the bone bodies and joints are first
/// created — editing them afterward has no effect on an already-built
/// ragdoll. To re-tune, remove and re-add `Ragdoll` (or edit the fields
/// before the skeleton finishes generating, e.g. before the first Play).
// `renzora::Inspectable` is itself gated behind `renzora`'s `editor` feature,
// so the derive (and its `#[inspectable(...)]` attribute) must be applied
// through `cfg_attr` rather than directly — a plain `#[derive(renzora::Inspectable)]`
// would fail to resolve in a lean build with `editor` off. Going through
// `renzora_macros` (always present, feature-independent) rather than the
// gated `renzora::Inspectable` re-export keeps the macro *name* resolvable
// even when the attribute itself is compiled out.
#[derive(Component, Reflect, Clone)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
// `Default` (the manual impl below) must be in the reflect attribute, not just
// derived, so `ReflectDefault` is registered. Without it, loading a scene saved
// before this struct gained `linear_damping`/`angular_damping`/`gravity_scale`
// panics: `FromReflect` returns `None` for the partial (missing-field) data and
// there's no default to fall back to. With it, the missing fields fill from the
// manual `Default` and the saved fields apply on top.
#[reflect(Component, Default)]
#[cfg_attr(
    feature = "editor",
    inspectable(name = "Ragdoll", icon = "BONE", category = "physics")
)]
pub struct Ragdoll {
    pub active: bool,
    /// Joint rigidity, `0` (loose/floppy) to `1` (rigid). Limbs holding their
    /// bend under the swing/twist limits below, rather than flopping freely,
    /// comes from this — not from the limits alone.
    #[cfg_attr(
        feature = "editor",
        field(name = "Stiffness", speed = 0.01, min = 0.0, max = 1.0)
    )]
    pub stiffness: f32,
    /// Max degrees a joint may swing off its bone's rest axis (an elbow or
    /// knee opening up, a shoulder lifting). `0` locks the joint straight.
    #[cfg_attr(
        feature = "editor",
        field(name = "Swing Limit", speed = 1.0, min = 0.0, max = 170.0)
    )]
    pub swing_limit_degrees: f32,
    /// Max degrees a joint may twist about its bone's rest axis (a forearm
    /// rotating). `0` locks out twisting entirely.
    #[cfg_attr(
        feature = "editor",
        field(name = "Twist Limit", speed = 1.0, min = 0.0, max = 170.0)
    )]
    pub twist_limit_degrees: f32,
    /// Drag on each bone's linear velocity. Higher reads as heavier and
    /// less flaily; this is the main knob against limbs whipping around.
    #[cfg_attr(
        feature = "editor",
        field(name = "Linear Damping", speed = 0.1, min = 0.0, max = 20.0)
    )]
    pub linear_damping: f32,
    /// Drag on each bone's angular velocity — resists spinning limbs
    /// independently of `linear_damping`.
    #[cfg_attr(
        feature = "editor",
        field(name = "Angular Damping", speed = 0.1, min = 0.0, max = 20.0)
    )]
    pub angular_damping: f32,
    /// Per-bone gravity multiplier. Lower makes the ragdoll fall and settle
    /// more slowly — the direct knob against "drops too fast".
    #[cfg_attr(
        feature = "editor",
        field(name = "Gravity Scale", speed = 0.01, min = 0.0, max = 3.0)
    )]
    pub gravity_scale: f32,
    /// How much of the pose the physics owns while active: `1` is a full
    /// ragdoll, lower blends the simulated bones with the still-playing
    /// animation (a stagger, a character bracing against a hit). Unlike the
    /// tuning fields above this is read every frame, so a script can ramp it.
    #[reflect(default = "default_blend")]
    #[cfg_attr(
        feature = "editor",
        field(name = "Physics Blend", speed = 0.01, min = 0.0, max = 1.0)
    )]
    pub blend: f32,
    /// Name of the bone whose subtree ragdolls (an arm hit by a shot: its
    /// upper-arm bone). Empty ragdolls the whole skeleton. The rest of the
    /// skeleton keeps animating, and the ragdolled part hangs off it by its
    /// joint.
    #[reflect(default)]
    #[cfg_attr(feature = "editor", field(name = "Partial Bone"))]
    pub bone: String,
    /// Seconds to blend back to the animation when the ragdoll switches off,
    /// so a character gets up instead of snapping into its idle. `0` snaps.
    #[reflect(default = "default_get_up_time")]
    #[cfg_attr(
        feature = "editor",
        field(name = "Get Up Time", speed = 0.01, min = 0.0, max = 10.0)
    )]
    pub get_up_time: f32,
}

fn default_blend() -> f32 {
    1.0
}

fn default_get_up_time() -> f32 {
    0.5
}

impl Default for Ragdoll {
    fn default() -> Self {
        Self {
            active: false,
            stiffness: 0.85,
            swing_limit_degrees: 60.0,
            twist_limit_degrees: 30.0,
            linear_damping: 2.0,
            angular_damping: 4.0,
            gravity_scale: 0.6,
            blend: default_blend(),
            bone: String::new(),
            get_up_time: default_get_up_time(),
        }
    }
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
        info!("[runtime] RagdollPlugin (skeletal ragdoll)");

        app.register_type::<RagdollBone>()
            .add_systems(
                Update,
                (
                    generate::build_ragdolls,
                    (toggle::apply_ragdoll_state, blend::update_blend_weights).chain(),
                ),
            )
            // The blended and partial paths (see `blend`): physics' own pose
            // back before the step, recorded after it, mixed with the clip
            // after the animation player.
            .add_systems(First, blend::restore_physics_pose)
            .add_systems(
                PostUpdate,
                (
                    blend::capture_physics_pose.before(bevy::app::AnimationSystems),
                    blend::blend_ragdoll_pose
                        .after(bevy::app::AnimationSystems)
                        .before(bevy::transform::TransformSystems::Propagate),
                ),
            )
            .add_observer(toggle::handle_ragdoll_script_actions);

        #[cfg(feature = "editor")]
        app.register_inspectable::<Ragdoll>();
        #[cfg(not(feature = "editor"))]
        app.register_type::<Ragdoll>();

        // Script functions owned by the ragdoll crate. Braced so the `cfg` has a
        // single item to sit on.
        #[cfg(feature = "scripting")]
        {
            let mut extensions = app.world_mut().get_resource_or_insert_with(
                renzora_scripting::extension::ScriptExtensions::default,
            );
            extensions.register(script_extension::RagdollScriptExtension);
        }
    }
}

renzora::add!(RagdollPlugin);
