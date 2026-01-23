//! Physics body and collision shape nodes

use bevy::prelude::*;
use std::collections::HashMap;

use crate::core::{EditorEntity, SceneNode};
use crate::node_system::components::{
    CollisionShapeData, CollisionShapeType, NodeTypeMarker, PhysicsBodyData, PhysicsBodyType,
};
use crate::node_system::definition::{NodeCategory, NodeDefinition};

// ============================================================================
// Physics Body Nodes
// ============================================================================

/// RigidBody3D - Dynamic physics body affected by forces
pub static RIGIDBODY3D: NodeDefinition = NodeDefinition {
    type_id: "physics.rigidbody3d",
    display_name: "RigidBody3D",
    category: NodeCategory::Physics,
    default_name: "RigidBody3D",
    spawn_fn: spawn_rigidbody,
    serialize_fn: Some(serialize_physics_body),
    deserialize_fn: Some(deserialize_physics_body),
    priority: 0,
};

/// StaticBody3D - Fixed collision body that never moves
pub static STATICBODY3D: NodeDefinition = NodeDefinition {
    type_id: "physics.staticbody3d",
    display_name: "StaticBody3D",
    category: NodeCategory::Physics,
    default_name: "StaticBody3D",
    spawn_fn: spawn_staticbody,
    serialize_fn: Some(serialize_physics_body),
    deserialize_fn: Some(deserialize_physics_body),
    priority: 1,
};

/// KinematicBody3D - Programmatically controlled body
pub static KINEMATICBODY3D: NodeDefinition = NodeDefinition {
    type_id: "physics.kinematicbody3d",
    display_name: "KinematicBody3D",
    category: NodeCategory::Physics,
    default_name: "KinematicBody3D",
    spawn_fn: spawn_kinematicbody,
    serialize_fn: Some(serialize_physics_body),
    deserialize_fn: Some(deserialize_physics_body),
    priority: 2,
};

// ============================================================================
// Collision Shape Nodes
// ============================================================================

/// BoxShape3D - Box collision shape
pub static COLLISION_BOX: NodeDefinition = NodeDefinition {
    type_id: "physics.collision_box",
    display_name: "CollisionShape3D (Box)",
    category: NodeCategory::Physics,
    default_name: "BoxShape3D",
    spawn_fn: spawn_collision_box,
    serialize_fn: Some(serialize_collision_shape),
    deserialize_fn: Some(deserialize_collision_shape),
    priority: 10,
};

/// SphereShape3D - Sphere collision shape
pub static COLLISION_SPHERE: NodeDefinition = NodeDefinition {
    type_id: "physics.collision_sphere",
    display_name: "CollisionShape3D (Sphere)",
    category: NodeCategory::Physics,
    default_name: "SphereShape3D",
    spawn_fn: spawn_collision_sphere,
    serialize_fn: Some(serialize_collision_shape),
    deserialize_fn: Some(deserialize_collision_shape),
    priority: 11,
};

/// CapsuleShape3D - Capsule collision shape
pub static COLLISION_CAPSULE: NodeDefinition = NodeDefinition {
    type_id: "physics.collision_capsule",
    display_name: "CollisionShape3D (Capsule)",
    category: NodeCategory::Physics,
    default_name: "CapsuleShape3D",
    spawn_fn: spawn_collision_capsule,
    serialize_fn: Some(serialize_collision_shape),
    deserialize_fn: Some(deserialize_collision_shape),
    priority: 12,
};

/// CylinderShape3D - Cylinder collision shape
pub static COLLISION_CYLINDER: NodeDefinition = NodeDefinition {
    type_id: "physics.collision_cylinder",
    display_name: "CollisionShape3D (Cylinder)",
    category: NodeCategory::Physics,
    default_name: "CylinderShape3D",
    spawn_fn: spawn_collision_cylinder,
    serialize_fn: Some(serialize_collision_shape),
    deserialize_fn: Some(deserialize_collision_shape),
    priority: 13,
};

// ============================================================================
// Spawn Functions - Physics Bodies
// ============================================================================

fn spawn_rigidbody(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_physics_body(
        commands,
        PhysicsBodyData::default(),
        RIGIDBODY3D.default_name,
        RIGIDBODY3D.type_id,
        parent,
    )
}

fn spawn_staticbody(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_physics_body(
        commands,
        PhysicsBodyData::static_body(),
        STATICBODY3D.default_name,
        STATICBODY3D.type_id,
        parent,
    )
}

fn spawn_kinematicbody(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_physics_body(
        commands,
        PhysicsBodyData::kinematic_body(),
        KINEMATICBODY3D.default_name,
        KINEMATICBODY3D.type_id,
        parent,
    )
}

