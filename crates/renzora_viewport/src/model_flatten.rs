//! Flatten pass-through nodes in spawned GLTF scenes.
//!
//! GLTF/FBX imports often produce chains like `character.glb → SceneRoot →
//! GltfNode0 → Mesh` where the intermediate entities add no information — they
//! have identity (or safely-composable) transforms, a single child, and no
//! mesh/light/camera/joint role. Keeping them confuses the gizmo (ambiguous
//! selection) and the animator (unclear where to attach).
//!
//! We walk each newly-spawned scene subtree and fold pass-through nodes into
//! their child, composing transforms as we go. Nodes referenced as skin
//! joints, or whose transform would introduce shear when composed, are
//! preserved.
//!
//! For large scenes with wide sibling fan-out (e.g. the Bistro scene with
//! hundreds of separate meshes), nothing collapses at the fan-out level —
//! only vertical single-child chains fold.

use bevy::prelude::*;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::scene::{SceneInstance, SceneRoot};
use std::collections::HashSet;

/// Marker on the top-level entity of an imported model. The gizmo and
/// animation tooling use this as the default "grab the whole thing" target.
#[derive(Component, Debug, Clone, Copy)]
pub struct ImportedRoot;

/// Marker placed on a `SceneRoot` entity that still needs flattening once
/// Bevy's scene spawner has populated its descendants. Removed by
/// `flatten_pending_scenes` once processed.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PendingFlatten {
    /// Frames we've waited for the scene to populate. Gives up after a cap
    /// to avoid sticking around forever on broken scenes.
    pub frames_waited: u32,
}

const MAX_WAIT_FRAMES: u32 = 30;

/// Angle tolerance (radians) below which a rotation is considered "no rotation"
/// for shear-safety purposes.
const ROTATION_EPSILON: f32 = 1e-4;

/// Scale uniformity tolerance.
const SCALE_EPSILON: f32 = 1e-4;

/// Returns `true` if composing `parent * child` is guaranteed shear-free given
/// Bevy's `Transform` (TRS) representation.
fn safe_to_compose(parent: &Transform) -> bool {
    let has_rotation = parent.rotation.to_axis_angle().1.abs() > ROTATION_EPSILON;
    let s = parent.scale;
    let uniform_scale = (s.x - s.y).abs() < SCALE_EPSILON && (s.y - s.z).abs() < SCALE_EPSILON;

    // Safe if: no rotation (any scale), OR uniform scale (any rotation).
    !has_rotation || uniform_scale
}

/// Returns `true` if `entity` plays a visible/functional role that disqualifies
/// it from being collapsed. We check for mesh/light/camera/skinned-mesh
/// components. Joint entities are filtered separately via the joint set.
fn has_scene_role(entity: Entity, world: &World) -> bool {
    let e = match world.get_entity(entity) {
        Ok(e) => e,
        Err(_) => return false,
    };
    e.contains::<Mesh3d>()
        || e.contains::<SkinnedMesh>()
        || e.contains::<PointLight>()
        || e.contains::<DirectionalLight>()
        || e.contains::<SpotLight>()
        || e.contains::<Camera3d>()
        || e.contains::<Camera>()
}

/// Collect all entities referenced as skin joints anywhere in the subtree
/// rooted at `root`. These must not be collapsed — their identity is load-
/// bearing for skinning and animation retargeting.
fn collect_joint_entities(root: Entity, world: &World) -> HashSet<Entity> {
    let mut joints = HashSet::new();
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Ok(e) = world.get_entity(entity) {
            if let Some(skinned) = e.get::<SkinnedMesh>() {
                joints.extend(skinned.joints.iter().copied());
            }
            if let Some(children) = e.get::<Children>() {
                stack.extend(children.iter());
            }
        }
    }
    joints
}

/// Walk every descendant of `root` and return them in bottom-up order so
/// leaves are processed before their parents.
fn collect_descendants_postorder(root: Entity, world: &World) -> Vec<Entity> {
    let mut result = Vec::new();
    let mut stack = vec![(root, false)];
    while let Some((entity, visited)) = stack.pop() {
        if visited {
            result.push(entity);
            continue;
        }
        stack.push((entity, true));
        if let Ok(e) = world.get_entity(entity) {
            if let Some(children) = e.get::<Children>() {
                for child in children.iter() {
                    stack.push((child, false));
                }
            }
        }
    }
    result
}

