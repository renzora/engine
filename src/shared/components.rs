//! Shared component data types used by both editor and runtime
//!
//! These are the serializable data types that represent node-specific data.
//! They are stored as components on entities and serialized to scene files.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Data component for mesh nodes - stores the mesh type so it can be serialized
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct MeshNodeData {
    pub mesh_type: MeshPrimitiveType,
}

/// Types of mesh primitives supported
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeshPrimitiveType {
    Cube,
    Sphere,
    Cylinder,
    Plane,
}

#[allow(dead_code)]
impl MeshPrimitiveType {
    /// Get the type_id string for this mesh type
    pub fn type_id(&self) -> &'static str {
        match self {
            MeshPrimitiveType::Cube => "mesh.cube",
            MeshPrimitiveType::Sphere => "mesh.sphere",
            MeshPrimitiveType::Cylinder => "mesh.cylinder",
            MeshPrimitiveType::Plane => "mesh.plane",
        }
    }

    /// Convert from type_id string
    pub fn from_type_id(type_id: &str) -> Option<Self> {
        match type_id {
            "mesh.cube" => Some(MeshPrimitiveType::Cube),
            "mesh.sphere" => Some(MeshPrimitiveType::Sphere),
            "mesh.cylinder" => Some(MeshPrimitiveType::Cylinder),
            "mesh.plane" => Some(MeshPrimitiveType::Plane),
            _ => None,
        }
    }
}

/// Data component for camera nodes
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct CameraNodeData {
    pub fov: f32,
    /// Whether this camera should be used as the default game camera at runtime
    #[serde(default)]
    pub is_default_camera: bool,
}

impl Default for CameraNodeData {
    fn default() -> Self {
        Self {
            fov: 45.0,
            is_default_camera: false,
        }
    }
}

/// Data component for mesh instance nodes - stores the path to a 3D model file
#[derive(Component, Clone, Debug, Serialize, Deserialize, Default)]
pub struct MeshInstanceData {
    /// Path to the 3D model file (relative to assets folder)
    /// None if no model is assigned yet
    pub model_path: Option<String>,
}

/// Data component for scene instance nodes - stores the path to a scene file
/// Scene instances appear as a single collapsed node in the hierarchy.
/// The contents are only loaded/shown when the scene is "opened" for editing.
#[derive(Component, Clone, Debug, Serialize, Deserialize, Default)]
pub struct SceneInstanceData {
    /// Path to the scene file (.scene)
    pub scene_path: String,
    /// Whether the scene instance is currently "open" for editing
    /// When open, children are shown; when closed, only the instance node is visible
    #[serde(default)]
    pub is_open: bool,
}

// ============================================================================
// Physics Components
// ============================================================================

/// Type of physics body
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PhysicsBodyType {
    /// Dynamic body - affected by forces and collisions
    #[default]
    RigidBody,
    /// Static body - never moves, infinite mass
    StaticBody,
    /// Kinematic body - moved programmatically, not affected by forces
    KinematicBody,
}

impl PhysicsBodyType {
    pub fn display_name(&self) -> &'static str {
        match self {
            PhysicsBodyType::RigidBody => "RigidBody3D",
            PhysicsBodyType::StaticBody => "StaticBody3D",
            PhysicsBodyType::KinematicBody => "KinematicBody3D",
        }
    }

    pub fn type_id(&self) -> &'static str {
        match self {
            PhysicsBodyType::RigidBody => "physics.rigidbody3d",
            PhysicsBodyType::StaticBody => "physics.staticbody3d",
            PhysicsBodyType::KinematicBody => "physics.kinematicbody3d",
        }
    }
}

