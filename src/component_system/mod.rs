//! Component-based entity system (Unity-style)
//!
//! This module provides a component registry that allows adding/removing components
//! dynamically from entities. Unlike the node-based system, entities are just entities
//! and components define their behavior.
//!
//! # Architecture
//!
//! - `ComponentRegistry`: Central registry of all available components
//! - `ComponentDefinition`: Describes a component type with add/remove/serialize/inspect functions
//! - `ComponentCategory`: Groups components for the Add Component menu
//! - `UnifiedComponentRegistry`: Wrapper that handles all registration in one place
//!
//! # Example
//!
//! ```rust,ignore
//! // Entity with multiple components
//! Entity "Player"
//!   ├── Transform
//!   ├── MeshRenderer (mesh + material)
//!   ├── RigidBody
//!   └── BoxCollider
//! ```
//!
//! # Automatic Registration
//!
//! Use the `register_component!` macro for simplified component registration:
//!
//! ```rust,ignore
//! register_component!(MyComponent {
//!     type_id: "my_component",
//!     display_name: "My Component",
//!     category: ComponentCategory::Gameplay,
//!     icon: STAR,
//! });
//! ```

mod definition;
mod registry;
mod inspector_integration;
pub mod presets;
pub mod register_macro;

pub mod components;
pub mod data;

#[cfg(test)]
mod tests;

pub use definition::*;
pub use registry::*;
pub use inspector_integration::*;

// Re-export data module items so crate::component_system::X works
pub use data::*;
pub use presets::{PresetCategory, get_presets_by_category, spawn_preset, spawn_component_as_node, preset_component_ids};
pub use register_macro::{ComponentDefBuilder, create_component_definition};

use bevy::prelude::*;
use bevy::reflect::GetTypeRegistration;
use serde::{de::DeserializeOwned, Serialize};

use crate::scene::SceneSaveableRegistry;

/// Unified registry that handles all component registration in one place
///
/// This wraps:
/// - `ComponentRegistry`: For inspector/Add Component menu
/// - `SceneSaveableRegistry`: For scene serialization
/// - Bevy's `TypeRegistry`: For reflection
///
/// # Example
///
/// ```rust,ignore
/// let mut unified = UnifiedComponentRegistry::new(&mut component_registry, &mut saveable_registry);
/// unified.register::<MyComponent>(&MY_COMPONENT_DEFINITION, app);
/// ```
pub struct UnifiedComponentRegistry<'a> {
    pub component_registry: &'a mut ComponentRegistry,
    pub saveable_registry: &'a mut SceneSaveableRegistry,
}

impl<'a> UnifiedComponentRegistry<'a> {
    /// Create a new unified registry wrapping existing registries
    pub fn new(
        component_registry: &'a mut ComponentRegistry,
        saveable_registry: &'a mut SceneSaveableRegistry,
    ) -> Self {
        Self {
            component_registry,
            saveable_registry,
        }
    }

