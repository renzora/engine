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

mod definition;
mod registry;
mod inspector_integration;
pub mod presets;

pub mod components;

pub use definition::*;
pub use registry::*;
pub use inspector_integration::*;
pub use presets::{PresetCategory, get_presets_by_category, spawn_preset};

use bevy::prelude::*;

/// Plugin that initializes the component system
pub struct ComponentSystemPlugin;

impl Plugin for ComponentSystemPlugin {
    fn build(&self, app: &mut App) {
        let mut registry = ComponentRegistry::new();

        // Register all built-in components
        components::register_all_components(&mut registry);

        app.insert_resource(registry);
        app.init_resource::<AddComponentPopupState>();
    }
}