/// Data component for physics body nodes
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct PhysicsBodyData {
    /// Type of physics body
    pub body_type: PhysicsBodyType,
    /// Mass of the body (only for dynamic bodies)
    pub mass: f32,
    /// Gravity multiplier (1.0 = normal gravity, 0.0 = no gravity)
    pub gravity_scale: f32,
    /// Linear velocity damping (drag)
    pub linear_damping: f32,
    /// Angular velocity damping (rotational drag)
    pub angular_damping: f32,
    /// Whether the body can rotate
    pub lock_rotation_x: bool,
    pub lock_rotation_y: bool,
    pub lock_rotation_z: bool,
    /// Whether the body can translate
    pub lock_translation_x: bool,
    pub lock_translation_y: bool,
    pub lock_translation_z: bool,
}

impl Default for PhysicsBodyData {
    fn default() -> Self {
        Self {
            body_type: PhysicsBodyType::RigidBody,
            mass: 1.0,
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.05,
            lock_rotation_x: false,
            lock_rotation_y: false,
            lock_rotation_z: false,
            lock_translation_x: false,
            lock_translation_y: false,
            lock_translation_z: false,
        }
    }
}

impl PhysicsBodyData {
    pub fn static_body() -> Self {
        Self {
            body_type: PhysicsBodyType::StaticBody,
            ..Default::default()
        }
    }

    pub fn kinematic_body() -> Self {
        Self {
            body_type: PhysicsBodyType::KinematicBody,
            ..Default::default()
        }
    }
}

/// Type of collision shape
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CollisionShapeType {
    /// Box collider
    #[default]
    Box,
    /// Sphere collider
    Sphere,
    /// Capsule collider (vertical)
    Capsule,
    /// Cylinder collider
    Cylinder,
}

impl CollisionShapeType {
    pub fn display_name(&self) -> &'static str {
        match self {
            CollisionShapeType::Box => "BoxShape3D",
            CollisionShapeType::Sphere => "SphereShape3D",
            CollisionShapeType::Capsule => "CapsuleShape3D",
            CollisionShapeType::Cylinder => "CylinderShape3D",
        }
    }

    pub fn type_id(&self) -> &'static str {
        match self {
            CollisionShapeType::Box => "physics.collision_box",
            CollisionShapeType::Sphere => "physics.collision_sphere",
            CollisionShapeType::Capsule => "physics.collision_capsule",
            CollisionShapeType::Cylinder => "physics.collision_cylinder",
        }
    }
}

/// Data component for collision shape nodes
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct CollisionShapeData {
    /// Type of collision shape
    pub shape_type: CollisionShapeType,
    /// Half extents for box shape (x, y, z)
    pub half_extents: Vec3,
    /// Radius for sphere, capsule, cylinder
    pub radius: f32,
    /// Half height for capsule, cylinder
    pub half_height: f32,
    /// Friction coefficient (0.0 = frictionless, 1.0 = high friction)
    pub friction: f32,
    /// Restitution (bounciness, 0.0 = no bounce, 1.0 = perfect bounce)
    pub restitution: f32,
    /// Whether this is a sensor (triggers events but doesn't collide)
    pub is_sensor: bool,
}

impl Default for CollisionShapeData {
    fn default() -> Self {
        Self {
            shape_type: CollisionShapeType::Box,
            half_extents: Vec3::splat(0.5),
            radius: 0.5,
            half_height: 0.5,
            friction: 0.5,
            restitution: 0.0,
            is_sensor: false,
        }
    }
}

impl CollisionShapeData {
    pub fn sphere(radius: f32) -> Self {
        Self {
            shape_type: CollisionShapeType::Sphere,
            radius,
            ..Default::default()
        }
    }

    pub fn capsule(radius: f32, half_height: f32) -> Self {
        Self {
            shape_type: CollisionShapeType::Capsule,
            radius,
            half_height,
            ..Default::default()
        }
    }

    pub fn cylinder(radius: f32, half_height: f32) -> Self {
        Self {
            shape_type: CollisionShapeType::Cylinder,
            radius,
            half_height,
            ..Default::default()
        }
    }

    pub fn cuboid(half_extents: Vec3) -> Self {
        Self {
            shape_type: CollisionShapeType::Box,
            half_extents,
            ..Default::default()
        }
    }
}
