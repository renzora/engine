//! Shared code between editor and runtime
//!
//! This module contains types and functionality that are used by both
//! the editor and the standalone game runtime.

pub mod components;
pub mod scene_format;
pub mod spawner;

// Re-export commonly used items
pub use components::{
    CameraNodeData, CameraRigData, CollisionShapeData, CollisionShapeType, MeshInstanceData, MeshNodeData,
    MeshPrimitiveType, PhysicsBodyData, PhysicsBodyType, SceneInstanceData,
    // 2D components
    Sprite2DData, Camera2DData,
    // UI components
    UIPanelData, UILabelData, UIButtonData, UIImageData,
};
pub use scene_format::{EditorCameraData, NodeData, SceneData, TransformData};
pub use spawner::{spawn_node_components, SpawnConfig};
