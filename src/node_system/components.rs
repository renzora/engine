use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Marker component that identifies the node type for serialization
/// This is attached to every spawned node and used to look up the correct
/// serialization/deserialization functions from the registry
#[derive(Component, Clone)]
pub struct NodeTypeMarker {
    /// The type_id from the NodeDefinition (e.g., "mesh.cube", "light.point")
    /// Used when saving scenes to identify the correct serialization function
    #[allow(dead_code)]
    pub type_id: &'static str,
}

impl NodeTypeMarker {
    pub fn new(type_id: &'static str) -> Self {
        Self { type_id }
    }
}

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
}

impl Default for CameraNodeData {
    fn default() -> Self {
        Self { fov: 45.0 }
    }
}

/// Data component for mesh instance nodes - stores the path to a 3D model file
#[derive(Component, Clone, Debug, Serialize, Deserialize, Default)]
pub struct MeshInstanceData {
    /// Path to the 3D model file (relative to assets folder)
    /// None if no model is assigned yet
    pub model_path: Option<String>,
}