fn spawn_physics_body(
    commands: &mut Commands,
    body_data: PhysicsBodyData,
    name: &str,
    type_id: &'static str,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        NodeTypeMarker::new(type_id),
        body_data,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

// ============================================================================
// Spawn Functions - Collision Shapes
// ============================================================================

fn spawn_collision_box(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_collision_shape(
        commands,
        CollisionShapeData::default(),
        COLLISION_BOX.default_name,
        COLLISION_BOX.type_id,
        parent,
    )
}

fn spawn_collision_sphere(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_collision_shape(
        commands,
        CollisionShapeData::sphere(0.5),
        COLLISION_SPHERE.default_name,
        COLLISION_SPHERE.type_id,
        parent,
    )
}

fn spawn_collision_capsule(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_collision_shape(
        commands,
        CollisionShapeData::capsule(0.5, 0.5),
        COLLISION_CAPSULE.default_name,
        COLLISION_CAPSULE.type_id,
        parent,
    )
}

fn spawn_collision_cylinder(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_collision_shape(
        commands,
        CollisionShapeData::cylinder(0.5, 0.5),
        COLLISION_CYLINDER.default_name,
        COLLISION_CYLINDER.type_id,
        parent,
    )
}

fn spawn_collision_shape(
    commands: &mut Commands,
    shape_data: CollisionShapeData,
    name: &str,
    type_id: &'static str,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        NodeTypeMarker::new(type_id),
        shape_data,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

// ============================================================================
// Serialization
// ============================================================================

fn serialize_physics_body(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let body_data = world.get::<PhysicsBodyData>(entity)?;
    let mut data = HashMap::new();
    data.insert(
        "body_type".to_string(),
        serde_json::to_value(&body_data.body_type).ok()?,
    );
    data.insert("mass".to_string(), serde_json::to_value(body_data.mass).ok()?);
    data.insert(
        "gravity_scale".to_string(),
        serde_json::to_value(body_data.gravity_scale).ok()?,
    );
    data.insert(
        "linear_damping".to_string(),
        serde_json::to_value(body_data.linear_damping).ok()?,
    );
    data.insert(
        "angular_damping".to_string(),
        serde_json::to_value(body_data.angular_damping).ok()?,
    );
    data.insert(
        "lock_rotation_x".to_string(),
        serde_json::to_value(body_data.lock_rotation_x).ok()?,
    );
    data.insert(
        "lock_rotation_y".to_string(),
        serde_json::to_value(body_data.lock_rotation_y).ok()?,
    );
    data.insert(
        "lock_rotation_z".to_string(),
        serde_json::to_value(body_data.lock_rotation_z).ok()?,
    );
    data.insert(
        "lock_translation_x".to_string(),
        serde_json::to_value(body_data.lock_translation_x).ok()?,
    );
    data.insert(
        "lock_translation_y".to_string(),
        serde_json::to_value(body_data.lock_translation_y).ok()?,
    );
    data.insert(
        "lock_translation_z".to_string(),
        serde_json::to_value(body_data.lock_translation_z).ok()?,
    );
    Some(data)
}

fn deserialize_physics_body(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let body_type = data
        .get("body_type")
        .and_then(|v| serde_json::from_value::<PhysicsBodyType>(v.clone()).ok())
        .unwrap_or_default();

    let body_data = PhysicsBodyData {
        body_type,
        mass: data
            .get("mass")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0) as f32,
        gravity_scale: data
            .get("gravity_scale")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0) as f32,
        linear_damping: data
            .get("linear_damping")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32,
        angular_damping: data
            .get("angular_damping")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.05) as f32,
        lock_rotation_x: data
            .get("lock_rotation_x")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        lock_rotation_y: data
            .get("lock_rotation_y")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        lock_rotation_z: data
            .get("lock_rotation_z")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        lock_translation_x: data
            .get("lock_translation_x")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        lock_translation_y: data
            .get("lock_translation_y")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        lock_translation_z: data
            .get("lock_translation_z")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    };

    entity_commands.insert(body_data);
}

fn serialize_collision_shape(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let shape_data = world.get::<CollisionShapeData>(entity)?;
    let mut data = HashMap::new();
    data.insert(
        "shape_type".to_string(),
        serde_json::to_value(&shape_data.shape_type).ok()?,
    );
    data.insert(
        "half_extents".to_string(),
        serde_json::to_value([shape_data.half_extents.x, shape_data.half_extents.y, shape_data.half_extents.z]).ok()?,
    );
    data.insert("radius".to_string(), serde_json::to_value(shape_data.radius).ok()?);
    data.insert(
        "half_height".to_string(),
        serde_json::to_value(shape_data.half_height).ok()?,
    );
    data.insert("friction".to_string(), serde_json::to_value(shape_data.friction).ok()?);
    data.insert(
        "restitution".to_string(),
        serde_json::to_value(shape_data.restitution).ok()?,
    );
    data.insert("is_sensor".to_string(), serde_json::to_value(shape_data.is_sensor).ok()?);
    Some(data)
}

fn deserialize_collision_shape(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let shape_type = data
        .get("shape_type")
        .and_then(|v| serde_json::from_value::<CollisionShapeType>(v.clone()).ok())
        .unwrap_or_default();

    let half_extents = data
        .get("half_extents")
        .and_then(|v| serde_json::from_value::<[f32; 3]>(v.clone()).ok())
        .map(|arr| Vec3::new(arr[0], arr[1], arr[2]))
        .unwrap_or(Vec3::splat(0.5));

    let shape_data = CollisionShapeData {
        shape_type,
        half_extents,
        radius: data.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
        half_height: data.get("half_height").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
        friction: data.get("friction").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
        restitution: data.get("restitution").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        is_sensor: data.get("is_sensor").and_then(|v| v.as_bool()).unwrap_or(false),
    };

    entity_commands.insert(shape_data);
}
