//! Shared component data types used by both editor and runtime
//!
//! These are the serializable data types that represent node-specific data.
//! They are stored as components on entities and serialized to scene files
//! via Bevy's DynamicScene and reflection system.

pub mod animation;
mod camera;
pub mod environment;
mod instances;
mod night_stars;
mod physics;
pub mod post_process;
mod rendering;
mod ui;

// Re-export all types for convenient access
pub use camera::{Camera2DData, CameraNodeData, CameraRigData};
pub use environment::{
    CloudsData, PanoramaSkyData, ProceduralSkyData, SkyMode, WorldEnvironmentData,
};
pub use post_process::{
    TonemappingMode, SkyboxData, FogData, AntiAliasingData, AmbientOcclusionData,
    ReflectionsData, BloomData, TonemappingData, DepthOfFieldData, MotionBlurData,
    AmbientLightData,
    // New post-processing types
    TaaData, SmaaData, SmaaPresetMode, CasData, ChromaticAberrationData,
    AutoExposureData, VolumetricFogData,
    VignetteData, FilmGrainData, PixelationData, CrtData, GodRaysData,
    GaussianBlurData, PaletteQuantizationData, DistortionData, UnderwaterData,
};
pub use night_stars::NightStarsData;
pub use instances::{MeshInstanceData, SceneInstanceData};
pub use physics::{
    CollisionShapeData, CollisionShapeType, PhysicsBodyData, PhysicsBodyType,
};
pub use rendering::{
    DirectionalLightData, DlssQualityMode, MaterialData, MeshNodeData, MeshPrimitiveType,
    MeshletMeshData, PointLightData, ShapeCategory, SolariLightingData, SpotLightData, Sprite2DData,
    SpriteAnimation, SpriteSheetData, SunData,
};
pub use ui::{UIButtonData, UIImageData, UILabelData, UIPanelData};
