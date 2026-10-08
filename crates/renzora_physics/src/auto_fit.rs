//! Auto-fit a newly added `CollisionShapeData` to the entity's combined mesh AABB.
//!
//! Marker `PendingAutoFit` is added when the shape is inserted; a system retries
//! each frame until mesh AABBs are available (or the user edits the shape, in
//! which case the marker is removed without changing anything).

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;

use crate::{CollisionShapeData, CollisionShapeType};

/// Marker added on `CollisionShapeData` insert. Removed once auto-fit succeeds
/// or when the user has manually edited the shape values.
#[derive(Component)]
pub struct PendingAutoFit;

/// Opt-out: spawn this alongside a `CollisionShapeData` whose values are
/// already exact (e.g. the tilemap's merged tile colliders). Without it the
/// shape would be tagged `PendingAutoFit` and — having no render AABB to fit
/// to — sit in the retry query forever.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct SkipAutoFit;

/// Tag the entity with `PendingAutoFit` so the next frame tries to size it to mesh.
pub fn mark_new_collision_shapes(
    mut commands: Commands,
    new_shapes: Query<(Entity, &CollisionShapeData), (Added<CollisionShapeData>, Without<SkipAutoFit>)>,
) {
    for (entity, shape) in &new_shapes {
        // Only factory-default shapes auto-fit (a fresh inspector add). A
        // shape arriving with authored values — a scene load, a tilemap object
        // stamp, a terrain chunk — must keep them: `Added` fires for
        // deserialized components too, and fitting would overwrite the very
        // extents the user (or generator) set.
        if *shape != CollisionShapeData::default() {
            continue;
        }
        commands.entity(entity).try_insert(PendingAutoFit);
    }
}

/// Fit the collider half-extents/radius/half-height to the union of mesh AABBs
/// on this entity and its descendants. Runs each frame for entities still
/// carrying `PendingAutoFit`.
pub fn auto_fit_collision_shapes(
    mut commands: Commands,
    pending: Query<(Entity, &CollisionShapeData, &GlobalTransform), With<PendingAutoFit>>,
    aabbs: Query<(&Aabb, &GlobalTransform)>,
    children: Query<&Children>,
) {
    for (entity, shape, entity_gt) in &pending {
        if shape.shape_type == CollisionShapeType::Mesh {
            // Mesh colliders don't have sizable fields — nothing to auto-fit.
            commands.entity(entity).remove::<PendingAutoFit>();
            continue;
        }
        // Bounds are taken in the collider's own local space, corner by corner.
        // Fitting a world-axis box first and mapping that back inflated every
        // rotated collider (a 15 degree Y platform grew to 1.4x1.6 of its mesh).
        let inv = entity_gt.affine().inverse();
        let Some((local_min, local_max)) = combined_local_aabb(entity, inv, &aabbs, &children) else {
            // Mesh not ready yet: try again next frame.
            continue;
        };

        let half = (local_max - local_min) * 0.5;
        let center = (local_max + local_min) * 0.5;

        let mut new_shape = shape.clone();
        new_shape.offset = center;

        match shape.shape_type {
            CollisionShapeType::Box => {
                new_shape.half_extents = half.max(Vec3::splat(0.01));
            }
            CollisionShapeType::Sphere => {
                new_shape.radius = half.max_element().max(0.01);
            }
            CollisionShapeType::Capsule => {
                let r = half.x.max(half.z).max(0.01);
                new_shape.radius = r;
                new_shape.half_height = (half.y - r).max(0.0);
            }
            CollisionShapeType::Cylinder => {
                new_shape.radius = half.x.max(half.z).max(0.01);
                new_shape.half_height = half.y.max(0.01);
            }
            CollisionShapeType::Mesh => unreachable!(),
        }

        // A default-valued shape on a unit mesh already fits; rewriting it only
        // injects float noise (0.5 -> 0.49999997) and dirties a loaded scene.
        if !shape_close(shape, &new_shape) {
            commands.entity(entity).insert(new_shape);
        }
        commands.entity(entity).remove::<PendingAutoFit>();
    }
}

fn shape_close(a: &CollisionShapeData, b: &CollisionShapeData) -> bool {
    const EPS: f32 = 1e-4;
    a.offset.abs_diff_eq(b.offset, EPS)
        && a.half_extents.abs_diff_eq(b.half_extents, EPS)
        && (a.radius - b.radius).abs() <= EPS
        && (a.half_height - b.half_height).abs() <= EPS
}

fn combined_local_aabb(
    root: Entity,
    to_local: bevy::math::Affine3A,
    aabbs: &Query<(&Aabb, &GlobalTransform)>,
    children: &Query<&Children>,
) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;

    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Ok((aabb, gt)) = aabbs.get(e) {
            let center = Vec3::from(aabb.center);
            let half = Vec3::from(aabb.half_extents);
            for sx in [-1.0, 1.0] {
                for sy in [-1.0, 1.0] {
                    for sz in [-1.0, 1.0] {
                        let local = center + half * Vec3::new(sx, sy, sz);
                        let root_local = to_local.transform_point3(gt.transform_point(local));
                        min = min.min(root_local);
                        max = max.max(root_local);
                    }
                }
            }
            found = true;
        }
        if let Ok(kids) = children.get(e) {
            stack.extend(kids.iter());
        }
    }

    if found {
        Some((min, max))
    } else {
        None
    }
}
