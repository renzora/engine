//! One-time bone -> physics-body generation, triggered when a `Ragdoll`
//! marker is added to a skeleton root.
//!
//! The skeleton's bone set comes from the nearest descendant `SkinnedMesh`'s
//! `joints` list (the same source of truth `renzora_viewport`'s model
//! flattener uses to protect joint entities from collapsing) rather than
//! walking `Name`d entities, since not every named child of the root is a
//! bone.

use crate::{Ragdoll, RagdollBone};
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use renzora::physics::{
    CollisionShapeData, JointKind, PhysicsBodyData, PhysicsJoint, RuntimeOnlyPhysics,
};
use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;

/// Collider radius as a fraction of the skeleton's average bone length.
const BONE_RADIUS_FRACTION: f32 = 0.18;
/// Floor for leaf-bone (hands, feet, head tip, ...) sphere colliders.
const MIN_LEAF_RADIUS: f32 = 0.03;
/// Compliance (m/N) at `Ragdoll::stiffness == 0.0` — loose enough to feel
/// floppy without the joint visibly falling apart. `stiffness == 1.0` maps
/// to `0.0` (perfectly rigid).
const MAX_JOINT_COMPLIANCE: f32 = 0.001;
/// Bone mass per cubic metre of collider. One, as avian's default collider
/// density was when it computed the bones' masses itself: the joint compliance
/// above was tuned against those (very light) bones.
const BONE_DENSITY: f32 = 1.0;
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
            let (shape, volume) = bone_collider(kids, &transforms, avg_bone_len);

            commands.entity(bone).insert((
                RagdollBone,
                // Generated, so a scene save leaves the bone's body and collider
                // out and the next load builds them again.
                RuntimeOnlyPhysics,
                PhysicsBodyData {
                    mass: (volume * BONE_DENSITY).max(1e-4),
                    linear_damping: ragdoll.linear_damping,
                    angular_damping: ragdoll.angular_damping,
                    gravity_scale: ragdoll.gravity_scale,
                    ..PhysicsBodyData::kinematic_body()
                },
                shape,
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
                    PhysicsJoint::new(
                        parent,
                        kid,
                        JointKind::Spherical {
                            twist_axis: Vec3::Y,
                            swing_limit: Some(swing),
                            twist_limit: Some(twist),
                        },
                    )
                    .with_anchors(kid_transform.translation, Vec3::ZERO)
                    .with_compliance(compliance),
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
/// a small sphere for leaf bones (no in-skeleton children), with its volume.
///
/// The capsule is the contract's upright one, laid along the bone by turning
/// it and moving its centre to the bone's midpoint.
fn bone_collider(
    kids: Option<&Vec<Entity>>,
    transforms: &Query<&Transform>,
    avg_bone_len: f32,
) -> (CollisionShapeData, f32) {
    let radius = (avg_bone_len * BONE_RADIUS_FRACTION).max(MIN_LEAF_RADIUS);
    let sphere = |r: f32| (CollisionShapeData::sphere(r), 4.0 / 3.0 * PI * r.powi(3));

    let Some(kids) = kids.filter(|k| !k.is_empty()) else {
        return sphere(radius);
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
        return sphere(radius);
    }
    let tip = sum / count as f32;
    let length = tip.length();
    if length < radius * 2.0 {
        return sphere(radius);
    }
    let radius = radius.min(length * 0.5);
    let half_height = length * 0.5;
    let shape = CollisionShapeData {
        offset: tip * 0.5,
        rotation: Quat::from_rotation_arc(Vec3::Y, tip / length),
        ..CollisionShapeData::capsule(radius, half_height)
    };
    let volume = PI * radius * radius * length + 4.0 / 3.0 * PI * radius.powi(3);
    (shape, volume)
}