/// Core flatten pass — run against the `SceneRoot` entity once its subtree is
/// populated. Collapses pass-through chains inside the subtree. The
/// `SceneRoot` entity itself is always collapsed into its sole child if
/// eligible, which removes the `SceneRoot → GltfNode0 → Mesh` noise.
fn flatten_subtree(scene_root: Entity, world: &mut World) {
    let joint_set = collect_joint_entities(scene_root, world);

    // Process bottom-up so we don't invalidate iteration.
    let entities = collect_descendants_postorder(scene_root, world);

    for entity in entities {
        // Don't collapse the scene_root entity itself from this pass — the
        // outer system handles it (different parenting path). Also skip
        // already-despawned entities.
        if entity == scene_root {
            continue;
        }
        if world.get_entity(entity).is_err() {
            continue;
        }
        try_collapse_into_child(entity, world, &joint_set);
    }

    // Finally, try to collapse the SceneRoot itself into its sole child.
    try_collapse_into_child(scene_root, world, &joint_set);
}

/// If `entity` is a pass-through node (single child, safe transform, no role,
/// not a joint), reparent its child directly under `entity`'s parent with the
/// composed transform, then despawn `entity`.
fn try_collapse_into_child(
    entity: Entity,
    world: &mut World,
    joint_set: &HashSet<Entity>,
) {
    if joint_set.contains(&entity) {
        return;
    }
    if has_scene_role(entity, world) {
        return;
    }

    // Must have exactly one child.
    let child = {
        let Ok(e) = world.get_entity(entity) else { return };
        let Some(children) = e.get::<Children>() else { return };
        if children.len() != 1 {
            return;
        }
        children[0]
    };

    // Parent transform must be safe to compose.
    let parent_transform = match world.get::<Transform>(entity) {
        Some(t) => *t,
        None => return,
    };
    if !safe_to_compose(&parent_transform) {
        return;
    }

    // Find `entity`'s parent (may be None — entity is a root).
    let grandparent = world.get::<ChildOf>(entity).map(|c| c.parent());

    // Compose: child_world_local = parent_local * child_local.
    let child_transform = match world.get::<Transform>(child) {
        Some(t) => *t,
        None => Transform::default(),
    };
    let composed = parent_transform * child_transform;

    // Preserve the collapsed entity's Name if the child has none or a generic one.
    let maybe_name = world.get::<Name>(entity).cloned();
    let child_has_useful_name = world
        .get::<Name>(child)
        .map(|n| {
            let s = n.as_str();
            !s.is_empty() && s != "GltfNode0" && !s.starts_with("GltfNode")
        })
        .unwrap_or(false);

    // Reparent the child.
    if let Some(gp) = grandparent {
        world.entity_mut(child).insert(ChildOf(gp));
    } else {
        world.entity_mut(child).remove::<ChildOf>();
    }

    if let Some(mut t) = world.get_mut::<Transform>(child) {
        *t = composed;
    } else {
        world.entity_mut(child).insert(composed);
    }

    if let Some(name) = maybe_name {
        if !child_has_useful_name {
            world.entity_mut(child).insert(name);
        }
    }

    // If the collapsed entity carried `ImportedRoot`, promote the child.
    if world.get::<ImportedRoot>(entity).is_some() {
        world.entity_mut(child).insert(ImportedRoot);
    }

    let entity_name = world
        .get::<Name>(entity)
        .map(|n| n.as_str().to_string())
        .unwrap_or_else(|| "<no name>".into());
    let child_name = world
        .get::<Name>(child)
        .map(|n| n.as_str().to_string())
        .unwrap_or_else(|| "<no name>".into());

    // Strip bevy_scene bookkeeping before despawn so the scene spawner
    // doesn't hold stale references.
    let mut entity_mut = world.entity_mut(entity);
    entity_mut.remove::<SceneRoot>();
    entity_mut.remove::<SceneInstance>();
    entity_mut.despawn();

    let child_parent_after = world
        .get::<ChildOf>(child)
        .map(|c| c.parent());
    info!(
        "[flatten] collapsed {:?}({}) into child {:?}({}); child.parent={:?}, grandparent_was={:?}",
        entity, entity_name, child, child_name, child_parent_after, grandparent
    );
}

/// System: find `SceneRoot` entities tagged with `PendingFlatten` and flatten
/// them once their subtree has been populated by Bevy's scene spawner.
pub fn flatten_pending_scenes(world: &mut World) {
    // Find candidates whose subtree is ready (has at least one child).
    let mut pending: Vec<Entity> = world
        .query_filtered::<Entity, With<PendingFlatten>>()
        .iter(world)
        .collect();

    pending.retain(|&e| world.get_entity(e).is_ok());

    for entity in pending {
        let has_children = world
            .get::<Children>(entity)
            .map(|c| !c.is_empty())
            .unwrap_or(false);

        let frames = world
            .get::<PendingFlatten>(entity)
            .map(|p| p.frames_waited)
            .unwrap_or(0);

        if !has_children {
            if frames >= MAX_WAIT_FRAMES {
                // Give up — remove the marker so we don't spin forever.
                world.entity_mut(entity).remove::<PendingFlatten>();
                continue;
            }
            if let Some(mut p) = world.get_mut::<PendingFlatten>(entity) {
                p.frames_waited = frames + 1;
            }
            continue;
        }

        // Remove marker before flattening — the entity may be despawned.
        world.entity_mut(entity).remove::<PendingFlatten>();
        let before = count_descendants(entity, world);
        flatten_subtree(entity, world);
        let parent = world.get::<ChildOf>(entity).map(|c| c.parent());
        info!(
            "[flatten] scene_root={:?} descendants_before={} parent_after={:?}",
            entity, before, parent
        );
    }
}

