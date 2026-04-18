use avian3d::{
    dynamics::rigid_body::sleeping::Sleeping,
    parry::{
        na::{Const, OPoint, Unit, Vector3},
        query::IntersectResult,
        shape::{Polyline, TriMesh, TypedShape},
    },
    prelude::Collider,
};
use bevy::{math::vec3, prelude::*};

use crate::{updater::CachableObstacle, world_to_mesh};

use super::{ObstacleSource, RESOLUTION};

fn try_intersect_trimesh(
    vertices: Vec<OPoint<f32, Const<3>>>,
    indices: Vec<[u32; 3]>,
    up_axis: &Unit<Vector3<f32>>,
    shift: f32,
    intersection_to_navmesh: &impl Fn(IntersectResult<Polyline>) -> Vec<Vec<Vec2>>,
) -> Vec<Vec<Vec2>> {
    match TriMesh::new(vertices, indices) {
        Ok(trimesh) => intersection_to_navmesh(
            trimesh.intersection_with_local_plane(up_axis, shift, f32::EPSILON),
        ),
        Err(e) => {
            warn!("Failed to create TriMesh for NavMesh obstacle generation: {e:?}, skipping");
            vec![]
        }
    }
}

impl ObstacleSource for Collider {
    fn get_polygons(
        &self,
        obstacle_transform: &GlobalTransform,
        navmesh_transform: &Transform,
        up: (Dir3, f32),
    ) -> Vec<Vec<Vec2>> {
        self.shape_scaled()
            .as_typed_shape()
            .get_polygons(obstacle_transform, navmesh_transform, up)
    }
}

trait InnerObstacleSource {
    fn get_polygons(
        &self,
        obstacle_transform: &GlobalTransform,
        navmesh_transform: &Transform,
        up: (Dir3, f32),
    ) -> Vec<Vec<Vec2>>;
}

