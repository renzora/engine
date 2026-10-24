//! Runtime procedural animation on skeletons.
//!
//! Components a user adds to bones (or to the character) in the Inspector,
//! solved every frame **after** the animation player has written the pose
//! (Bevy's `AnimationSystems`) and **before** transform propagation, so the
//! rendered skin, attached props and next frame's physics all see the
//! constrained pose:
//!
//! - [`TwoBoneIk`]: an arm or leg reaching for a target, with a pole.
//! - [`LookAt`]: a bone aiming an axis at a target, within a cone.
//! - [`FootIk`]: feet planted on uneven ground through `renzora::physics`
//!   casts, hips lowered to keep both feet reachable, feet tilted onto the
//!   slope.
//! - [`RotationLimit`]: swing/twist limits.
//! - [`CopyRotation`] / [`CopyPosition`]: follow another entity, per axis.
//! - [`SpringBones`] + [`SpringCollider`]: jiggle chains (hair, tails, ears),
//!   at a fixed simulation step so they behave the same at any frame rate.
//!
//! Everything only runs while the game runs (`renzora::core::game_running`),
//! like animation, cloth and physics: while editing, a model holds exactly the
//! pose the scene file says. Leaving Play hands every unanimated bone its
//! pre-constraint pose back (see [`pose`]).
//!
//! The math is in [`solve`], plain functions on vectors and quaternions with
//! their own tests; [`apply`] is the ECS around it.

pub mod apply;
pub mod components;
pub mod names;
pub mod pose;
mod script;
#[cfg(feature = "scripting")]
mod script_extension;
pub mod solve;

pub use components::{
    CopyPosition, CopyRotation, FootIk, LookAt, RotationLimit, SpringBones, SpringCollider, TwoBoneIk,
};

use bevy::prelude::*;
use renzora::core::game_running;
#[cfg(feature = "editor")]
use renzora::AppEditorExt;

/// Where the constraints run in `PostUpdate`: after the animation player and
/// before propagation. Public so a crate that writes bone poses of its own can
/// order itself against the whole pass.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IkSystems;

#[derive(Default)]
pub struct IkPlugin;

impl Plugin for IkPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] IkPlugin (IK, constraints, spring bones)");

        app.init_resource::<names::NameLookup>()
            .init_resource::<apply::foot::FootRays>()
            .configure_sets(
                PostUpdate,
                IkSystems
                    .after(bevy::app::AnimationSystems)
                    .before(bevy::transform::TransformSystems::Propagate),
            )
            .add_systems(
                PostUpdate,
                (
                    names::update_name_lookup,
                    (
                        apply::foot::ensure_foot_state,
                        apply::spring::ensure_spring_state,
                        pose::restore_inputs,
                        apply::constraints::apply_copy_constraints,
                        apply::foot::gather_foot_rays,
                        apply::foot::cast_foot_rays,
                        apply::foot::apply_foot_ik,
                        apply::constraints::apply_two_bone_ik,
                        apply::constraints::apply_look_at,
                        apply::constraints::apply_rotation_limits,
                        apply::spring::apply_spring_bones,
                        pose::record_outputs,
                    )
                        .chain()
                        .run_if(game_running),
                    pose::release_on_stop.run_if(not(game_running)),
                )
                    .chain()
                    .in_set(IkSystems),
            )
            .add_observer(apply::constraints::capture_limit_reference)
            .add_observer(script::handle_ik_script_actions);

        #[cfg(feature = "editor")]
        {
            app.register_inspectable::<TwoBoneIk>()
                .register_inspectable::<LookAt>()
                .register_inspectable::<FootIk>()
                .register_inspectable::<RotationLimit>()
                .register_inspectable::<CopyRotation>()
                .register_inspectable::<CopyPosition>()
                .register_inspectable::<SpringBones>()
                .register_inspectable::<SpringCollider>();
        }
        #[cfg(not(feature = "editor"))]
        {
            app.register_type::<TwoBoneIk>()
                .register_type::<LookAt>()
                .register_type::<FootIk>()
                .register_type::<RotationLimit>()
                .register_type::<CopyRotation>()
                .register_type::<CopyPosition>()
                .register_type::<SpringBones>()
                .register_type::<SpringCollider>();
        }

        #[cfg(feature = "scripting")]
        {
            let mut extensions = app.world_mut().get_resource_or_insert_with(
                renzora_scripting::extension::ScriptExtensions::default,
            );
            extensions.register(script_extension::IkScriptExtension);
        }
    }
}

renzora::add!(IkPlugin);