    /// Register a component with all registries
    ///
    /// This single call:
    /// 1. Registers the ComponentDefinition with ComponentRegistry
    /// 2. Registers the type with SceneSaveableRegistry for serialization
    /// 3. Registers the type with Bevy's TypeRegistry (if app is provided)
    pub fn register<T>(&mut self, definition: &'static ComponentDefinition)
    where
        T: Component + Reflect + GetTypeRegistration + 'static,
    {
        // Register with component registry (for inspector/menu)
        self.component_registry.register(definition);

        // Register with saveable registry (for scene serialization)
        self.saveable_registry.register::<T>();
    }

    /// Register a component using the builder pattern
    pub fn register_with_def<T>(&mut self, definition: ComponentDefinition)
    where
        T: Component<Mutability = bevy::ecs::component::Mutable> + Default + Reflect + Serialize + DeserializeOwned + GetTypeRegistration + 'static,
    {
        // For dynamic definitions, we need to leak them to get 'static lifetime
        // This is acceptable for registration that happens once at startup
        let static_def: &'static ComponentDefinition = Box::leak(Box::new(definition));

        self.component_registry.register(static_def);
        self.saveable_registry.register::<T>();
    }
}

/// Plugin that initializes the component system
pub struct ComponentSystemPlugin;

impl Plugin for ComponentSystemPlugin {
    fn build(&self, app: &mut App) {
        let mut registry = ComponentRegistry::new();

        // Register all built-in components
        components::register_all_components(&mut registry);

        app.insert_resource(registry);
        app.init_resource::<AddComponentPopupState>();
        app.init_resource::<components::skybox::SkyboxState>();
        app.init_resource::<components::sun::SunDiscState>();
        app.init_resource::<components::clouds::CloudsState>();
        app.add_plugins(bevy::pbr::MaterialPlugin::<components::clouds::CloudMaterial>::default());

        // Register FullscreenMaterial plugins for custom post-processing effects
        {
            use bevy::core_pipeline::fullscreen_material::FullscreenMaterialPlugin;
            use crate::post_process::*;
            app.add_plugins((
                FullscreenMaterialPlugin::<VignetteSettings>::default(),
                FullscreenMaterialPlugin::<FilmGrainSettings>::default(),
                FullscreenMaterialPlugin::<PixelationSettings>::default(),
                FullscreenMaterialPlugin::<CrtSettings>::default(),
                FullscreenMaterialPlugin::<GodRaysSettings>::default(),
                FullscreenMaterialPlugin::<GaussianBlurSettings>::default(),
                FullscreenMaterialPlugin::<PaletteQuantizationSettings>::default(),
                FullscreenMaterialPlugin::<DistortionSettings>::default(),
                FullscreenMaterialPlugin::<UnderwaterSettings>::default(),
            ));
        }

        // Add system to process pending component operations
        app.add_systems(Update, (
            process_pending_component_operations,
            components::lighting::sync_light_data_to_bevy,
        ));

        // Cloth sync system
        app.add_systems(Update, components::cloth::sync_cloth_data);

        // Navigation agent pathfinding + movement
        app.add_systems(Update, components::navigation_agent::navigation_agent_system);

        // Post-processing and lighting sync systems
        app.add_systems(Update, (
            components::ambient_light::sync_ambient_light,
            components::fog::sync_fog,
            components::anti_aliasing::sync_anti_aliasing,
            components::ambient_occlusion::sync_ambient_occlusion,
            components::reflections::sync_reflections,
            components::bloom::sync_bloom,
            components::tonemapping::sync_tonemapping,
            components::depth_of_field::sync_depth_of_field,
            components::motion_blur::sync_motion_blur,
            components::skybox::sync_skybox,
            components::sun::sync_sun_disc,
            components::clouds::sync_clouds,
        ).run_if(in_state(crate::core::AppState::Editor).or(in_state(crate::core::AppState::Runtime))));

        // New post-processing sync systems (split into groups of <=12 for tuple size limit)
        app.add_systems(Update, (
            components::taa::sync_taa,
            components::smaa::sync_smaa,
            components::cas::sync_cas,
            components::chromatic_aberration::sync_chromatic_aberration,
            components::auto_exposure::sync_auto_exposure,
            components::volumetric_fog::sync_volumetric_fog,
            components::vignette::sync_vignette,
            components::film_grain::sync_film_grain,
            components::pixelation::sync_pixelation,
            components::crt::sync_crt,
            components::god_rays::sync_god_rays,
            components::gaussian_blur::sync_gaussian_blur,
        ).run_if(in_state(crate::core::AppState::Editor).or(in_state(crate::core::AppState::Runtime))));

        app.add_systems(Update, (
            components::palette_quantization::sync_palette_quantization,
            components::distortion::sync_distortion,
            components::underwater::sync_underwater,
        ).run_if(in_state(crate::core::AppState::Editor).or(in_state(crate::core::AppState::Runtime))));
    }
}

/// System to process pending component add/remove operations
/// These are queued from the inspector UI and processed here to avoid borrow conflicts
fn process_pending_component_operations(
    mut popup_state: ResMut<AddComponentPopupState>,
    registry: Res<ComponentRegistry>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Process pending add
    if let Some((entity, type_id)) = popup_state.pending_add.take() {
        if let Some(def) = registry.get(type_id) {
            (def.add_fn)(&mut commands, entity, &mut meshes, &mut materials);
        }
    }

    // Process pending remove
    if let Some((entity, type_id)) = popup_state.pending_remove.take() {
        if let Some(def) = registry.get(type_id) {
            (def.remove_fn)(&mut commands, entity);
        }
    }
}
