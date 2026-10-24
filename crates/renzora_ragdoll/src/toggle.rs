//! Ragdoll activation. The script calls land here as `renzora::ScriptAction`s
//! (the same indirection `renzora_physics::PhysicsScriptExtension` uses for
//! `apply_force` etc.) and only edit the `Ragdoll` component;
//! `apply_ragdoll_state` is the single place that reacts to it changing, so a
//! script call and an Inspector edit behave identically.
//!
//! Which bones simulate is `Ragdoll.bone`'s subtree (all of them when empty),
//! and how they hand the pose over depends on `Ragdoll.blend`; see
//! [`crate::blend`] for the blended and get-up paths.

use crate::blend::{BoneBlend, RagdollBlendState};
use crate::{Ragdoll, RagdollBone};
use bevy::animation::AnimatedBy;
use bevy::prelude::*;
use renzora::physics::{PhysicsBodyData, PhysicsBodyType};
use renzora::ScriptActionValue;
use renzora_animation::{AnimationCommand, AnimationCommandQueue};

/// A `blend` at or above this is a full ragdoll, which takes the bones away
/// from the animation player entirely instead of blending.
pub(crate) const FULL_BLEND: f32 = 0.999;

/// Holds a bone's `AnimatedBy` link while it is ragdolling, so deactivation can
/// reconnect the bone to the exact `AnimationPlayer` that drove it.
///
/// Pausing the player is **not** enough to free a bone for physics: Bevy's
/// `animate_targets` keeps writing the (now frozen) clip pose onto every bone
/// it drives each frame, stamping the pose back over the solver and pinning the
/// skeleton in place. Severing `AnimatedBy` is what actually stops that write;
/// the `renzora_animation` skeleton tagger notes "missing `AnimatedBy` means
/// clips silently do nothing". We deliberately leave `AnimationTargetId` on the
/// bone: it's the flag `ensure_animation_targets` keys off, so removing it would
/// make that system re-add `AnimatedBy` and undo the handoff.
///
/// Only a *full* ragdoll severs the link. A blended or partial one needs the
/// animation still playing, for the blend and for the bones that are not
/// ragdolling.
#[derive(Component, Clone, Copy)]
pub(crate) struct StashedAnimatedBy(Entity);

fn arg_f32(action: &renzora::ScriptAction, key: &str) -> Option<f32> {
    match action.args.get(key)? {
        ScriptActionValue::Float(f) => Some(*f),
        ScriptActionValue::Int(i) => Some(*i as f32),
        _ => None,
    }
}

