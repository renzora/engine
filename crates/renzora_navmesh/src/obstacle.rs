//! Navmesh obstacles from the colliders the physics API resolved.
//!
//! An obstacle carves the outline of its collider where that collider crosses
//! the navmesh's plane. This used to come from avian's own `Collider`, through
//! the navigator's avian integration, which tied the navmesh to one physics
//! engine. Reading `ResolvedCollider` instead works under any backend, or none:
//! the collider geometry exists whether or not anything simulates it.
//!
//! The slice is done here, on a tessellation of the shape. A convex shape's
//! cross-section is the hull of where its edges cross the plane, which is exact
//! and immune to the order the triangles come in. A triangle mesh may be
//! concave, so its crossing segments are chained into loops instead.

use std::collections::HashMap;

use bevy::mesh::{Indices, Meshable, VertexAttributeValues};
use bevy::prelude::*;
use renzora::physics::{ResolvedCollider, ResolvedShape};
use vleue_navigator::prelude::ObstacleSource;

use crate::NavMeshObstacle;

/// Segments around a round shape. The navigator's own primitives use 32.
const RESOLUTION: u32 = 32;

/// The collider an obstacle carves with, mirrored from its `ResolvedCollider`.
/// A component of this crate's own because the navigator's updater needs a
/// component implementing its trait, and neither the trait nor
/// `ResolvedCollider` is ours to join.
#[derive(Component, Clone, Debug)]
pub struct ColliderObstacle(pub ResolvedCollider);

/// Keep [`ColliderObstacle`] in step with the collider of every entity marked
/// [`NavMeshObstacle`].
pub(crate) fn mirror_collider_obstacles(
    mut commands: Commands,
    changed: Query<
        (Entity, &ResolvedCollider),
        (With<NavMeshObstacle>, Or<(Changed<ResolvedCollider>, Added<NavMeshObstacle>)>),
    >,
    mut lost_collider: RemovedComponents<ResolvedCollider>,
    mut lost_marker: RemovedComponents<NavMeshObstacle>,
    still: Query<(), (With<ResolvedCollider>, With<NavMeshObstacle>)>,
) {
    for (entity, collider) in &changed {
        commands.entity(entity).try_insert(ColliderObstacle(collider.clone()));
    }
    for entity in lost_collider.read().chain(lost_marker.read()) {
        if !still.contains(entity) {
            if let Ok(mut e) = commands.get_entity(entity) {
                e.try_remove::<ColliderObstacle>();
            }
        }
    }
}

impl ObstacleSource for ColliderObstacle {
    fn get_polygons(
        &self,
        obstacle_transform: &GlobalTransform,
        navmesh_transform: &Transform,
        (up, shift): (Dir3, f32),
    ) -> Vec<Vec<Vec2>> {
        let Some((vertices, triangles, convex)) = tessellate(&self.0) else {
            return Vec::new();
        };
        let to_world = obstacle_transform.affine();
        let world: Vec<Vec3> = vertices.iter().map(|v| to_world.transform_point3(*v)).collect();

        // The plane the navmesh sits on, written as `up · p = bias`, exactly as
        // the navigator's own obstacle sources place it.
        let up = up.as_vec3();
        let bias = shift + up.dot(navmesh_transform.translation);
        let to_mesh = navmesh_transform.compute_affine().inverse();
        let flat = |p: Vec3| to_mesh.transform_point3(p).xy();

        let segments = slice(&world, &triangles, up, bias);
        if segments.is_empty() {
            return Vec::new();
        }
        if convex {
            let points: Vec<Vec2> = segments.iter().flat_map(|(a, b)| [flat(*a), flat(*b)]).collect();
            let hull = convex_hull(points);
            return if hull.len() >= 3 { vec![hull] } else { Vec::new() };
        }
        chain(&segments)
            .into_iter()
            .map(|ring| ring.into_iter().map(flat).collect::<Vec<_>>())
            .filter(|ring| ring.len() >= 3)
            .collect()
    }
}

/// The shape as triangles in the entity's local space, and whether it is
/// convex. `None` for a 2D shape, which has no extent off its plane.
fn tessellate(collider: &ResolvedCollider) -> Option<(Vec<Vec3>, Vec<[u32; 3]>, bool)> {
    let mesh = match &collider.shape {
        ResolvedShape::Cuboid { half_extents } => Cuboid::from_size(*half_extents * 2.0).mesh().build(),
        ResolvedShape::Sphere { radius } => Sphere::new(*radius).mesh().uv(RESOLUTION, RESOLUTION / 2),
        ResolvedShape::Capsule { radius, half_height } => Capsule3d::new(*radius, half_height * 2.0)
            .mesh()
            .longitudes(RESOLUTION)
            .build(),
        ResolvedShape::Cylinder { radius, half_height } => Cylinder::new(*radius, half_height * 2.0)
            .mesh()
            .resolution(RESOLUTION)
            .build(),
        ResolvedShape::TriMesh(mesh) => {
            let place = |v: &Vec3| collider.offset + collider.rotation * *v;
            return Some((mesh.vertices.iter().map(place).collect(), mesh.indices.clone(), false));
        }
        ResolvedShape::Rectangle { .. } | ResolvedShape::Circle { .. } | ResolvedShape::Capsule2d { .. } => {
            return None
        }
    };
    let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION)? {
        VertexAttributeValues::Float32x3(p) => p,
        _ => return None,
    };
    let vertices = positions
        .iter()
        .map(|p| collider.offset + collider.rotation * Vec3::from_array(*p))
        .collect();
    let indices: Vec<u32> = match mesh.indices()? {
        Indices::U32(i) => i.clone(),
        Indices::U16(i) => i.iter().map(|&x| x as u32).collect(),
    };
    let triangles = indices.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    Some((vertices, triangles, true))
}

