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

/// Data component for camera rig nodes - a third-person camera that follows a target
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct CameraRigData {
    /// Distance from the target (how far behind)
    pub distance: f32,
    /// Height offset from the target
    pub height: f32,
    /// Horizontal offset (for over-the-shoulder cameras)
    pub horizontal_offset: f32,
    /// Field of view in degrees
    pub fov: f32,
    /// How fast the camera follows (0 = instant, higher = smoother)
    pub follow_smoothing: f32,
    /// How fast the camera rotates to look at target
    pub look_smoothing: f32,
    /// Whether this is the default game camera
    #[serde(default)]
    pub is_default_camera: bool,
}

impl Default for CameraRigData {
    fn default() -> Self {
        Self {
            distance: 5.0,
            height: 2.0,
            horizontal_offset: 0.0,
            fov: 60.0,
            follow_smoothing: 5.0,
            look_smoothing: 10.0,
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

// ============================================================================
// 2D Components
// ============================================================================

/// Data component for 2D sprite nodes
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Sprite2DData {
    /// Path to the texture file (relative to assets folder)
    pub texture_path: String,
    /// Sprite color/tint (RGBA)
    pub color: Vec4,
    /// Whether to flip the sprite horizontally
    pub flip_x: bool,
    /// Whether to flip the sprite vertically
    pub flip_y: bool,
    /// Anchor point (0.5, 0.5 = center)
    pub anchor: Vec2,
}

impl Default for Sprite2DData {
    fn default() -> Self {
        Self {
            texture_path: String::new(),
            color: Vec4::ONE,
            flip_x: false,
            flip_y: false,
            anchor: Vec2::new(0.5, 0.5),
        }
    }
}

/// Data component for 2D camera nodes
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Camera2DData {
    /// Camera zoom level (1.0 = normal, 2.0 = 2x zoom in)
    pub zoom: f32,
    /// Whether this is the default game camera
    #[serde(default)]
    pub is_default_camera: bool,
}

impl Default for Camera2DData {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            is_default_camera: false,
        }
    }
}

// ============================================================================
// UI Components
// ============================================================================

/// Data component for UI panel nodes
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct UIPanelData {
    /// Panel width in pixels
    pub width: f32,
    /// Panel height in pixels
    pub height: f32,
    /// Background color (RGBA)
    pub background_color: Vec4,
    /// Border radius for rounded corners
    pub border_radius: f32,
    /// Padding inside the panel
    pub padding: f32,
}

impl Default for UIPanelData {
    fn default() -> Self {
        Self {
            width: 200.0,
            height: 100.0,
            background_color: Vec4::new(0.2, 0.2, 0.25, 1.0),
            border_radius: 4.0,
            padding: 8.0,
        }
    }
}

/// Data component for UI label nodes
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct UILabelData {
    /// Text content
    pub text: String,
    /// Font size
    pub font_size: f32,
    /// Text color (RGBA)
    pub color: Vec4,
}

impl Default for UILabelData {
    fn default() -> Self {
        Self {
            text: "Label".to_string(),
            font_size: 16.0,
            color: Vec4::ONE,
        }
    }
}

/// Data component for UI button nodes
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct UIButtonData {
    /// Button text
    pub text: String,
    /// Button width
    pub width: f32,
    /// Button height
    pub height: f32,
    /// Font size
    pub font_size: f32,
    /// Normal background color
    pub normal_color: Vec4,
    /// Hover background color
    pub hover_color: Vec4,
    /// Pressed background color
    pub pressed_color: Vec4,
    /// Text color
    pub text_color: Vec4,
}

impl Default for UIButtonData {
    fn default() -> Self {
        Self {
            text: "Button".to_string(),
            width: 120.0,
            height: 40.0,
            font_size: 16.0,
            normal_color: Vec4::new(0.3, 0.3, 0.35, 1.0),
            hover_color: Vec4::new(0.4, 0.4, 0.45, 1.0),
            pressed_color: Vec4::new(0.2, 0.2, 0.25, 1.0),
            text_color: Vec4::ONE,
        }
    }
}

/// Data component for UI image nodes
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct UIImageData {
    /// Path to the image texture
    pub texture_path: String,
    /// Image width
    pub width: f32,
    /// Image height
    pub height: f32,
    /// Color tint (RGBA)
    pub tint: Vec4,
}

impl Default for UIImageData {
    fn default() -> Self {
        Self {
            texture_path: String::new(),
            width: 100.0,
            height: 100.0,
            tint: Vec4::ONE,
        }
    }
}
