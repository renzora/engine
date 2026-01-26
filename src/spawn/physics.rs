//! Physics body and collision shape spawning

use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};
use crate::shared::{PhysicsBodyData, CollisionShapeData};
use super::{Category, EntityTemplate};

pub static TEMPLATES: &[EntityTemplate] = &[
    // Physics bodies
    EntityTemplate { name: "RigidBody3D", category: Category::Physics, spawn: spawn_rigidbody },
    EntityTemplate { name: "StaticBody3D", category: Category::Physics, spawn: spawn_staticbody },
    EntityTemplate { name: "KinematicBody3D", category: Category::Physics, spawn: spawn_kinematicbody },
    // Collision shapes
    EntityTemplate { name: "BoxShape3D", category: Category::Physics, spawn: spawn_collision_box },
    EntityTemplate { name: "SphereShape3D", category: Category::Physics, spawn: spawn_collision_sphere },
    EntityTemplate { name: "CapsuleShape3D", category: Category::Physics, spawn: spawn_collision_capsule },
    EntityTemplate { name: "CylinderShape3D", category: Category::Physics, spawn: spawn_collision_cylinder },
];

// --- Physics Bodies ---

pub fn spawn_rigidbody(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_physics_body(commands, PhysicsBodyData::default(), "RigidBody3D", parent)
}

pub fn spawn_staticbody(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_physics_body(commands, PhysicsBodyData::static_body(), "StaticBody3D", parent)
}

pub fn spawn_kinematicbody(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_physics_body(commands, PhysicsBodyData::kinematic_body(), "KinematicBody3D", parent)
}

fn spawn_physics_body(
    commands: &mut Commands,
    body_data: PhysicsBodyData,
    name: &str,
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
        body_data,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

// --- Collision Shapes ---

pub fn spawn_collision_box(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_collision_shape(commands, CollisionShapeData::default(), "BoxShape3D", parent)
}

pub fn spawn_collision_sphere(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_collision_shape(commands, CollisionShapeData::sphere(0.5), "SphereShape3D", parent)
}

pub fn spawn_collision_capsule(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_collision_shape(commands, CollisionShapeData::capsule(0.5, 0.5), "CapsuleShape3D", parent)
}

pub fn spawn_collision_cylinder(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    spawn_collision_shape(commands, CollisionShapeData::cylinder(0.5, 0.5), "CylinderShape3D", parent)
}

fn spawn_collision_shape(
    commands: &mut Commands,
    shape_data: CollisionShapeData,
    name: &str,
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
        shape_data,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}
