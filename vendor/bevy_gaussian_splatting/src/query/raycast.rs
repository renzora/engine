use bevy::{
    mesh::{Indices, Mesh3d, PrimitiveTopology},
    prelude::*,
};

#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Point {
    pub position: Vec3,
}

#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct InsideMesh;

#[derive(Default)]
pub struct RaycastSelectionPlugin;

impl Plugin for RaycastSelectionPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Point>();
        app.register_type::<InsideMesh>();
        app.add_systems(Update, point_in_mesh_system);
    }
}

struct Triangle {
    vertices: [Vec3; 3],
}

fn point_in_mesh_system(
    mesh_query: Query<(&Mesh3d, &GlobalTransform)>,
    point_query: Query<(Entity, &Point), Without<InsideMesh>>,
    meshes: Res<Assets<Mesh>>,
    mut commands: Commands,
) {
    for (mesh_handle, transform) in mesh_query.iter() {
        let Some(mesh) = meshes.get(&mesh_handle.0) else {
            continue;
        };

        for (entity, point) in point_query.iter() {
            let local_point = transform
                .to_matrix()
                .inverse()
                .transform_point3(point.position);
            if is_point_in_mesh(local_point, mesh) {
                commands.entity(entity).insert(InsideMesh);
            }
        }
    }
}

fn is_point_in_mesh(point: Vec3, mesh: &Mesh) -> bool {
    if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
        return false;
    }

    let Some(vertex_attribute) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        return false;
    };
    let Some(vertices) = vertex_attribute.as_float3() else {
        return false;
    };

    let Some(indices) = mesh.indices() else {
        return false;
    };
    let Indices::U32(indices) = indices else {
        return false;
    };

    let mut intersections = 0usize;
    for chunk in indices.chunks_exact(3) {
        let triangle = Triangle {
            vertices: [
                Vec3::from(vertices[chunk[0] as usize]),
                Vec3::from(vertices[chunk[1] as usize]),
                Vec3::from(vertices[chunk[2] as usize]),
            ],
        };

        let ray_direction = Vec3::new(1.0, 0.0, 0.0);
        if ray_intersects_triangle(point, ray_direction, &triangle) {
            intersections += 1;
        }
    }

    intersections % 2 == 1
}

fn ray_intersects_triangle(ray_origin: Vec3, ray_direction: Vec3, triangle: &Triangle) -> bool {
    let epsilon = 0.000_001;
    let vertex0 = triangle.vertices[0];
    let vertex1 = triangle.vertices[1];
    let vertex2 = triangle.vertices[2];

    let edge1 = vertex1 - vertex0;
    let edge2 = vertex2 - vertex0;
    let h = ray_direction.cross(edge2);
    let a = edge1.dot(h);

    if a > -epsilon && a < epsilon {
        return false;
    }

    let f = 1.0 / a;
    let s = ray_origin - vertex0;
    let u = f * s.dot(h);

    if !(0.0..=1.0).contains(&u) {
        return false;
    }

    let q = s.cross(edge1);
    let v = f * ray_direction.dot(q);

    if v < 0.0 || (u + v) > 1.0 {
        return false;
    }

    let t = f * edge2.dot(q);
    t > epsilon
}
