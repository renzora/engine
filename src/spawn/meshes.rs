//! Mesh entity spawning (cubes, spheres, etc.)

use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};
use crate::shared::{MeshNodeData, MeshPrimitiveType, MeshInstanceData, MeshletMeshData};
use super::{Category, EntityTemplate};

pub static TEMPLATES: &[EntityTemplate] = &[
    EntityTemplate { name: "Cube", category: Category::Mesh, spawn: spawn_cube },
    EntityTemplate { name: "Sphere", category: Category::Mesh, spawn: spawn_sphere },
    EntityTemplate { name: "Cylinder", category: Category::Mesh, spawn: spawn_cylinder },
    EntityTemplate { name: "Plane", category: Category::Mesh, spawn: spawn_plane },
    EntityTemplate { name: "MeshInstance", category: Category::Mesh, spawn: spawn_mesh_instance },
    EntityTemplate { name: "MeshletMesh", category: Category::Mesh, spawn: spawn_meshlet_mesh },
    EntityTemplate { name: "Node3D (Empty)", category: Category::Nodes3D, spawn: spawn_node3d },
];

fn create_standard_material(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.7, 0.7),
        perceptual_roughness: 0.9,
        ..default()
    })
}

pub fn spawn_cube(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Cube, "Cube", parent)
}

pub fn spawn_sphere(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = meshes.add(Sphere::new(0.5).mesh().ico(5).unwrap());
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Sphere, "Sphere", parent)
}

pub fn spawn_cylinder(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = meshes.add(Cylinder::new(0.5, 1.0));
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Cylinder, "Cylinder", parent)
}

pub fn spawn_plane(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = meshes.add(Plane3d::default().mesh().size(2.0, 2.0));
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Plane, "Plane", parent)
}

fn spawn_mesh_entity(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    mesh_type: MeshPrimitiveType,
    name: &str,
    parent: Option<Entity>,
) -> Entity {
    // Note: RaytracingMesh3d is managed by sync_rendering_settings based on Solari state
    let mut entity_commands = commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, 0.5, 0.0),
        Visibility::default(),
        EditorEntity {
            name: name.to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
        MeshNodeData { mesh_type },
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

pub fn spawn_mesh_instance(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "MeshInstance".to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
        MeshInstanceData { model_path: None },
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

/// Spawn an entity configured for meshlet (virtual geometry) rendering.
/// The entity starts with no mesh - assign a .meshlet asset path via the inspector.
/// Meshlet rendering provides GPU-driven LOD and occlusion culling for high-poly meshes.
pub fn spawn_meshlet_mesh(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "MeshletMesh".to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
        MeshletMeshData {
            meshlet_path: String::new(),
            enabled: true,
            lod_bias: 0.0,
        },
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

pub fn spawn_node3d(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "Node3D".to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}
