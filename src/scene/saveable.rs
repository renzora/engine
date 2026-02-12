//! Scene saveable component registration
//!
//! This module provides automatic registration of components for scene serialization.
//! Instead of manually adding each component to the scene saver's allow list,
//! components register themselves here and the saver uses this registry.

use bevy::prelude::*;
use bevy::scene::DynamicSceneBuilder;

/// A function that allows a component type on a DynamicSceneBuilder
/// Takes ownership and returns the modified builder
type AllowComponentFn = fn(DynamicSceneBuilder) -> DynamicSceneBuilder;

/// Registry of all components that should be saved in scenes
#[derive(Resource, Default)]
pub struct SceneSaveableRegistry {
    /// Functions to allow each registered component type
    allow_fns: Vec<AllowComponentFn>,
    /// Type names for debugging
    type_names: Vec<&'static str>,
}

impl SceneSaveableRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a component type for scene saving
    /// The component must implement Reflect and Component
    pub fn register<T: Component + Reflect + bevy::reflect::GetTypeRegistration>(&mut self) {
        let type_name = std::any::type_name::<T>();

        // Avoid duplicate registration
        if self.type_names.contains(&type_name) {
            return;
        }

        self.allow_fns.push(|builder| {
            builder.allow_component::<T>()
        });
        self.type_names.push(type_name);
    }

    /// Apply all registered component types to a DynamicSceneBuilder
    /// Takes ownership of the builder and returns it after allowing all registered types
    pub fn allow_all<'w>(&self, mut builder: DynamicSceneBuilder<'w>) -> DynamicSceneBuilder<'w> {
        for allow_fn in &self.allow_fns {
            builder = allow_fn(builder);
        }
        builder
    }

    /// Get the number of registered saveable components
    pub fn len(&self) -> usize {
        self.allow_fns.len()
    }

    /// Check if no components are registered
    pub fn is_empty(&self) -> bool {
        self.allow_fns.is_empty()
    }

    /// Get registered type names (for debugging)
    pub fn type_names(&self) -> &[&'static str] {
        &self.type_names
    }
}

/// Helper macro to register multiple components at once
#[macro_export]
macro_rules! register_saveable {
    ($registry:expr, $($component:ty),+ $(,)?) => {
        $(
            $registry.register::<$component>();
        )+
    };
}

/// Initialize the scene saveable registry with all default components
pub fn create_default_registry() -> SceneSaveableRegistry {
    use crate::core::{EditorEntity, SceneNode, WorldEnvironmentMarker};
    use crate::shared::{
        MeshNodeData, CameraNodeData, CameraRigData, MeshInstanceData, SceneInstanceData,
        PhysicsBodyData, CollisionShapeData, Sprite2DData, Camera2DData,
        UIPanelData, UILabelData, UIButtonData, UIImageData, MaterialData,
        SolariLightingData, MeshletMeshData,
    };
    use crate::scripting::ScriptComponent;
    use crate::terrain::{TerrainData, TerrainChunkData};
    use crate::particles::HanabiEffectData;

    let mut registry = SceneSaveableRegistry::new();

    // Core Bevy components
    registry.register::<Transform>();
    registry.register::<Name>();
    registry.register::<ChildOf>();

    // Editor components
    registry.register::<EditorEntity>();
    registry.register::<SceneNode>();
    registry.register::<crate::core::DisabledComponents>();

    // Shared game components
    registry.register::<MeshNodeData>();
    registry.register::<MaterialData>();
    registry.register::<CameraNodeData>();
    registry.register::<CameraRigData>();
    registry.register::<MeshInstanceData>();
    registry.register::<SceneInstanceData>();
    registry.register::<PhysicsBodyData>();
    registry.register::<CollisionShapeData>();
    registry.register::<Sprite2DData>();
    registry.register::<Camera2DData>();
    registry.register::<UIPanelData>();
    registry.register::<UILabelData>();
    registry.register::<UIButtonData>();
    registry.register::<UIImageData>();

    // Scripting
    registry.register::<ScriptComponent>();

    // Environment
    registry.register::<WorldEnvironmentMarker>();

    // Post-processing components
    registry.register::<crate::shared::SkyboxData>();
    registry.register::<crate::shared::FogData>();
    registry.register::<crate::shared::AntiAliasingData>();
    registry.register::<crate::shared::AmbientOcclusionData>();
    registry.register::<crate::shared::ReflectionsData>();
    registry.register::<crate::shared::BloomData>();
    registry.register::<crate::shared::TonemappingData>();
    registry.register::<crate::shared::DepthOfFieldData>();
    registry.register::<crate::shared::MotionBlurData>();
    registry.register::<crate::shared::AmbientLightData>();
    registry.register::<crate::shared::CloudsData>();
    registry.register::<crate::shared::TaaData>();
    registry.register::<crate::shared::SmaaData>();
    registry.register::<crate::shared::CasData>();
    registry.register::<crate::shared::ChromaticAberrationData>();
    registry.register::<crate::shared::AutoExposureData>();
    registry.register::<crate::shared::VolumetricFogData>();
    registry.register::<crate::shared::VignetteData>();
    registry.register::<crate::shared::FilmGrainData>();
    registry.register::<crate::shared::PixelationData>();
    registry.register::<crate::shared::CrtData>();
    registry.register::<crate::shared::GodRaysData>();
    registry.register::<crate::shared::GaussianBlurData>();
    registry.register::<crate::shared::PaletteQuantizationData>();
    registry.register::<crate::shared::DistortionData>();
    registry.register::<crate::shared::UnderwaterData>();

    // Advanced rendering
    registry.register::<SolariLightingData>();  // Raytraced lighting settings
    registry.register::<MeshletMeshData>();     // Virtual geometry/meshlet settings

    // Terrain (both root and chunk data)
    registry.register::<TerrainData>();
    registry.register::<TerrainChunkData>();

    // Particle effects
    registry.register::<HanabiEffectData>();

    // Lights (Bevy built-in + data components)
    registry.register::<PointLight>();
    registry.register::<DirectionalLight>();
    registry.register::<SpotLight>();
    registry.register::<crate::shared::SunData>();

    registry
}