pub fn handle_ragdoll_script_actions(
    trigger: On<renzora::ScriptAction>,
    mut ragdolls: Query<&mut Ragdoll>,
) {
    let action = trigger.event();
    let Ok(mut ragdoll) = ragdolls.get_mut(action.entity) else {
        return;
    };
    match action.name.as_str() {
        "enable_ragdoll" => {
            // The classic call means the whole skeleton, fully simulated, even
            // if a partial ragdoll was set up earlier.
            ragdoll.active = true;
            ragdoll.bone.clear();
            ragdoll.blend = 1.0;
        }
        "disable_ragdoll" => ragdoll.active = false,
        "enable_partial_ragdoll" => {
            ragdoll.active = true;
            ragdoll.bone = match action.args.get("bone") {
                Some(ScriptActionValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            if let Some(b) = arg_f32(action, "blend") {
                ragdoll.blend = b.clamp(0.0, 1.0);
            }
        }
        "set_ragdoll_blend" => {
            if let Some(b) = arg_f32(action, "blend") {
                let b = b.clamp(0.0, 1.0);
                // Only touch the component when it changes: every write marks it
                // changed, and a change re-walks the skeleton.
                if (ragdoll.blend - b).abs() > f32::EPSILON {
                    ragdoll.blend = b;
                }
            }
        }
        "set_ragdoll_get_up_time" => {
            if let Some(s) = arg_f32(action, "seconds") {
                ragdoll.get_up_time = s.max(0.0);
            }
        }
        _ => {}
    }
}

/// The per-skeleton settings [`apply_ragdoll_state`] walks one skeleton with.
struct Walk<'a> {
    root: Entity,
    active: bool,
    full: bool,
    bone_name: &'a str,
}

/// The queries the walk reads, grouped so the recursion carries one argument.
#[derive(bevy::ecs::system::SystemParam)]
pub struct WalkQueries<'w, 's> {
    children: Query<'w, 's, &'static Children>,
    names: Query<'w, 's, &'static Name>,
    animated_by: Query<'w, 's, &'static AnimatedBy>,
    stashed: Query<'w, 's, &'static StashedAnimatedBy>,
}

/// Re-applies the ragdoll's state to its bones whenever `Ragdoll` changes,
/// including the frame it is added. That first run is a harmless no-op: no
/// `RagdollBone`s exist yet (`generate::build_ragdolls`'s inserts haven't been
/// applied), and the default `active: false` matches the `Kinematic` state
/// bones are generated in.
///
/// For each bone: inside the chosen subtree and active, it becomes `Dynamic`;
/// otherwise `Kinematic`. A full ragdoll severs the bones' animation links
/// (see [`StashedAnimatedBy`]) and pauses the animator; anything less keeps
/// both, and [`crate::blend`] mixes the two poses.
pub fn apply_ragdoll_state(
    mut changed: Query<(Entity, &Ragdoll, Option<&mut RagdollBlendState>), Changed<Ragdoll>>,
    q: WalkQueries,
    mut bones: Query<(&mut PhysicsBodyData, &Transform, Option<&mut BoneBlend>), With<RagdollBone>>,
    mut commands: Commands,
    mut anim_queue: Option<ResMut<AnimationCommandQueue>>,
) {
    for (root, ragdoll, state) in &mut changed {
        let full = ragdoll.blend >= FULL_BLEND;
        let walk = Walk { root, active: ragdoll.active, full, bone_name: &ragdoll.bone };
        // A partial ragdoll whose bone is not found simulates nothing rather
        // than everything: a typo should not drop the whole character.
        let whole = ragdoll.bone.is_empty();
        if !whole && ragdoll.active && !subtree_exists(root, &ragdoll.bone, &q.children, &q.names) {
            warn!("Ragdoll on {root:?}: no bone named '{}' under it", ragdoll.bone);
        }
        set_bones_recursive(&walk, &q, root, whole, &mut bones, &mut commands);

        let pause = ragdoll.active && full && whole;
        let was_active;
        match state {
            Some(mut state) => {
                was_active = state.was_active;
                state.was_active = ragdoll.active;
                if was_active && !ragdoll.active {
                    state.fade = if ragdoll.get_up_time > 0.0 { 1.0 / ragdoll.get_up_time } else { f32::INFINITY };
                }
                if pause != state.paused {
                    state.paused = pause;
                    push_pause(&mut anim_queue, root, pause);
                }
            }
            None => {
                commands.entity(root).try_insert(RagdollBlendState {
                    was_active: ragdoll.active,
                    paused: pause,
                    ..default()
                });
                if pause {
                    push_pause(&mut anim_queue, root, true);
                }
            }
        }
    }
}

fn push_pause(queue: &mut Option<ResMut<AnimationCommandQueue>>, entity: Entity, pause: bool) {
    if let Some(queue) = queue {
        queue.commands.push(if pause {
            AnimationCommand::Pause { entity }
        } else {
            AnimationCommand::Resume { entity }
        });
    }
}

fn subtree_exists(root: Entity, bone: &str, children: &Query<&Children>, names: &Query<&Name>) -> bool {
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if names.get(e).is_ok_and(|n| n.as_str() == bone) {
            return true;
        }
        if let Ok(kids) = children.get(e) {
            stack.extend(kids.iter());
        }
    }
    false
}

fn set_bones_recursive(
    walk: &Walk,
    q: &WalkQueries,
    entity: Entity,
    in_set: bool,
    bones: &mut Query<(&mut PhysicsBodyData, &Transform, Option<&mut BoneBlend>), With<RagdollBone>>,
    commands: &mut Commands,
) {
    let in_set = in_set
        || (!walk.bone_name.is_empty() && q.names.get(entity).is_ok_and(|n| n.as_str() == walk.bone_name));
    if let Ok((mut body, transform, blend)) = bones.get_mut(entity) {
        let simulate = walk.active && in_set;
        let sever = simulate && walk.full;
        let wanted = if simulate { PhysicsBodyType::RigidBody } else { PhysicsBodyType::KinematicBody };
        if body.body_type != wanted {
            body.body_type = wanted;
        }
        if sever {
            // Hand the bone to physics: stash and sever its player link so
            // `animate_targets` stops overwriting the solver every frame. The
            // `if let` makes re-activation idempotent (nothing to stash once the
            // link is already gone).
            if let Ok(by) = q.animated_by.get(entity) {
                commands
                    .entity(entity)
                    .insert(StashedAnimatedBy(by.0))
                    .remove::<AnimatedBy>();
            }
        } else if let Ok(saved) = q.stashed.get(entity) {
            // Give the bone back to the animator: reconnect the exact player it
            // was detached from, then drop the stash.
            commands
                .entity(entity)
                .insert(AnimatedBy(saved.0))
                .remove::<StashedAnimatedBy>();
        }
        match blend {
            Some(mut b) => {
                // A bone that was simulating when the ragdoll switched off stays
                // in the get-up blend; a re-activation starts the set afresh.
                b.involved = simulate || (!walk.active && (b.simulated || b.involved));
                b.simulated = simulate;
                b.linked = !sever;
                b.root = walk.root;
            }
            None => {
                commands.entity(entity).try_insert(BoneBlend {
                    root: walk.root,
                    physics: *transform,
                    simulated: simulate,
                    linked: !sever,
                    involved: simulate,
                });
            }
        }
    }
    if let Ok(kids) = q.children.get(entity) {
        for kid in kids.iter() {
            set_bones_recursive(walk, q, kid, in_set, bones, commands);
        }
    }
}
