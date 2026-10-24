//! Turns each [`CollisionShapeData`] into the [`ResolvedCollider`] a backend
//! builds from.
//!
//! What a shape *means* is decided here, once, rather than by every backend:
//! a cylinder in the 2D world is a circle of the same radius, a 2D mesh
//! collider is its fitted box, and a 3D mesh collider is a trimesh read out of
//! the entity's `Mesh3d` once the asset has loaded.

use std::sync::Arc;

use bevy::prelude::*;
use renzora::physics::{PhysicsDim, ResolvedCollider, ResolvedShape, TriMeshData};

use crate::data::{CollisionShapeData, CollisionShapeType};

/// Marker: this entity has `CollisionShapeData { shape_type: Mesh }` but its
/// `Mesh3d` has not loaded yet, so there is no geometry to resolve.
#[derive(Component)]
pub struct PendingMeshCollider;

/// The collider for `shape` in `dim`, or `None` for a 3D mesh collider, whose
/// geometry has to wait for its asset (see [`resolve_mesh_colliders`]).
pub fn resolve(shape: &CollisionShapeData, dim: PhysicsDim) -> Option<ResolvedCollider> {
    let resolved = match dim {
        PhysicsDim::D3 => match shape.shape_type {
            CollisionShapeType::Box => ResolvedShape::Cuboid { half_extents: shape.half_extents },
            CollisionShapeType::Sphere => ResolvedShape::Sphere { radius: shape.radius },
            CollisionShapeType::Capsule => ResolvedShape::Capsule {
                radius: shape.radius,
                half_height: shape.half_height,
            },
            CollisionShapeType::Cylinder => ResolvedShape::Cylinder {
                radius: shape.radius,
                half_height: shape.half_height,
            },
            CollisionShapeType::Mesh => return None,
        },
        PhysicsDim::D2 => match shape.shape_type {
            CollisionShapeType::Box => ResolvedShape::Rectangle {
                half_extents: shape.half_extents.truncate(),
            },
            CollisionShapeType::Sphere => ResolvedShape::Circle { radius: shape.radius },
            CollisionShapeType::Capsule => ResolvedShape::Capsule2d {
                radius: shape.radius,
                half_height: shape.half_height,
            },
            // No cylinder in a plane; its silhouette is a circle of the same radius.
            CollisionShapeType::Cylinder => ResolvedShape::Circle { radius: shape.radius },
            // Trimesh-from-Mesh3d has no 2D source geometry; use the fitted box.
            CollisionShapeType::Mesh => ResolvedShape::Rectangle {
                half_extents: shape.half_extents.truncate(),
            },
        },
    };
    Some(collider(shape, resolved, shape.offset))
}

fn collider(shape: &CollisionShapeData, resolved: ResolvedShape, offset: Vec3) -> ResolvedCollider {
    ResolvedCollider {
        shape: resolved,
        offset,
        rotation: shape.rotation,
        friction: shape.friction,
        restitution: shape.restitution,
        sensor: shape.is_sensor,
        layers: shape.layers(),
    }
}

/// Insert `entity`'s resolved collider, or mark it pending until its mesh loads.
pub fn insert_resolved(
    commands: &mut Commands,
    entity: Entity,
    shape: &CollisionShapeData,
    dim: PhysicsDim,
) {
    match resolve(shape, dim) {
        Some(resolved) => {
            commands
                .entity(entity)
                .try_insert(resolved)
                .try_remove::<PendingMeshCollider>();
        }
        None => {
            commands
                .entity(entity)
                .try_remove::<ResolvedCollider>()
                .try_insert(PendingMeshCollider);
        }
    }
}

/// Build the trimesh for every pending mesh collider whose `Mesh3d` has loaded.
/// Runs every frame; removes the marker on success.
///
/// The offset and rotation are left at zero: a trimesh's vertices are already in
/// the entity's local space, and the fitted offset auto-fit writes describes the
/// mesh's box, not a displacement of it.
pub fn resolve_mesh_colliders(
    mut commands: Commands,
    meshes: Option<Res<Assets<Mesh>>>,
    pending: Query<(Entity, &Mesh3d, &CollisionShapeData), With<PendingMeshCollider>>,
) {
    let Some(meshes) = meshes else { return };
    for (entity, mesh_ref, shape) in &pending {
        let Some(mesh) = meshes.get(&mesh_ref.0) else {
            continue;
        };
        let Some(trimesh) = trimesh_from_mesh(mesh) else {
            continue;
        };
        let resolved = ResolvedCollider {
            rotation: Quat::IDENTITY,
            ..collider(shape, ResolvedShape::TriMesh(Arc::new(trimesh)), Vec3::ZERO)
        };
        commands
            .entity(entity)
            .try_insert(resolved)
            .try_remove::<PendingMeshCollider>();
    }
}

/// Triangle soup from a Bevy `Mesh` (TriangleList only).
pub fn trimesh_from_mesh(mesh: &Mesh) -> Option<TriMeshData> {
    use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
    if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
        return None;
    }
    let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION)? {
        VertexAttributeValues::Float32x3(v) => v,
        _ => return None,
    };
    let vertices: Vec<Vec3> = positions
        .iter()
        .map(|p| Vec3::new(p[0], p[1], p[2]))
        .collect();
    let indices: Vec<[u32; 3]> = match mesh.indices()? {
        Indices::U32(idx) => idx.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
        Indices::U16(idx) => idx
            .chunks_exact(3)
            .map(|c| [c[0] as u32, c[1] as u32, c[2] as u32])
            .collect(),
    };
    if indices.is_empty() {
        return None;
    }
    Some(TriMeshData { vertices, indices })
}
