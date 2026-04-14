//! Auto-fit a newly added `CollisionShapeData` to the entity's combined mesh AABB.
//!
//! Marker `PendingAutoFit` is added when the shape is inserted; a system retries
//! each frame until mesh AABBs are available (or the user edits the shape, in
//! which case the marker is removed without changing anything).

use bevy::prelude::*;
use bevy::camera::primitives::Aabb;

use crate::{CollisionShapeData, CollisionShapeType};

/// Marker added on `CollisionShapeData` insert. Removed once auto-fit succeeds
/// or when the user has manually edited the shape values.
#[derive(Component)]
pub struct PendingAutoFit;

/// Tag the entity with `PendingAutoFit` so the next frame tries to size it to mesh.
pub fn mark_new_collision_shapes(
    mut commands: Commands,
    new_shapes: Query<Entity, Added<CollisionShapeData>>,
) {
    for entity in &new_shapes {
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
        let Some((min, max)) = combined_world_aabb(entity, &aabbs, &children) else {
            // Mesh not ready yet — try again next frame.
            continue;
        };

        let inv = entity_gt.affine().inverse();
        let corners = [
            Vec3::new(min.x, min.y, min.z), Vec3::new(max.x, min.y, min.z),
            Vec3::new(min.x, max.y, min.z), Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z), Vec3::new(max.x, min.y, max.z),
            Vec3::new(min.x, max.y, max.z), Vec3::new(max.x, max.y, max.z),
        ];

        let mut local_min = Vec3::splat(f32::INFINITY);
        let mut local_max = Vec3::splat(f32::NEG_INFINITY);
        for c in corners {
            let lp = inv.transform_point3(c);
            local_min = local_min.min(lp);
            local_max = local_max.max(lp);
        }

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

        commands.entity(entity).insert(new_shape);
        commands.entity(entity).remove::<PendingAutoFit>();
    }
}

fn combined_world_aabb(
    root: Entity,
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
                        let world = gt.transform_point(local);
                        min = min.min(world);
                        max = max.max(world);
                    }
                }
            }
            found = true;
        }
        if let Ok(kids) = children.get(e) {
            stack.extend(kids.iter());
        }
    }

    if found { Some((min, max)) } else { None }
}
