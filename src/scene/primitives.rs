use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};

pub enum PrimitiveType {
    Cube,
    Sphere,
    Cylinder,
    Plane,
}

pub fn spawn_primitive(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    primitive_type: PrimitiveType,
    name: &str,
    parent: Option<Entity>,
) {
    let mesh = match primitive_type {
        PrimitiveType::Cube => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        PrimitiveType::Sphere => meshes.add(Sphere::new(0.5).mesh().ico(5).unwrap()),
        PrimitiveType::Cylinder => meshes.add(Cylinder::new(0.5, 1.0)),
        PrimitiveType::Plane => meshes.add(Plane3d::default().mesh().size(2.0, 2.0)),
    };

    let mut entity_commands = commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.7, 0.7),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.5, 0.0),
        Visibility::default(),
        EditorEntity {
            name: name.to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }
}
