//! One-time bone -> physics-body generation, triggered when a `Ragdoll`
//! marker is added to a skeleton root.
//!
//! The skeleton's bone set comes from the nearest descendant `SkinnedMesh`'s
//! `joints` list (the same source of truth `renzora_viewport`'s model
//! flattener uses to protect joint entities from collapsing) rather than
//! walking `Name`d entities, since not every named child of the root is a
//! bone.

use crate::{Ragdoll, RagdollBone};
use avian3d::prelude::*;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

/// Collider radius as a fraction of the skeleton's average bone length.
const BONE_RADIUS_FRACTION: f32 = 0.18;
/// Floor for leaf-bone (hands, feet, head tip, ...) sphere colliders.
const MIN_LEAF_RADIUS: f32 = 0.03;
/// Compliance (m/N) at `Ragdoll::stiffness == 0.0` — loose enough to feel
/// floppy without the joint visibly falling apart. `stiffness == 1.0` maps
/// to `0.0` (avian's default — perfectly rigid).
const MAX_JOINT_COMPLIANCE: f32 = 0.001;
/// How many frames to keep polling for the skinned mesh before giving up. A
/// `Ragdoll` loaded from a scene races the async GLB instantiation that spawns
/// the skeleton, so the mesh usually isn't present for the first few frames;
/// 600 (~10s @ 60 Hz) is far more than any load needs, and only a genuinely
/// unskinned target ever reaches it.
const SKELETON_WAIT_FRAMES: u32 = 600;

/// Marker inserted on a `Ragdoll` root once its bone bodies have been generated,
/// so [`build_ragdolls`] builds exactly once even though it polls every frame.
#[derive(Component)]
pub struct RagdollBuilt;

pub fn build_ragdolls(
    mut commands: Commands,
    // Poll every frame (not just on `Added`): the skeleton is instantiated a few
    // frames after the `Ragdoll` component loads, so bailing on frame one would
    // permanently skip a scene-loaded ragdoll. `RagdollBuilt` stops the poll once
    // bodies exist.
    pending: Query<(Entity, &Ragdoll), Without<RagdollBuilt>>,
    skinned: Query<&SkinnedMesh>,
    children: Query<&Children>,
    transforms: Query<&Transform>,
    mut waited: Local<HashMap<Entity, u32>>,
) {
    for (root, ragdoll) in &pending {
        let Some(joints) = find_skinned_joints(root, &skinned, &children) else {
            // Skeleton not spawned yet — retry next frame, but bound the wait so a
            // truly unskinned target warns once instead of polling forever.
            let n = waited.entry(root).or_insert(0);
            *n += 1;
            if *n >= SKELETON_WAIT_FRAMES {
                warn!("Ragdoll on {root:?}: no SkinnedMesh found in its subtree after {SKELETON_WAIT_FRAMES} frames, giving up");
                commands.entity(root).insert(RagdollBuilt);
                waited.remove(&root);
            }
            continue;
        };
        if joints.is_empty() {
            continue;
        }
        let joint_set: HashSet<Entity> = joints.iter().copied().collect();

        // Bone -> its in-skeleton children (a bone authored as a glTF node
        // can have non-bone children too, e.g. attachment sockets; those are
        // filtered out by the `joint_set` membership check).
        let mut children_of: HashMap<Entity, Vec<Entity>> = HashMap::new();
        for &bone in &joints {
            let Ok(kids) = children.get(bone) else {
                continue;
            };
            for kid in kids.iter() {
                if joint_set.contains(&kid) {
                    children_of.entry(bone).or_default().push(kid);
                }
            }
        }

        let avg_bone_len = average_bone_length(&children_of, &transforms);

        for &bone in &joints {
            let kids = children_of.get(&bone);
            let collider = bone_collider(kids, &transforms, avg_bone_len);

            commands.entity(bone).insert((
                RagdollBone,
                RigidBody::Kinematic,
                collider,
                LinearDamping(ragdoll.linear_damping),
                AngularDamping(ragdoll.angular_damping),
                GravityScale(ragdoll.gravity_scale),
            ));
        }

        // Only swing/twist resist bending — point compliance stays rigid so
        // limbs never visibly pull apart at the joint regardless of `stiffness`.
        let compliance = (1.0 - ragdoll.stiffness.clamp(0.0, 1.0)) * MAX_JOINT_COMPLIANCE;
        let swing = ragdoll.swing_limit_degrees.to_radians();
        let twist = ragdoll.twist_limit_degrees.to_radians();

        for (&parent, kids) in &children_of {
            for &kid in kids {
                let Ok(kid_transform) = transforms.get(kid) else {
                    continue;
                };
                // The bind-pose offset of the child bone, in the parent's
                // local space, is exactly where the two bones should stay
                // pinned together — using it as both anchors keeps the
                // ragdoll in its animated pose the instant it activates.
                commands.spawn(
                    SphericalJoint::new(parent, kid)
                        .with_local_anchor1(kid_transform.translation)
                        .with_local_anchor2(Vec3::ZERO)
                        .with_swing_limits(-swing, swing)
                        .with_twist_limits(-twist, twist)
                        .with_swing_compliance(compliance)
                        .with_twist_compliance(compliance),
                );
            }
        }

        // Built once — stop polling this root. (Activation of an already-`active`
        // ragdoll is left to `toggle::apply_ragdoll_state`, which fires when the
        // flag is next set/toggled, by which point the skeleton is animation-ready.)
        waited.remove(&root);
        commands.entity(root).insert(RagdollBuilt);
        info!("Ragdoll on {root:?}: generated {} bone bodies", joints.len());
    }
}

/// Depth-first search from `root` for the first `SkinnedMesh`, returning its
/// joint entities. `None` if no skinned mesh exists under `root`.
fn find_skinned_joints(
    root: Entity,
    skinned: &Query<&SkinnedMesh>,
    children: &Query<&Children>,
) -> Option<Vec<Entity>> {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Ok(mesh) = skinned.get(entity) {
            return Some(mesh.joints.clone());
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
    None
}

fn average_bone_length(
    children_of: &HashMap<Entity, Vec<Entity>>,
    transforms: &Query<&Transform>,
) -> f32 {
    let mut total = 0.0f32;
    let mut count = 0u32;
    for kids in children_of.values() {
        for &kid in kids {
            if let Ok(t) = transforms.get(kid) {
                total += t.translation.length();
                count += 1;
            }
        }
    }
    if count > 0 {
        total / count as f32
    } else {
        0.2
    }
}

/// Capsule spanning this bone's origin to its (averaged) child position, or
/// a small sphere for leaf bones (no in-skeleton children).
fn bone_collider(
    kids: Option<&Vec<Entity>>,
    transforms: &Query<&Transform>,
    avg_bone_len: f32,
) -> Collider {
    let radius = (avg_bone_len * BONE_RADIUS_FRACTION).max(MIN_LEAF_RADIUS);

    let Some(kids) = kids.filter(|k| !k.is_empty()) else {
        return Collider::sphere(radius);
    };

    let mut sum = Vec3::ZERO;
    let mut count = 0u32;
    for &kid in kids {
        if let Ok(t) = transforms.get(kid) {
            sum += t.translation;
            count += 1;
        }
    }
    if count == 0 {
        return Collider::sphere(radius);
    }
    let tip = sum / count as f32;
    let length = tip.length();
    if length < radius * 2.0 {
        return Collider::sphere(radius);
    }
    Collider::capsule_endpoints(radius.min(length * 0.5), Vec3::ZERO, tip)
}