impl InnerObstacleSource for TypedShape<'_> {
    fn get_polygons(
        &self,
        obstacle_transform: &GlobalTransform,
        navmesh_transform: &Transform,
        (up, shift): (Dir3, f32),
    ) -> Vec<Vec<Vec2>> {
        let mut transform = obstacle_transform.compute_transform();
        transform.scale = Vec3::ONE;
        let world_to_mesh = world_to_mesh(navmesh_transform);

        let to_navmesh =
            |p: OPoint<f32, Const<3>>| world_to_mesh.transform_point(vec3(p.x, p.y, p.z)).xy();

        let intersection_to_navmesh = |intersection: IntersectResult<Polyline>| match intersection {
            IntersectResult::Intersect(i) => i
                .extract_connected_components()
                .iter()
                .map(|p| p.segments().map(|s| s.a).map(to_navmesh).collect())
                .collect(),
            IntersectResult::Negative => vec![],
            IntersectResult::Positive => vec![],
        };

        let d = (-up.x * navmesh_transform.translation.x
            - up.y * navmesh_transform.translation.y
            - up.z * navmesh_transform.translation.z)
            / (up.x.powi(2) + up.y.powi(2) + up.z.powi(2)).sqrt();
        let shift: f32 = shift - d;

        let to_world = |p: &OPoint<f32, Const<3>>| transform.transform_point(vec3(p.x, p.y, p.z));

        let up_axis = Unit::new_normalize(Vector3::new(up.x, up.y, up.z));
        let trimesh_to_world = |vertices: Vec<OPoint<f32, Const<3>>>| {
            vertices
                .iter()
                .map(to_world)
                .map(|v| v.into())
                .collect::<Vec<OPoint<f32, Const<3>>>>()
        };
        match self {
            TypedShape::Cuboid(collider) => {
                let (vertices, indices) = collider.to_trimesh();
                vec![try_intersect_trimesh(trimesh_to_world(vertices), indices, &up_axis, shift, &intersection_to_navmesh)]
            }
            TypedShape::Ball(collider) => {
                let (vertices, indices) = collider.to_trimesh(RESOLUTION, RESOLUTION);
                vec![try_intersect_trimesh(trimesh_to_world(vertices), indices, &up_axis, shift, &intersection_to_navmesh)]
            }
            TypedShape::Capsule(collider) => {
                let (vertices, indices) = collider.to_trimesh(RESOLUTION, RESOLUTION);
                vec![try_intersect_trimesh(trimesh_to_world(vertices), indices, &up_axis, shift, &intersection_to_navmesh)]
            }
            TypedShape::TriMesh(collider) => {
                let vertices = collider.vertices().to_vec();
                let indices = collider.indices().to_vec();
                vec![try_intersect_trimesh(trimesh_to_world(vertices), indices, &up_axis, shift, &intersection_to_navmesh)]
            }
            TypedShape::HeightField(collider) => {
                let (vertices, indices) = collider.to_trimesh();
                vec![try_intersect_trimesh(trimesh_to_world(vertices), indices, &up_axis, shift, &intersection_to_navmesh)]
            }
            TypedShape::Compound(collider) => {
                collider
                    .shapes()
                    .iter()
                    .map(|(iso, shape)| {
                        let shape_transform = Transform::from_translation(
                            Vec3::new(iso.translation.x, iso.translation.y, iso.translation.z),
                        )
                        .with_rotation(Quat::from_xyzw(
                            iso.rotation.i,
                            iso.rotation.j,
                            iso.rotation.k,
                            iso.rotation.w,
                        ));
                        let composed = GlobalTransform::from(
                            obstacle_transform.compute_transform() * shape_transform,
                        );
                        shape
                            .as_typed_shape()
                            .get_polygons(&composed, navmesh_transform, (up, shift))
                    })
                    .collect()
            }
            TypedShape::ConvexPolyhedron(collider) => {
                let (vertices, indices) = collider.to_trimesh();
                vec![try_intersect_trimesh(trimesh_to_world(vertices), indices, &up_axis, shift, &intersection_to_navmesh)]
            }
            TypedShape::Cylinder(collider) => {
                let (vertices, indices) = collider.to_trimesh(RESOLUTION);
                vec![try_intersect_trimesh(trimesh_to_world(vertices), indices, &up_axis, shift, &intersection_to_navmesh)]
            }
            TypedShape::Cone(collider) => {
                let (vertices, indices) = collider.to_trimesh(RESOLUTION);
                vec![try_intersect_trimesh(trimesh_to_world(vertices), indices, &up_axis, shift, &intersection_to_navmesh)]
            }
            TypedShape::RoundCuboid(collider) => {
                let (vertices, indices) = collider.inner_shape.to_trimesh();
                vec![try_intersect_trimesh(trimesh_to_world(vertices), indices, &up_axis, shift, &intersection_to_navmesh)]
            }
            TypedShape::RoundCylinder(collider) => {
                let (vertices, indices) = collider.inner_shape.to_trimesh(RESOLUTION);
                vec![try_intersect_trimesh(trimesh_to_world(vertices), indices, &up_axis, shift, &intersection_to_navmesh)]
            }
            TypedShape::RoundCone(collider) => {
                let (vertices, indices) = collider.inner_shape.to_trimesh(RESOLUTION);
                vec![try_intersect_trimesh(trimesh_to_world(vertices), indices, &up_axis, shift, &intersection_to_navmesh)]
            }
            TypedShape::RoundConvexPolyhedron(collider) => {
                let (vertices, indices) = collider.inner_shape.to_trimesh();
                vec![try_intersect_trimesh(trimesh_to_world(vertices), indices, &up_axis, shift, &intersection_to_navmesh)]
            }
            TypedShape::Segment(_) => {
                warn!("Segment collider not supported for NavMesh obstacle generation");
                vec![]
            }
            TypedShape::Triangle(_) => {
                warn!("Triangle collider not supported for NavMesh obstacle generation");
                vec![]
            }
            TypedShape::Polyline(_) => {
                warn!("Polyline collider not supported for NavMesh obstacle generation");
                vec![]
            }
            TypedShape::HalfSpace(_) => {
                warn!("HalfSpace collider not supported for NavMesh obstacle generation");
                vec![]
            }
            TypedShape::RoundTriangle(_) => {
                warn!("RoundTriangle collider not supported for NavMesh obstacle generation");
                vec![]
            }
            TypedShape::Custom(_) => {
                warn!("Custom collider not supported for NavMesh obstacle generation");
                vec![]
            }
            TypedShape::Voxels(_) => {
                warn!("Voxels collider not supported for NavMesh obstacle generation");
                vec![]
            }
        }
        .into_iter()
        .flatten()
        .collect()
    }
}

pub fn on_sleeping_inserted(trigger: On<Insert, Sleeping>, mut commands: Commands) {
    commands
        .entity(trigger.event().entity)
        .try_insert(CachableObstacle);
}

pub fn on_sleeping_removed(trigger: On<Remove, Sleeping>, mut commands: Commands) {
    commands
        .entity(trigger.event().entity)
        .try_remove::<CachableObstacle>();
}
