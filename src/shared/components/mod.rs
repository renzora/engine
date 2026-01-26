//! Shared component data types used by both editor and runtime
//!
//! These are the serializable data types that represent node-specific data.
//! They are stored as components on entities and serialized to scene files
//! via Bevy's DynamicScene and reflection system.

pub mod animation;
mod camera;
mod environment;
mod instances;
mod physics;
mod rendering;
mod ui;

// Re-export all types for convenient access
pub use camera::{Camera2DData, CameraNodeData, CameraRigData};
pub use environment::{
    PanoramaSkyData, ProceduralSkyData, SkyMode, TonemappingMode, WorldEnvironmentData,
};
pub use instances::{MeshInstanceData, SceneInstanceData};
pub use physics::{
    CollisionShapeData, CollisionShapeType, PhysicsBodyData, PhysicsBodyType,
};
pub use rendering::{MeshNodeData, MeshPrimitiveType, Sprite2DData};
pub use ui::{UIButtonData, UIImageData, UILabelData, UIPanelData};
