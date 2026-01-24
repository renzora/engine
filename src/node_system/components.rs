use bevy::prelude::*;

// Re-export shared components
pub use crate::shared::components::{
    CameraNodeData, CollisionShapeData, CollisionShapeType, MeshInstanceData, MeshNodeData,
    MeshPrimitiveType, PhysicsBodyData, PhysicsBodyType, SceneInstanceData,
};

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
