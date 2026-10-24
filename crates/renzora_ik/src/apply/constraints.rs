//! Copy constraints, two-bone IK, look-at and rotation limits.

use super::target_point;
use crate::components::{CopyPosition, CopyRotation, LookAt, RotationLimit, TwoBoneIk};
use crate::names::NameLookup;
use crate::pose::{track, Pose, PoseCache};
use crate::solve::{aim, limit, two_bone};
use bevy::math::EulerRot;
use bevy::prelude::*;

/// The rotation a [`RotationLimit`] is measured from: the bone's local
/// rotation when the limit was added. Kept off the reflected component
/// because it is the bone's, not the author's, and a scene should not carry a
/// value that silently goes stale when the model is re-imported.
#[derive(Component, Clone, Copy, Debug)]
pub struct LimitReference(pub Quat);

pub fn capture_limit_reference(trigger: On<Add<RotationLimit>>, transforms: Query<&Transform>, mut commands: Commands) {
    let entity = trigger.entity;
    if let Ok(t) = transforms.get(entity) {
        commands.entity(entity).try_insert(LimitReference(t.rotation));
    }
}

/// Shared parameters of every constraint system here.
type Lookups<'w, 's> = (Res<'w, NameLookup>, Query<'w, 's, &'static Name>, Query<'w, 's, &'static ChildOf>);

fn axis_mask(source: Vec3, current: Vec3, mask: [bool; 3]) -> Vec3 {
    Vec3::new(
        if mask[0] { source.x } else { current.x },
        if mask[1] { source.y } else { current.y },
        if mask[2] { source.z } else { current.z },
    )
}

pub fn apply_copy_constraints(
    mut commands: Commands,
    (lookup, names, parents): Lookups,
    positions: Query<(Entity, &CopyPosition)>,
    rotations: Query<(Entity, &CopyRotation)>,
    mut transforms: Query<&mut Transform>,
    cached: Query<(), With<PoseCache>>,
) {
    let mut pose = Pose { transforms: &mut transforms, parents: &parents };
    for (entity, c) in &positions {
        let w = c.weight.clamp(0.0, 1.0);
        if w <= 0.0 {
            continue;
        }
        let Some(source) = lookup.find(&c.source, entity, &names, &parents).and_then(|s| pose.world(s)) else {
            continue;
        };
        let Some(own) = pose.world(entity) else {
            continue;
        };
        if let Some(local) = pose.local(entity) {
            track(&mut commands, &cached, entity, local);
        }
        let wanted = axis_mask(source.translation + c.offset, own.translation, [c.x, c.y, c.z]);
        pose.set_world_translation(entity, own.translation.lerp(wanted, w));
    }
    for (entity, c) in &rotations {
        let w = c.weight.clamp(0.0, 1.0);
        if w <= 0.0 {
            continue;
        }
        let Some(source) = lookup.find(&c.source, entity, &names, &parents).and_then(|s| pose.world(s)) else {
            continue;
        };
        let Some(own) = pose.world(entity) else {
            continue;
        };
        if let Some(local) = pose.local(entity) {
            track(&mut commands, &cached, entity, local);
        }
        // The whole rotation when every axis is copied: going through Euler
        // angles would add nothing but their singularities.
        let copied = if c.x && c.y && c.z {
            source.rotation
        } else {
            let (sx, sy, sz) = source.rotation.to_euler(EulerRot::XYZ);
            let (ox, oy, oz) = own.rotation.to_euler(EulerRot::XYZ);
            let e = axis_mask(Vec3::new(sx, sy, sz), Vec3::new(ox, oy, oz), [c.x, c.y, c.z]);
            Quat::from_euler(EulerRot::XYZ, e.x, e.y, e.z)
        };
        let offset = Quat::from_euler(
            EulerRot::XYZ,
            c.offset.x.to_radians(),
            c.offset.y.to_radians(),
            c.offset.z.to_radians(),
        );
        pose.set_world_rotation(entity, own.rotation.slerp(offset * copied, w));
    }
}

pub fn apply_two_bone_ik(
    mut commands: Commands,
    (lookup, names, parents): Lookups,
    iks: Query<(Entity, &TwoBoneIk)>,
    mut transforms: Query<&mut Transform>,
    cached: Query<(), With<PoseCache>>,
) {
    let mut pose = Pose { transforms: &mut transforms, parents: &parents };
    for (end, ik) in &iks {
        let w = ik.weight.clamp(0.0, 1.0);
        if w <= 0.0 {
            continue;
        }
        let Some(mid) = pose.parent(end) else {
            continue;
        };
        let Some(root) = pose.parent(mid) else {
            continue;
        };
        let (Some(wr), Some(wm), Some(we)) = (pose.world(root), pose.world(mid), pose.world(end)) else {
            continue;
        };
        let (target, target_world) = target_point(&ik.target, ik.target_position, end, &lookup, &names, &pose);
        let pole = match lookup.find(&ik.pole_target, end, &names, &parents).and_then(|p| pose.world(p)) {
            Some(p) => Some(p.translation),
            None if ik.pole_direction.length_squared() > 1e-8 => {
                let frame = pose.parent_world(root);
                Some(wr.translation + frame.rotation * ik.pole_direction)
            }
            None => None,
        };

        for bone in [root, mid, end] {
            if let Some(local) = pose.local(bone) {
                track(&mut commands, &cached, bone, local);
            }
        }

        let (upper, lower) = two_bone::solve(wr.translation, wm.translation, we.translation, target, pole);
        pose.set_world_rotation(root, wr.rotation.slerp(upper * wr.rotation, w));
        pose.set_world_rotation(mid, wm.rotation.slerp(lower * wm.rotation, w));
        let end_rotation = match target_world {
            Some(t) if ik.match_target_rotation => Some(we.rotation.slerp(t.rotation, w)),
            _ if ik.keep_end_rotation => Some(we.rotation),
            _ => None,
        };
        if let Some(r) = end_rotation {
            pose.set_world_rotation(end, r);
        }
    }
}

pub fn apply_look_at(
    mut commands: Commands,
    (lookup, names, parents): Lookups,
    looks: Query<(Entity, &LookAt)>,
    mut transforms: Query<&mut Transform>,
    cached: Query<(), With<PoseCache>>,
) {
    let mut pose = Pose { transforms: &mut transforms, parents: &parents };
    for (bone, look) in &looks {
        let w = look.weight.clamp(0.0, 1.0);
        if w <= 0.0 {
            continue;
        }
        let Some(world) = pose.world(bone) else {
            continue;
        };
        let (target, _) = target_point(&look.target, look.target_position, bone, &lookup, &names, &pose);
        if let Some(local) = pose.local(bone) {
            track(&mut commands, &cached, bone, local);
        }
        let aimed = aim::aim(
            world.rotation,
            world.translation,
            target,
            look.aim_axis,
            look.up_axis,
            look.world_up,
            look.max_angle.to_radians(),
        );
        pose.set_world_rotation(bone, world.rotation.slerp(aimed, w));
    }
}

pub fn apply_rotation_limits(
    mut commands: Commands,
    mut bones: Query<(Entity, &RotationLimit, &LimitReference, &mut Transform)>,
    cached: Query<(), With<PoseCache>>,
) {
    for (bone, lim, reference, mut transform) in &mut bones {
        let w = lim.weight.clamp(0.0, 1.0);
        if w <= 0.0 {
            continue;
        }
        let clamped = limit::clamp(
            transform.rotation,
            reference.0,
            lim.twist_axis,
            lim.swing_limit.to_radians(),
            lim.twist_min.to_radians(),
            lim.twist_max.to_radians(),
        );
        let out = transform.rotation.slerp(clamped, w);
        // Within limits the answer is the input: leave the transform (and its
        // change detection) alone.
        if out.dot(transform.rotation).abs() < 1.0 - 1e-7 {
            track(&mut commands, &cached, bone, *transform);
            transform.rotation = out;
        }
    }
}
