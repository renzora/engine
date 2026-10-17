//! [`FollowNode`]: put an entity where a named node is, every frame.
//!
//! Runs after Bevy has applied this frame's animation and before transforms
//! propagate, so the follower lands on the pose the node is drawn in rather
//! than the one it had a frame ago. A camera one frame behind a fly-through
//! judders against the geometry it is flying past.

use bevy::prelude::*;
use renzora::FollowNode;
use std::collections::HashMap;

/// Copy each follower's target world position and rotation onto it.
///
/// The target's world transform is composed from its ancestors' local
/// transforms here, because `GlobalTransform` is not recomputed until
/// propagation, which this has to run before.
pub fn follow_nodes(
    followers: Query<(Entity, &FollowNode, Option<&ChildOf>)>,
    names: Query<(Entity, &Name)>,
    mut transforms: ParamSet<(
        Query<(&Transform, Option<&ChildOf>)>,
        Query<&mut Transform, With<FollowNode>>,
    )>,
    mut resolved: Local<HashMap<Entity, Entity>>,
) {
    resolved.retain(|follower, _| followers.contains(*follower));

    let mut updates: Vec<(Entity, Vec3, Quat)> = Vec::new();
    for (follower, follow, parent) in &followers {
        if follow.target.is_empty() {
            continue;
        }
        let key = renzora::sanitize_id(&follow.target);
        let matches = |entity: Entity| {
            names
                .get(entity)
                .is_ok_and(|(_, name)| renzora::sanitize_id(name.as_str()) == key)
        };
        // Look the name up only when the cached entity no longer answers to
        // it: the target despawned, was renamed, or the field was edited.
        let target = match resolved.get(&follower) {
            Some(&cached) if matches(cached) => cached,
            _ => {
                let Some((found, _)) = names.iter().find(|(entity, name)| {
                    *entity != follower && renzora::sanitize_id(name.as_str()) == key
                }) else {
                    continue;
                };
                resolved.insert(follower, found);
                found
            }
        };

        let local = transforms.p0();
        let Some(target_world) = world_of(target, &local) else {
            continue;
        };
        // A parented follower needs the target in its parent's space.
        let wanted = match parent.and_then(|p| world_of(p.parent(), &local)) {
            Some(parent_world) => parent_world.affine().inverse() * target_world.affine(),
            None => target_world.affine(),
        };
        let (_, rotation, translation) = wanted.to_scale_rotation_translation();
        updates.push((follower, translation, rotation));
    }

    let mut writable = transforms.p1();
    for (follower, translation, rotation) in updates {
        if let Ok(mut transform) = writable.get_mut(follower) {
            transform.translation = translation;
            transform.rotation = rotation;
        }
    }
}

/// An entity's world transform from the local transforms above it.
fn world_of(
    entity: Entity,
    query: &Query<(&Transform, Option<&ChildOf>)>,
) -> Option<GlobalTransform> {
    let (transform, mut parent) = query.get(entity).ok()?;
    let mut world = GlobalTransform::from(*transform);
    while let Some(up) = parent {
        let Ok((transform, next)) = query.get(up.parent()) else {
            break;
        };
        world = GlobalTransform::from(*transform) * world;
        parent = next;
    }
    Some(world)
}