/// Where each triangle crosses the plane `up · p = bias`, as segments.
fn slice(vertices: &[Vec3], triangles: &[[u32; 3]], up: Vec3, bias: f32) -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    for tri in triangles {
        let Some(p) = tri.map(|i| vertices.get(i as usize).copied()).into_iter().collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let d = [up.dot(p[0]) - bias, up.dot(p[1]) - bias, up.dot(p[2]) - bias];
        let mut crossing = Vec::with_capacity(2);
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            // Strictly opposite sides, or one end on the plane: a vertex lying
            // exactly on it is counted by the edge leaving it, once.
            if (d[a] < 0.0) != (d[b] < 0.0) {
                let t = d[a] / (d[a] - d[b]);
                crossing.push(p[a].lerp(p[b], t));
            }
        }
        if let [a, b] = crossing[..] {
            if a.distance_squared(b) > 1e-12 {
                out.push((a, b));
            }
        }
    }
    out
}

/// Join slice segments into rings by their shared endpoints.
fn chain(segments: &[(Vec3, Vec3)]) -> Vec<Vec<Vec3>> {
    // Endpoints meet exactly in theory and to within rounding in practice.
    let key = |p: Vec3| ((p.x * 1e4).round() as i64, (p.y * 1e4).round() as i64, (p.z * 1e4).round() as i64);
    let mut from: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for (i, (a, b)) in segments.iter().enumerate() {
        from.entry(key(*a)).or_default().push(i);
        from.entry(key(*b)).or_default().push(i);
    }
    let mut used = vec![false; segments.len()];
    let mut rings = Vec::new();
    for start in 0..segments.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let (first, mut at) = segments[start];
        let mut ring = vec![first, at];
        loop {
            let next = from
                .get(&key(at))
                .and_then(|list| list.iter().copied().find(|&i| !used[i]));
            let Some(i) = next else { break };
            used[i] = true;
            let (a, b) = segments[i];
            at = if key(a) == key(at) { b } else { a };
            if key(at) == key(first) {
                break;
            }
            ring.push(at);
        }
        rings.push(ring);
    }
    rings
}

/// Andrew's monotone chain: the convex hull, counter-clockwise.
fn convex_hull(mut points: Vec<Vec2>) -> Vec<Vec2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup_by(|a, b| a.distance_squared(*b) < 1e-10);
    if points.len() < 3 {
        return points;
    }
    let cross = |o: Vec2, a: Vec2, b: Vec2| (a - o).perp_dot(b - o);
    let mut hull: Vec<Vec2> = Vec::with_capacity(points.len() * 2);
    for pass in 0..2 {
        let start = hull.len();
        let iter: Box<dyn Iterator<Item = &Vec2>> =
            if pass == 0 { Box::new(points.iter()) } else { Box::new(points.iter().rev()) };
        for &p in iter {
            while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn area(ring: &[Vec2]) -> f32 {
        let n = ring.len();
        (0..n).map(|i| ring[i].perp_dot(ring[(i + 1) % n])).sum::<f32>().abs() * 0.5
    }

    /// The navmesh lies flat (rotated so its local XY is the world's XZ, as
    /// `on_volume_added` does) and a 2 x 2 box straddles it: the hole is the
    /// box's 2 x 2 footprint.
    #[test]
    fn a_box_carves_its_footprint() {
        let obstacle = ColliderObstacle(ResolvedCollider::new(ResolvedShape::Cuboid { half_extents: Vec3::ONE }));
        let navmesh = Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
        let polys = obstacle.get_polygons(&GlobalTransform::IDENTITY, &navmesh, (Dir3::Y, 0.0));
        assert_eq!(polys.len(), 1);
        assert!((area(&polys[0]) - 4.0).abs() < 1e-3, "{:?}", polys[0]);
    }

    #[test]
    fn a_box_above_the_plane_carves_nothing() {
        let obstacle = ColliderObstacle(ResolvedCollider::new(ResolvedShape::Cuboid { half_extents: Vec3::ONE }));
        let navmesh = Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
        let up_high = GlobalTransform::from_translation(Vec3::Y * 5.0);
        assert!(obstacle.get_polygons(&up_high, &navmesh, (Dir3::Y, 0.0)).is_empty());
    }

    /// An L-shaped prism is concave; its slice must keep the notch rather than
    /// be filled in to a hull.
    #[test]
    fn a_concave_mesh_keeps_its_notch() {
        // L footprint in XZ: a 2 x 2 square missing its 1 x 1 corner, area 3.
        let outline = [
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 2.0),
            Vec2::new(0.0, 2.0),
        ];
        let mut vertices = Vec::new();
        for y in [-1.0, 1.0] {
            vertices.extend(outline.iter().map(|p| Vec3::new(p.x, y, p.y)));
        }
        let n = outline.len() as u32;
        let indices = (0..n)
            .flat_map(|i| {
                let j = (i + 1) % n;
                [[i, j, i + n], [j, j + n, i + n]]
            })
            .collect();
        let shape = ResolvedShape::TriMesh(Arc::new(renzora::physics::TriMeshData { vertices, indices }));
        let obstacle = ColliderObstacle(ResolvedCollider::new(shape));
        let navmesh = Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
        let polys = obstacle.get_polygons(&GlobalTransform::IDENTITY, &navmesh, (Dir3::Y, 0.0));
        assert_eq!(polys.len(), 1);
        assert!((area(&polys[0]) - 3.0).abs() < 1e-3, "{:?}", polys[0]);
    }
}
