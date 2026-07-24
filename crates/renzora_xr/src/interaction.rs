//! Controller interaction: proximity grab.
//!
//! v1 semantics — squeeze a controller's grip near a scene entity to pick it
//! up; it rides the wand until the grip releases, then keeps the pose it was
//! released at. Implementation is deliberately physics-agnostic: the grabbed
//! entity is reparented under the wand with its world pose preserved (local
//! transform recomputed on both ends), so ANY named scene entity works —
//! props, lights, splat clouds. Follow-ups tracked in the roadmap: throw
//! velocity on release (needs avian), ray-based pick for distant objects.
//!
//! Grab targets are `Name`d scene entities (the same rule scene saving uses),
//! excluding the XR rig itself and editor chrome (`HideInHierarchy`).

use bevy::prelude::*;
use bevy_xr_utils::tracking_utils::{XrTrackedLeftGrip, XrTrackedRightGrip};

use crate::rig::VrControllerVisual;
use crate::VrInput;

/// How close (meters) a grab point must be to an entity's origin to pick it
/// up. Generous because v1 measures to the entity ORIGIN, not its surface.
const GRAB_RADIUS: f32 = 0.35;
/// Grip axis thresholds to start / release a grab (hysteresis).
const GRIP_CLOSE: f32 = 0.7;
const GRIP_OPEN: f32 = 0.3;

/// Stashed restore-state on a grabbed entity.
#[derive(Component)]
pub struct Grabbed {
    /// Which hand holds it (`true` = left).
    left: bool,
    /// Parent to restore on release (`None` = scene root).
    original_parent: Option<Entity>,
}

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, (grab_begin, grab_end));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn grab_begin(
    mut commands: Commands,
    input: Res<VrInput>,
    play: Option<Res<renzora::VrPlayState>>,
    left_hand: Query<Entity, With<XrTrackedLeftGrip>>,
    right_hand: Query<Entity, With<XrTrackedRightGrip>>,
    transforms: Query<&GlobalTransform>,
    held: Query<&Grabbed>,
    candidates: Query<
        (Entity, &GlobalTransform, Option<&ChildOf>),
        (
            With<Name>,
            Without<renzora::HideInHierarchy>,
            Without<VrControllerVisual>,
            Without<bevy_mod_xr::session::XrTracker>,
            Without<Grabbed>,
        ),
    >,
    mut was_closed: Local<[bool; 2]>,
) {
    if !play.is_some_and(|p| p.active) {
        return;
    }
    for (index, (left, grip)) in [(true, input.left_grip), (false, input.right_grip)]
        .into_iter()
        .enumerate()
    {
        // Rising-edge with hysteresis so a half-squeezed grip doesn't
        // machine-gun grab attempts.
        let closed = grip > GRIP_CLOSE;
        let rising_edge = closed && !was_closed[index];
        if closed {
            was_closed[index] = true;
        } else if grip < GRIP_OPEN {
            was_closed[index] = false;
        }
        if !rising_edge {
            continue;
        }
        // One object per hand.
        if held.iter().any(|g| g.left == left) {
            continue;
        }
        let hand = if left {
            left_hand.iter().next()
        } else {
            right_hand.iter().next()
        };
        let Some(hand) = hand else { continue };
        let Ok(hand_tf) = transforms.get(hand) else {
            continue;
        };
        let hand_pos = hand_tf.translation();

        let nearest = candidates
            .iter()
            .map(|(entity, tf, child_of)| {
                (entity, tf, child_of, tf.translation().distance(hand_pos))
            })
            .filter(|(.., distance)| *distance < GRAB_RADIUS)
            .min_by(|a, b| a.3.total_cmp(&b.3));
        let Some((entity, entity_tf, child_of, _)) = nearest else {
            continue;
        };

        // Reparent under the wand, preserving the world pose: the new local
        // transform is the entity's pose expressed in wand space.
        let local = Transform::from_matrix(Mat4::from(
            hand_tf.affine().inverse() * entity_tf.affine(),
        ));
        commands.entity(entity).insert((
            Grabbed {
                left,
                original_parent: child_of.map(|c| c.parent()),
            },
            ChildOf(hand),
            local,
        ));
        info!(
            "[XR] grabbed {entity:?} with {} hand",
            if left { "left" } else { "right" }
        );
    }
}

/// Release: restore the original parent (or detach to the scene root), with
/// the local transform recomputed so the object keeps its release-time world
/// pose instead of teleporting into the old parent's frame.
fn grab_end(
    mut commands: Commands,
    input: Res<VrInput>,
    held: Query<(Entity, &Grabbed, &GlobalTransform)>,
    parent_transforms: Query<&GlobalTransform, Without<Grabbed>>,
) {
    for (entity, grabbed, entity_tf) in held.iter() {
        let grip = if grabbed.left {
            input.left_grip
        } else {
            input.right_grip
        };
        if grip > GRIP_OPEN {
            continue;
        }
        match grabbed.original_parent {
            Some(parent) => {
                let local = parent_transforms
                    .get(parent)
                    .map(|parent_tf| {
                        Transform::from_matrix(Mat4::from(
                            parent_tf.affine().inverse() * entity_tf.affine(),
                        ))
                    })
                    // Original parent gone (despawned mid-grab): fall back to
                    // detaching at the world pose.
                    .unwrap_or_else(|_| entity_tf.compute_transform());
                match parent_transforms.get(parent) {
                    Ok(_) => {
                        commands.entity(entity).insert((ChildOf(parent), local));
                    }
                    Err(_) => {
                        commands.entity(entity).remove::<ChildOf>();
                        commands.entity(entity).insert(local);
                    }
                }
            }
            None => {
                commands.entity(entity).remove::<ChildOf>();
                commands
                    .entity(entity)
                    .insert(entity_tf.compute_transform());
            }
        }
        commands.entity(entity).remove::<Grabbed>();
        info!("[XR] released {entity:?}");
    }
}
