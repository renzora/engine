//! Shared code between editor and runtime
//!
//! This module contains types and functionality that are used by both
//! the editor and the standalone game runtime.
//!
//! All component data types use Bevy's Reflect system for automatic
//! serialization to scene files (.ron).

pub mod components;
pub mod core_components;
pub mod light_spawner;
pub mod physics_plugin;
pub mod physics_spawner;

#[cfg(test)]
mod tests;

// Re-export commonly used items
pub use components::{
    // Camera
    Camera2DData, CameraNodeData, CameraRigData,
    // Environment
    CloudsData, PanoramaSkyData, ProceduralSkyData, SkyMode, WorldEnvironmentData,
    // Post-processing
    TonemappingMode, SkyboxData, FogData, AntiAliasingData, AmbientOcclusionData,
    ReflectionsData, BloomData, TonemappingData, DepthOfFieldData, MotionBlurData, AmbientLightData,
    TaaData, SmaaData, SmaaPresetMode, CasData, ChromaticAberrationData,
    AutoExposureData, VolumetricFogData,
    VignetteData, FilmGrainData, PixelationData, CrtData, GodRaysData,
    GaussianBlurData, PaletteQuantizationData, DistortionData, UnderwaterData,
    // Instances
    MeshInstanceData, SceneInstanceData,
    // Lights
    DirectionalLightData, PointLightData, SpotLightData, SunData,
    // Physics
    CollisionShapeData, CollisionShapeType, PhysicsBodyData, PhysicsBodyType,
    // Rendering
    DlssQualityMode, MaterialData, MeshNodeData, MeshPrimitiveType, MeshletMeshData, ShapeCategory, SolariLightingData, Sprite2DData,
    // UI
    UIButtonData, UIImageData, UILabelData, UIPanelData,
    // Animation
    animation::{EditorAnimationClip, GltfAnimations, GltfAnimationHandles, GltfAnimationStorage, KeyframeValue},
};

// Light exports
pub use light_spawner::{
    despawn_light_components, spawn_directional_light, spawn_entity_lights, spawn_point_light,
    spawn_spot_light, RuntimeLight,
};

// Physics exports
pub use physics_plugin::RenzoraPhysicsPlugin;
pub use physics_spawner::{
    despawn_physics_components, spawn_collision_shape, spawn_entity_physics, spawn_physics_body,
    RuntimePhysics,
};

// Core component exports (for runtime scripting support)
pub use core_components::{
    DisabledComponents, EditorEntity, HealthData, SceneNode, SceneTabId, ScriptComponent,
    ScriptRuntimeState, ScriptVariableValue, WorldEnvironmentMarker, register_core_types,
};
