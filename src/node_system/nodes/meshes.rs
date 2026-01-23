use bevy::prelude::*;
use std::collections::HashMap;

use crate::core::{EditorEntity, SceneNode};
use crate::node_system::components::{MeshInstanceData, MeshNodeData, MeshPrimitiveType, NodeTypeMarker};
use crate::node_system::definition::{NodeCategory, NodeDefinition};

/// Cube mesh primitive
pub static CUBE: NodeDefinition = NodeDefinition {
    type_id: "mesh.cube",
    display_name: "Cube",
    category: NodeCategory::Meshes,
    default_name: "Cube",
    spawn_fn: spawn_cube,
    serialize_fn: Some(serialize_mesh),
    deserialize_fn: Some(deserialize_mesh),
    priority: 0,
};

/// Sphere mesh primitive
pub static SPHERE: NodeDefinition = NodeDefinition {
    type_id: "mesh.sphere",
    display_name: "Sphere",
    category: NodeCategory::Meshes,
    default_name: "Sphere",
    spawn_fn: spawn_sphere,
    serialize_fn: Some(serialize_mesh),
    deserialize_fn: Some(deserialize_mesh),
    priority: 1,
};

/// Cylinder mesh primitive
pub static CYLINDER: NodeDefinition = NodeDefinition {
    type_id: "mesh.cylinder",
    display_name: "Cylinder",
    category: NodeCategory::Meshes,
    default_name: "Cylinder",
    spawn_fn: spawn_cylinder,
    serialize_fn: Some(serialize_mesh),
    deserialize_fn: Some(deserialize_mesh),
    priority: 2,
};

/// Plane mesh primitive
pub static PLANE: NodeDefinition = NodeDefinition {
    type_id: "mesh.plane",
    display_name: "Plane",
    category: NodeCategory::Meshes,
    default_name: "Plane",
    spawn_fn: spawn_plane,
    serialize_fn: Some(serialize_mesh),
    deserialize_fn: Some(deserialize_mesh),
    priority: 3,
};

/// MeshInstance - can have a 3D model file assigned to it
pub static MESH_INSTANCE: NodeDefinition = NodeDefinition {
    type_id: "mesh.instance",
    display_name: "MeshInstance",
    category: NodeCategory::Meshes,
    default_name: "MeshInstance",
    spawn_fn: spawn_mesh_instance,
    serialize_fn: Some(serialize_mesh_instance),
    deserialize_fn: Some(deserialize_mesh_instance),
    priority: 4,
};

/// Create the standard material used for all primitives
fn create_standard_material(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.7, 0.7),
        perceptual_roughness: 0.9,
        ..default()
    })
}

/// Spawn a cube mesh
fn spawn_cube(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let material = create_standard_material(materials);

    spawn_mesh_entity(
        commands,
        mesh,
        material,
        MeshPrimitiveType::Cube,
        CUBE.default_name,
        CUBE.type_id,
        parent,
    )
}

/// Spawn a sphere mesh
fn spawn_sphere(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = meshes.add(Sphere::new(0.5).mesh().ico(5).unwrap());
    let material = create_standard_material(materials);

    spawn_mesh_entity(
        commands,
        mesh,
        material,
        MeshPrimitiveType::Sphere,
        SPHERE.default_name,
        SPHERE.type_id,
        parent,
    )
}

/// Spawn a cylinder mesh
fn spawn_cylinder(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = meshes.add(Cylinder::new(0.5, 1.0));
    let material = create_standard_material(materials);

    spawn_mesh_entity(
        commands,
        mesh,
        material,
        MeshPrimitiveType::Cylinder,
        CYLINDER.default_name,
        CYLINDER.type_id,
        parent,
    )
}

/// Spawn a plane mesh
fn spawn_plane(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = meshes.add(Plane3d::default().mesh().size(2.0, 2.0));
    let material = create_standard_material(materials);

    spawn_mesh_entity(
        commands,
        mesh,
        material,
        MeshPrimitiveType::Plane,
        PLANE.default_name,
        PLANE.type_id,
        parent,
    )
}

/// Common function to spawn a mesh entity with all required components
fn spawn_mesh_entity(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    mesh_type: MeshPrimitiveType,
    name: &str,
    type_id: &'static str,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, 0.5, 0.0),
        Visibility::default(),
        EditorEntity {
            name: name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        NodeTypeMarker::new(type_id),
        MeshNodeData { mesh_type },
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

/// Serialize mesh node data
fn serialize_mesh(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let mesh_data = world.get::<MeshNodeData>(entity)?;
    let mut data = HashMap::new();
    data.insert(
        "mesh_type".to_string(),
        serde_json::to_value(&mesh_data.mesh_type).ok()?,
    );
    Some(data)
}

/// Deserialize mesh node data
fn deserialize_mesh(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    if let Some(mesh_type_value) = data.get("mesh_type") {
        if let Ok(mesh_type) = serde_json::from_value::<MeshPrimitiveType>(mesh_type_value.clone())
        {
            // Create the appropriate mesh
            let mesh = match mesh_type {
                MeshPrimitiveType::Cube => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
                MeshPrimitiveType::Sphere => meshes.add(Sphere::new(0.5).mesh().ico(5).unwrap()),
                MeshPrimitiveType::Cylinder => meshes.add(Cylinder::new(0.5, 1.0)),
                MeshPrimitiveType::Plane => meshes.add(Plane3d::default().mesh().size(2.0, 2.0)),
            };

            let material = materials.add(StandardMaterial {
                base_color: Color::srgb(0.7, 0.7, 0.7),
                perceptual_roughness: 0.9,
                ..default()
            });

            entity_commands.insert((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                MeshNodeData { mesh_type },
            ));
        }
    }
}

/// Spawn an empty mesh instance (no model assigned yet)
fn spawn_mesh_instance(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        Visibility::default(),
        EditorEntity {
            name: MESH_INSTANCE.default_name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        NodeTypeMarker::new(MESH_INSTANCE.type_id),
        MeshInstanceData { model_path: None },
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

/// Serialize mesh instance data
fn serialize_mesh_instance(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let instance_data = world.get::<MeshInstanceData>(entity)?;
    let mut data = HashMap::new();
    data.insert(
        "model_path".to_string(),
        serde_json::to_value(&instance_data.model_path).ok()?,
    );
    Some(data)
}

/// Deserialize mesh instance data
fn deserialize_mesh_instance(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let model_path = data.get("model_path")
        .and_then(|v| serde_json::from_value::<Option<String>>(v.clone()).ok())
        .flatten();

    entity_commands.insert(MeshInstanceData { model_path });
    // Note: The actual model loading will be handled by a separate system
    // that watches for MeshInstanceData changes
}
