//! Shared code between editor and runtime
//!
//! This module contains types and functionality that are used by both
//! the editor and the standalone game runtime.
//!
//! All component data types use Bevy's Reflect system for automatic
//! serialization to scene files (.ron).

pub mod components;

// Re-export commonly used items
pub use components::{
    // Camera
    Camera2DData, CameraNodeData, CameraRigData,
    // Environment
    PanoramaSkyData, ProceduralSkyData, SkyMode, TonemappingMode, WorldEnvironmentData,
    // Instances
    MeshInstanceData, SceneInstanceData,
    // Physics
    CollisionShapeData, CollisionShapeType, PhysicsBodyData, PhysicsBodyType,
    // Rendering
    MeshNodeData, MeshPrimitiveType, Sprite2DData,
    // UI
    UIButtonData, UIImageData, UILabelData, UIPanelData,
};