fn count_descendants(root: Entity, world: &World) -> usize {
    let mut count = 0;
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        count += 1;
        if let Ok(er) = world.get_entity(e) {
            if let Some(children) = er.get::<Children>() {
                stack.extend(children.iter());
            }
        }
    }
    count
}

// ── GLTF wrapper hiding ───────────────────────────────────────────────────

/// Marker placed on an `ImportedRoot` once its GLTF wrapper descendants have
/// been processed by `hide_gltf_wrappers`. Once present, the system skips the
/// root — wrapper names don't change for the life of the entity.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct WrappersHidden;

/// Per-frame wait counter for `hide_gltf_wrappers`, kept on the root until
/// the scene finishes spawning. Cleared when children appear and the actual
/// hiding pass runs.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct WrappersPending {
    pub frames_waited: u32,
}

/// Returns true for entity names that come from Bevy's GLTF spawner or the
/// drop pipeline as plumbing rather than user-meaningful content.
///
/// Examples that match: `SceneRoot` (spawned by `model_drop`), `RootNode`,
/// `RootNode.001` (Blender-exported), `RootNode_2` (some GLTF tooling),
/// `Scene` (Blender's default scene).
fn is_gltf_wrapper_name(name: &str) -> bool {
    if name == "SceneRoot" || name == "RootNode" || name == "Scene" {
        return true;
    }
    let suffix_only_digits = |sep: char| {
        name.split_once(sep).map_or(false, |(prefix, rest)| {
            prefix == "RootNode" && !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
        })
    };
    suffix_only_digits('.') || suffix_only_digits('_')
}

/// System: walk each newly-imported model's subtree and tag GLTF wrapper
/// nodes with `HideInHierarchy` so they don't clutter the hierarchy panel.
/// The hierarchy panel skips hidden entities and re-parents their visible
/// children to the nearest visible ancestor — so the dropped model shows
/// `ModelRoot → Mesh1, Mesh2, ...` instead of
/// `ModelRoot → SceneRoot → RootNode.001 → Mesh1, Mesh2, ...`.
///
/// Waits up to `MAX_WAIT_FRAMES` for the spawned scene to populate before
/// stamping `WrappersHidden`. Once stamped, won't re-process — wrapper
/// names are stable for the life of the entity.
pub fn hide_gltf_wrappers(
    mut commands: Commands,
    pending: Query<
        (Entity, Option<&Children>, Option<&WrappersPending>),
        // Use `MeshInstanceData` rather than `ImportedRoot` so rehydrated
        // scenes loaded from disk get the same treatment — `ImportedRoot`
        // is a runtime-only marker that isn't serialized.
        (With<renzora::MeshInstanceData>, Without<WrappersHidden>),
    >,
    name_query: Query<&Name>,
    children_query: Query<&Children>,
    hidden_query: Query<(), With<renzora::HideInHierarchy>>,
) {
    for (root, root_children, pending_marker) in pending.iter() {
        let has_children = root_children.map(|c| !c.is_empty()).unwrap_or(false);
        if !has_children {
            let frames = pending_marker.map(|p| p.frames_waited).unwrap_or(0);
            if frames >= MAX_WAIT_FRAMES {
                commands.entity(root).try_insert(WrappersHidden);
                commands.entity(root).remove::<WrappersPending>();
            } else {
                commands.entity(root).try_insert(WrappersPending {
                    frames_waited: frames + 1,
                });
            }
            continue;
        }

        // Walk descendants (skip the root itself — its name is the file name
        // and the user expects to see it). Use `try_insert` so the command
        // no-ops if an entity is despawned between this frame and command
        // application — `flatten_pending_scenes` runs in the same set and
        // collapses some of these wrappers, so a wrapper we tagged here may
        // be gone by the time the insert applies.
        let mut stack: Vec<Entity> = root_children
            .map(|c| c.iter().collect())
            .unwrap_or_default();
        while let Some(entity) = stack.pop() {
            if let Ok(kids) = children_query.get(entity) {
                stack.extend(kids.iter());
            }
            if hidden_query.get(entity).is_ok() {
                continue;
            }
            let Ok(name) = name_query.get(entity) else { continue };
            if is_gltf_wrapper_name(name.as_str()) {
                commands.entity(entity).try_insert(renzora::HideInHierarchy);
            }
        }

        commands.entity(root).try_insert(WrappersHidden);
        commands.entity(root).remove::<WrappersPending>();
    }
}
