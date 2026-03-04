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
    use crate::component_system::{
        MeshNodeData, CameraNodeData, CameraRigData, MeshInstanceData, SceneInstanceData,
        PhysicsBodyData, CollisionShapeData, Sprite2DData, Camera2DData,
        UIPanelData, UILabelData, UIButtonData, UIImageData, MaterialData,
        SolariLightingData, MeshletMeshData, Text3DData,
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
    registry.register::<crate::core::EntityLabelColor>();
    registry.register::<crate::core::ComponentOrder>();

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
    registry.register::<Text3DData>();
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
    registry.register::<crate::component_system::SkyboxData>();
    registry.register::<crate::component_system::FogData>();
    registry.register::<crate::component_system::AntiAliasingData>();
    registry.register::<crate::component_system::AmbientOcclusionData>();
    registry.register::<crate::component_system::ReflectionsData>();
    registry.register::<crate::component_system::BloomData>();
    registry.register::<crate::component_system::TonemappingData>();
    registry.register::<crate::component_system::DepthOfFieldData>();
    registry.register::<crate::component_system::MotionBlurData>();
    registry.register::<crate::component_system::AmbientLightData>();
    registry.register::<crate::component_system::CloudsData>();
    registry.register::<crate::component_system::NightStarsData>();
    registry.register::<crate::component_system::TaaData>();
    registry.register::<crate::component_system::SmaaData>();
    registry.register::<crate::component_system::CasData>();
    registry.register::<crate::component_system::ChromaticAberrationData>();
    registry.register::<crate::component_system::AutoExposureData>();
    registry.register::<crate::component_system::VolumetricFogData>();
    registry.register::<crate::component_system::VignetteData>();
    registry.register::<crate::component_system::FilmGrainData>();
    registry.register::<crate::component_system::PixelationData>();
    registry.register::<crate::component_system::CrtData>();
    registry.register::<crate::component_system::GodRaysData>();
    registry.register::<crate::component_system::GaussianBlurData>();
    registry.register::<crate::component_system::PaletteQuantizationData>();
    registry.register::<crate::component_system::DistortionData>();
    registry.register::<crate::component_system::UnderwaterData>();

    // Advanced rendering
    registry.register::<SolariLightingData>();  // Raytraced lighting settings
    registry.register::<MeshletMeshData>();     // Virtual geometry/meshlet settings

    // Terrain (both root and chunk data)
    registry.register::<TerrainData>();
    registry.register::<TerrainChunkData>();

    // Particle effects
    registry.register::<HanabiEffectData>();

    // Audio
    registry.register::<crate::component_system::components::audio_emitter::AudioEmitterData>();
    registry.register::<crate::component_system::components::audio_listener::AudioListenerData>();

    // Lights (Bevy built-in + data components)
    registry.register::<PointLight>();
    registry.register::<DirectionalLight>();
    registry.register::<SpotLight>();
    registry.register::<crate::component_system::SunData>();

    registry
}
