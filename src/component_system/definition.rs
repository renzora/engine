//! Component definition types

#![allow(dead_code)]

use bevy::prelude::*;
use bevy_egui::egui;

/// Function signature for adding a component to an entity
pub type AddComponentFn = fn(&mut Commands, Entity, &mut Assets<Mesh>, &mut Assets<StandardMaterial>);

/// Function signature for removing a component from an entity
pub type RemoveComponentFn = fn(&mut Commands, Entity);

/// Function signature for checking if an entity has this component
pub type HasComponentFn = fn(&World, Entity) -> bool;

/// Function signature for serializing a component to JSON
pub type SerializeComponentFn = fn(&World, Entity) -> Option<serde_json::Value>;

/// Function signature for deserializing a component from JSON
pub type DeserializeComponentFn = fn(
    &mut EntityCommands,
    &serde_json::Value,
    &mut Assets<Mesh>,
    &mut Assets<StandardMaterial>,
);

/// Function signature for rendering component inspector UI
/// Returns true if the component was modified
pub type InspectorFn = fn(
    &mut egui::Ui,
    &mut World,
    Entity,
    &mut Assets<Mesh>,
    &mut Assets<StandardMaterial>,
) -> bool;

/// Categories for grouping components in the Add Component menu
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentCategory {
    /// Mesh rendering, sprites, etc.
    Rendering,
    /// Point lights, directional lights, spot lights
    Lighting,
    /// Rigid bodies, colliders
    Physics,
    /// 3D and 2D cameras
    Camera,
    /// Audio sources, listeners
    Audio,
    /// Script components
    Scripting,
    /// UI elements
    UI,
}

impl ComponentCategory {
    /// Get display name for the category
    pub fn display_name(&self) -> &'static str {
        match self {
            ComponentCategory::Rendering => "Rendering",
            ComponentCategory::Lighting => "Lighting",
            ComponentCategory::Physics => "Physics",
            ComponentCategory::Camera => "Camera",
            ComponentCategory::Audio => "Audio",
            ComponentCategory::Scripting => "Scripting",
            ComponentCategory::UI => "UI",
        }
    }

    /// Get icon for the category
    pub fn icon(&self) -> &'static str {
        match self {
            ComponentCategory::Rendering => "\u{e9a2}", // Cube
            ComponentCategory::Lighting => "\u{e90f}",  // Lightbulb
            ComponentCategory::Physics => "\u{e9d9}",   // Atom/physics
            ComponentCategory::Camera => "\u{e918}",    // Camera
            ComponentCategory::Audio => "\u{e9ce}",     // Volume
            ComponentCategory::Scripting => "\u{ea0c}", // Code
            ComponentCategory::UI => "\u{e922}",        // Layout
        }
    }

    /// Get all categories in menu order
    pub fn all_in_order() -> &'static [ComponentCategory] {
        &[
            ComponentCategory::Rendering,
            ComponentCategory::Lighting,
            ComponentCategory::Camera,
            ComponentCategory::Physics,
            ComponentCategory::Audio,
            ComponentCategory::Scripting,
            ComponentCategory::UI,
        ]
    }
}

/// Definition of a component type that can be added/removed from entities
pub struct ComponentDefinition {
    /// Unique identifier for the component (e.g., "mesh_renderer", "point_light")
    pub type_id: &'static str,

    /// Human-readable name (e.g., "Mesh Renderer", "Point Light")
    pub display_name: &'static str,

    /// Category for grouping in menus
    pub category: ComponentCategory,

    /// Icon for hierarchy and inspector
    pub icon: &'static str,

    /// Priority for ordering within category (lower = higher in list)
    pub priority: i32,

    /// Function to add this component to an entity
    pub add_fn: AddComponentFn,

    /// Function to remove this component from an entity
    pub remove_fn: RemoveComponentFn,

    /// Function to check if an entity has this component
    pub has_fn: HasComponentFn,

    /// Function to serialize this component to JSON
    pub serialize_fn: SerializeComponentFn,

    /// Function to deserialize this component from JSON
    pub deserialize_fn: DeserializeComponentFn,

    /// Function to render inspector UI for this component
    pub inspector_fn: InspectorFn,

    /// Components that conflict with this one (can't have both)
    pub conflicts_with: &'static [&'static str],

    /// Components that this one requires (must be present)
    pub requires: &'static [&'static str],
}

impl ComponentDefinition {
    /// Check if this component conflicts with another
    pub fn conflicts_with_type(&self, other_type_id: &str) -> bool {
        self.conflicts_with.contains(&other_type_id)
    }

    /// Check if this component requires another
    pub fn requires_type(&self, other_type_id: &str) -> bool {
        self.requires.contains(&other_type_id)
    }
}

impl std::fmt::Debug for ComponentDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentDefinition")
            .field("type_id", &self.type_id)
            .field("display_name", &self.display_name)
            .field("category", &self.category)
            .finish()
    }
}
