use bevy::prelude::*;
use std::collections::HashMap;

/// Function signature for spawning a node
pub type SpawnFn = fn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity;

/// Function signature for serializing a node's type-specific data
pub type SerializeFn = fn(
    entity: Entity,
    world: &World,
) -> Option<HashMap<String, serde_json::Value>>;

/// Function signature for deserializing a node's type-specific data
pub type DeserializeFn = fn(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
);

/// Category for organizing nodes in menus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeCategory {
    /// Basic 3D nodes (Node3D, empty)
    Nodes3D,
    /// Mesh primitives (Cube, Sphere, Cylinder, Plane)
    Meshes,
    /// Light sources (Point, Directional, Spot)
    Lights,
    /// Physics bodies and collision shapes
    Physics,
    /// Environment nodes (WorldEnvironment, AudioListener)
    Environment,
    /// Camera nodes
    Cameras,
    /// 2D nodes (Sprite, Node2D, etc.)
    Nodes2D,
    /// UI nodes (Panel, Label, Button, etc.)
    UI,
    /// Custom user-defined nodes (kept for future extensibility)
    #[allow(dead_code)]
    Custom,
}

impl NodeCategory {
    /// Display name for the category in menus
    pub fn display_name(&self) -> &'static str {
        match self {
            NodeCategory::Nodes3D => "3D Nodes",
            NodeCategory::Meshes => "Meshes",
            NodeCategory::Lights => "Lights",
            NodeCategory::Physics => "Physics",
            NodeCategory::Environment => "Environment",
            NodeCategory::Cameras => "Camera",
            NodeCategory::Nodes2D => "2D Nodes",
            NodeCategory::UI => "UI",
            NodeCategory::Custom => "Custom",
        }
    }

    /// Order for displaying categories in menus
    pub fn menu_order(&self) -> i32 {
        match self {
            NodeCategory::Nodes3D => 0,
            NodeCategory::Meshes => 1,
            NodeCategory::Lights => 2,
            NodeCategory::Physics => 3,
            NodeCategory::Environment => 4,
            NodeCategory::Cameras => 5,
            NodeCategory::Nodes2D => 6,
            NodeCategory::UI => 7,
            NodeCategory::Custom => 100,
        }
    }

    /// All standard categories in menu order (kept for future use)
    #[allow(dead_code)]
    pub fn all_in_order() -> &'static [NodeCategory] {
        &[
            NodeCategory::Nodes3D,
            NodeCategory::Meshes,
            NodeCategory::Lights,
            NodeCategory::Physics,
            NodeCategory::Environment,
            NodeCategory::Cameras,
            NodeCategory::Nodes2D,
            NodeCategory::UI,
            NodeCategory::Custom,
        ]
    }
}

/// Definition of a node type - declares how to create, serialize, and display a node
pub struct NodeDefinition {
    /// Unique identifier for this node type (e.g., "mesh.cube", "light.point")
    pub type_id: &'static str,
    /// Display name shown in menus and inspector (e.g., "Cube", "Point Light")
    pub display_name: &'static str,
    /// Category for grouping in menus
    pub category: NodeCategory,
    /// Default name for newly spawned nodes
    pub default_name: &'static str,
    /// Function to spawn this node type
    pub spawn_fn: SpawnFn,
    /// Optional function to serialize type-specific data (kept for save functionality)
    #[allow(dead_code)]
    pub serialize_fn: Option<SerializeFn>,
    /// Optional function to deserialize type-specific data
    pub deserialize_fn: Option<DeserializeFn>,
    /// Priority for sorting within category (lower = higher in menu)
    pub priority: i32,
}

#[allow(dead_code)]
impl NodeDefinition {
    /// Create a new node definition with required fields
    pub const fn new(
        type_id: &'static str,
        display_name: &'static str,
        category: NodeCategory,
        default_name: &'static str,
        spawn_fn: SpawnFn,
    ) -> Self {
        Self {
            type_id,
            display_name,
            category,
            default_name,
            spawn_fn,
            serialize_fn: None,
            deserialize_fn: None,
            priority: 0,
        }
    }

    /// Set serialization function
    pub const fn with_serialize(mut self, serialize_fn: SerializeFn) -> Self {
        self.serialize_fn = Some(serialize_fn);
        self
    }

    /// Set deserialization function
    pub const fn with_deserialize(mut self, deserialize_fn: DeserializeFn) -> Self {
        self.deserialize_fn = Some(deserialize_fn);
        self
    }

    /// Set both serialization and deserialization functions
    pub const fn with_serialization(
        mut self,
        serialize_fn: SerializeFn,
        deserialize_fn: DeserializeFn,
    ) -> Self {
        self.serialize_fn = Some(serialize_fn);
        self.deserialize_fn = Some(deserialize_fn);
        self
    }

    /// Set priority for menu ordering
    pub const fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}
