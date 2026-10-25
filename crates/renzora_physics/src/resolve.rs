//! Turns each [`CollisionShapeData`] into the [`ResolvedCollider`] a backend
//! builds from.
//!
//! What a shape *means* is decided here, once, rather than by every backend:
//! a cylinder in the 2D world is a circle of the same radius, a 2D mesh
//! collider is its fitted box, and a 3D mesh collider is a trimesh read out of
//! the entity's `Mesh3d` once the asset has loaded.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::camera::visibility::VisibilityRange;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use renzora::physics::{
    PhysicsDim, ResolvedCollider, ResolvedShape, RuntimePhysics, RuntimePhysics2d, TriMeshData,
};

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
        let Some(trimesh) = meshes.get(&mesh_ref.0).and_then(trimesh_from_mesh) else {
            continue;
        };
        insert_trimesh(&mut commands, entity, shape, trimesh);
    }
}

fn insert_trimesh(commands: &mut Commands, entity: Entity, shape: &CollisionShapeData, trimesh: TriMeshData) {
    let resolved = ResolvedCollider {
        rotation: Quat::IDENTITY,
        ..collider(shape, ResolvedShape::TriMesh(Arc::new(trimesh)), Vec3::ZERO)
    };
    commands
        .entity(entity)
        .try_insert(resolved)
        .try_remove::<PendingMeshCollider>();
}

/// Frames a model's mesh tally must hold still before its collider is built.
const SETTLE_FRAMES: u32 = 2;
/// Frames after which a model collider that has built nothing says why.
const STALL_FRAMES: u32 = 600;

/// Where a model-wide collider stands: what its meshes added up to last frame,
/// how long that has held, and what the collider in place was built from.
#[derive(Default)]
pub struct ModelColliderState {
    tally: (usize, usize),
    held: u32,
    built: Option<(usize, usize)>,
    waited: u32,
    warned: bool,
}

type ModelRoot = (
    Entity,
    &'static CollisionShapeData,
    Has<PendingMeshCollider>,
    Has<ResolvedCollider>,
);
type ModelRootFilter = (Without<Mesh3d>, With<RuntimePhysics>, Without<RuntimePhysics2d>);

/// A mesh collider on an entity with no mesh of its own, such as the root of an
/// imported model, collides as every mesh under it merged into one trimesh.
///
/// That is what makes a whole level solid in a way that survives a save: a
/// model's descendants are regenerated from the `.glb` on load and never saved,
/// so colliders put on them are gone the next time the scene opens, which
/// includes every Play, since the game is a child process that loads the file.
///
/// It builds from whatever has loaded once the tally of meshes (found, loaded)
/// has held still for a couple of frames, and builds again whenever the tally
/// moves after that. An earlier version waited for every mesh to load and for
/// the count to settle first, and in a game process it never built at all, so
/// the player fell through the level; a collider with a hole in it is better
/// than none, and the rebuild closes the hole when the rest arrives.
pub fn resolve_model_colliders(
    mut commands: Commands,
    meshes: Option<Res<Assets<Mesh>>>,
    roots: Query<ModelRoot, ModelRootFilter>,
    hierarchy: HierarchyParts,
    mut states: Local<HashMap<Entity, ModelColliderState>>,
) {
    let Some(meshes) = meshes else { return };
    states.retain(|entity, _| {
        roots
            .get(*entity)
            .is_ok_and(|(_, shape, ..)| shape.shape_type == CollisionShapeType::Mesh)
    });
    for (entity, shape, pending, resolved) in &roots {
        if shape.shape_type != CollisionShapeType::Mesh {
            continue;
        }
        let state = states.entry(entity).or_default();
        // A shape edit swaps the collider back to pending; build afresh.
        if pending || !resolved {
            state.built = None;
        }
        let parts = hierarchy.mesh_parts(entity);
        let loaded: Vec<_> = parts.iter().filter(|(handle, _)| meshes.get(handle).is_some()).cloned().collect();
        let tally = (parts.len(), loaded.len());
        if tally == state.tally {
            state.held = state.held.saturating_add(1);
        } else {
            state.tally = tally;
            state.held = 0;
        }
        if state.built.is_none() {
            state.waited = state.waited.saturating_add(1);
            if state.waited > STALL_FRAMES && !state.warned {
                state.warned = true;
                renzora::console_log::console_warn(
                    "Physics",
                    format!(
                        "The mesh collider for {entity} has built nothing yet: {} meshes found, {} loaded",
                        tally.0, tally.1
                    ),
                );
            }
        }
        if loaded.is_empty() || state.held < SETTLE_FRAMES || state.built == Some(tally) {
            continue;
        }
        state.built = Some(tally);
        let Some(trimesh) = merged_trimesh(&meshes, &loaded) else {
            continue;
        };
        // Said each time it builds, because a whole level's collider takes a
        // visible moment and is the first thing to rule out when something
        // falls through it.
        renzora::console_log::console_info(
            "Physics",
            format!(
                "Built a mesh collider for {entity} from {} of {} meshes ({} triangles)",
                tally.1,
                tally.0,
                trimesh.indices.len()
            ),
        );
        insert_trimesh(&mut commands, entity, shape, trimesh);
    }
}

/// The hierarchy queries a model-wide mesh collider walks.
#[derive(SystemParam)]
pub struct HierarchyParts<'w, 's> {
    children: Query<'w, 's, &'static Children>,
    parents: Query<'w, 's, &'static ChildOf>,
    transforms: Query<'w, 's, &'static Transform>,
    parts: Query<
        'w,
        's,
        (&'static Mesh3d, Option<&'static VisibilityRange>),
        Without<CollisionShapeData>,
    >,
}

impl HierarchyParts<'_, '_> {
    /// Every mesh under `root`, with the transform from its space to `root`'s.
    ///
    /// Read from the `Transform` chain rather than `GlobalTransform`, which is
    /// a frame behind for parts spawned this frame and would put them at the
    /// origin. A mesh with a collider of its own is left to it, and so is a
    /// coarser LOD, which starts at a distance and is the same surface again.
    fn mesh_parts(&self, root: Entity) -> Vec<(Handle<Mesh>, Mat4)> {
        let mut out = Vec::new();
        for entity in self.children.iter_descendants(root) {
            let Ok((mesh, range)) = self.parts.get(entity) else {
                continue;
            };
            if range.is_some_and(|r| r.start_margin.end > 0.0) {
                continue;
            }
            let mut to_root = Mat4::IDENTITY;
            let mut at = entity;
            while at != root {
                if let Ok(t) = self.transforms.get(at) {
                    to_root = t.to_matrix() * to_root;
                }
                let Ok(child_of) = self.parents.get(at) else { break };
                at = child_of.parent();
            }
            out.push((mesh.0.clone(), to_root));
        }
        out
    }
}

/// One trimesh out of several meshes, each moved into the root's space.
/// Meshes that are not triangle lists are skipped rather than failing the lot.
fn merged_trimesh(meshes: &Assets<Mesh>, parts: &[(Handle<Mesh>, Mat4)]) -> Option<TriMeshData> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for (handle, to_root) in parts {
        let Some(part) = meshes.get(handle).and_then(trimesh_from_mesh) else {
            continue;
        };
        let base = vertices.len() as u32;
        vertices.extend(part.vertices.iter().map(|v| to_root.transform_point3(*v)));
        indices.extend(part.indices.iter().map(|[a, b, c]| [a + base, b + base, c + base]));
    }
    (!indices.is_empty()).then_some(TriMeshData { vertices, indices })
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
