//! Ragdoll activation. `enable_ragdoll()` / `disable_ragdoll()` script calls
//! land here as `renzora::ScriptAction`s (the same indirection
//! `renzora_physics::PhysicsScriptExtension` uses for `apply_force` etc.) and
//! just flip `Ragdoll.active` — `apply_ragdoll_state` is the single place
//! that reacts to the flag changing, so a script toggle and an Inspector
//! checkbox edit behave identically.

use crate::{Ragdoll, RagdollBone};
use avian3d::prelude::RigidBody;
use bevy::animation::AnimatedBy;
use bevy::prelude::*;
use renzora_animation::{AnimationCommand, AnimationCommandQueue};

/// Holds a bone's `AnimatedBy` link while it is ragdolling, so deactivation can
/// reconnect the bone to the exact `AnimationPlayer` that drove it.
///
/// Pausing the player is **not** enough to free a bone for physics: Bevy's
/// `animate_targets` keeps writing the (now frozen) clip pose onto every bone
/// it drives each frame, stamping the pose back over the solver and pinning the
/// skeleton in place. Severing `AnimatedBy` is what actually stops that write —
/// the `renzora_animation` skeleton tagger notes "missing `AnimatedBy` means
/// clips silently do nothing". We deliberately leave `AnimationTargetId` on the
/// bone: it's the flag `ensure_animation_targets` keys off, so removing it would
/// make that system re-add `AnimatedBy` and undo the handoff.
#[derive(Component, Clone, Copy)]
pub(crate) struct StashedAnimatedBy(Entity);

pub fn handle_ragdoll_script_actions(
    trigger: On<renzora::ScriptAction>,
    mut ragdolls: Query<&mut Ragdoll>,
) {
    let action = trigger.event();
    let active = match action.name.as_str() {
        "enable_ragdoll" => true,
        "disable_ragdoll" => false,
        _ => return,
    };
    if let Ok(mut ragdoll) = ragdolls.get_mut(action.entity) {
        ragdoll.active = active;
    }
}

/// Flips every generated bone between `Kinematic` (the `AnimationPlayer`
/// drives it, physics just rides along for collision) and `Dynamic` (the
/// avian solver + joints drive it), detaches/reattaches each bone from its
/// animation player (see [`StashedAnimatedBy`]), and pauses/resumes the
/// animator through `renzora_animation`'s existing command queue, whenever
/// `Ragdoll.active` changes — including the frame it's first added. That's a
/// harmless no-op: no `RagdollBone`s exist yet (`generate::build_ragdolls`'s
/// inserts haven't been applied), and the default `active: false` matches the
/// `Kinematic` state bones are generated in.
pub fn apply_ragdoll_state(
    changed: Query<(Entity, &Ragdoll), Changed<Ragdoll>>,
    children: Query<&Children>,
    bones: Query<(), With<RagdollBone>>,
    animated_by: Query<&AnimatedBy>,
    stashed: Query<&StashedAnimatedBy>,
    mut commands: Commands,
    mut anim_queue: Option<ResMut<AnimationCommandQueue>>,
) {
    for (root, ragdoll) in &changed {
        set_bones_recursive(
            root,
            ragdoll.active,
            &children,
            &bones,
            &animated_by,
            &stashed,
            &mut commands,
        );

        if let Some(ref mut queue) = anim_queue {
            queue.commands.push(if ragdoll.active {
                AnimationCommand::Pause { entity: root }
            } else {
                AnimationCommand::Resume { entity: root }
            });
        }
    }
}

fn set_bones_recursive(
    entity: Entity,
    active: bool,
    children: &Query<&Children>,
    bones: &Query<(), With<RagdollBone>>,
    animated_by: &Query<&AnimatedBy>,
    stashed: &Query<&StashedAnimatedBy>,
    commands: &mut Commands,
) {
    if bones.get(entity).is_ok() {
        if active {
            commands.entity(entity).insert(RigidBody::Dynamic);
            // Hand the bone to physics: stash and sever its player link so
            // `animate_targets` stops overwriting the solver every frame. The
            // `if let` makes re-activation idempotent (nothing to stash once the
            // link is already gone).
            if let Ok(by) = animated_by.get(entity) {
                commands
                    .entity(entity)
                    .insert(StashedAnimatedBy(by.0))
                    .remove::<AnimatedBy>();
            }
        } else {
            commands.entity(entity).insert(RigidBody::Kinematic);
            // Give the bone back to the animator — reconnect the exact player it
            // was detached from, then drop the stash.
            if let Ok(saved) = stashed.get(entity) {
                commands
                    .entity(entity)
                    .insert(AnimatedBy(saved.0))
                    .remove::<StashedAnimatedBy>();
            }
        }
    }
    if let Ok(kids) = children.get(entity) {
        for kid in kids.iter() {
            set_bones_recursive(kid, active, children, bones, animated_by, stashed, commands);
        }
    }
}
