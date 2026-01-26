//! Entity presets for the Create menu
//!
//! Presets are templates for creating common entity configurations.
//! Each preset creates an entity with a specific set of components.

#![allow(dead_code)]

use bevy::prelude::*;

use super::ComponentRegistry;

/// Categories for the Create menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetCategory {
    /// Empty entity with just transform
    Empty,
    /// 3D mesh objects (cube, sphere, etc.)
    Objects3D,
    /// Light sources
    Lights,
    /// Camera types
    Cameras,
    /// Physics objects
    Physics,
    /// 2D objects
    Objects2D,
    /// UI elements
    UI,
}

impl PresetCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            PresetCategory::Empty => "Empty",
            PresetCategory::Objects3D => "3D Objects",
            PresetCategory::Lights => "Lights",
            PresetCategory::Cameras => "Cameras",
            PresetCategory::Physics => "Physics",
            PresetCategory::Objects2D => "2D Objects",
            PresetCategory::UI => "UI",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            PresetCategory::Empty => "\u{e9a2}", // Circle
            PresetCategory::Objects3D => "\u{e9a2}", // Cube
            PresetCategory::Lights => "\u{e90f}",   // Lightbulb
            PresetCategory::Cameras => "\u{e918}",  // Camera
            PresetCategory::Physics => "\u{e9d9}", // Atom
            PresetCategory::Objects2D => "\u{e9ce}", // Image
            PresetCategory::UI => "\u{e922}",      // Layout
        }
    }

    /// Get all categories in menu order
    pub fn all_in_order() -> &'static [PresetCategory] {
        &[
            PresetCategory::Empty,
            PresetCategory::Objects3D,
            PresetCategory::Lights,
            PresetCategory::Cameras,
            PresetCategory::Physics,
            PresetCategory::Objects2D,
            PresetCategory::UI,
        ]
    }
}

/// Definition of an entity preset
pub struct EntityPreset {
    /// Unique identifier
    pub id: &'static str,
    /// Display name in menu
    pub display_name: &'static str,
    /// Category for grouping
    pub category: PresetCategory,
    /// Icon to show
    pub icon: &'static str,
    /// Default name for new entities
    pub default_name: &'static str,
    /// Component type IDs to add
    pub components: &'static [&'static str],
    /// Menu priority (lower = higher)
    pub priority: i32,
}

/// All built-in entity presets
pub static PRESETS: &[EntityPreset] = &[
    // Empty
    EntityPreset {
        id: "empty",
        display_name: "Empty Entity",
        category: PresetCategory::Empty,
        icon: "\u{e9a2}",
        default_name: "Entity",
        components: &[],
        priority: 0,
    },
    // 3D Objects
    EntityPreset {
        id: "cube",
        display_name: "Cube",
        category: PresetCategory::Objects3D,
        icon: "\u{e9a2}",
        default_name: "Cube",
        components: &["mesh_renderer"], // Will add with Cube mesh type
        priority: 0,
    },
    EntityPreset {
        id: "sphere",
        display_name: "Sphere",
        category: PresetCategory::Objects3D,
        icon: "\u{e9a2}",
        default_name: "Sphere",
        components: &["mesh_renderer"], // Will add with Sphere mesh type
        priority: 1,
    },
    EntityPreset {
        id: "cylinder",
        display_name: "Cylinder",
        category: PresetCategory::Objects3D,
        icon: "\u{e9a2}",
        default_name: "Cylinder",
        components: &["mesh_renderer"], // Will add with Cylinder mesh type
        priority: 2,
    },
    EntityPreset {
        id: "plane",
        display_name: "Plane",
        category: PresetCategory::Objects3D,
        icon: "\u{e9a2}",
        default_name: "Plane",
        components: &["mesh_renderer"], // Will add with Plane mesh type
        priority: 3,
    },
    // Lights
    EntityPreset {
        id: "point_light",
        display_name: "Point Light",
        category: PresetCategory::Lights,
        icon: "\u{e90f}",
        default_name: "Point Light",
        components: &["point_light"],
        priority: 0,
    },
    EntityPreset {
        id: "directional_light",
        display_name: "Directional Light",
        category: PresetCategory::Lights,
        icon: "\u{e9b3}",
        default_name: "Directional Light",
        components: &["directional_light"],
        priority: 1,
    },
    EntityPreset {
        id: "spot_light",
        display_name: "Spot Light",
        category: PresetCategory::Lights,
        icon: "\u{e91a}",
        default_name: "Spot Light",
        components: &["spot_light"],
        priority: 2,
    },
    // Cameras
    EntityPreset {
        id: "camera_3d",
        display_name: "Camera 3D",
        category: PresetCategory::Cameras,
        icon: "\u{e918}",
        default_name: "Camera",
        components: &["camera_3d"],
        priority: 0,
    },
    EntityPreset {
        id: "camera_rig",
        display_name: "Camera Rig",
        category: PresetCategory::Cameras,
        icon: "\u{e918}",
        default_name: "Camera Rig",
        components: &["camera_rig"],
        priority: 1,
    },
    EntityPreset {
        id: "camera_2d",
        display_name: "Camera 2D",
        category: PresetCategory::Cameras,
        icon: "\u{e918}",
        default_name: "Camera 2D",
        components: &["camera_2d"],
        priority: 2,
    },
    // Physics
    EntityPreset {
        id: "rigid_body",
        display_name: "Rigid Body",
        category: PresetCategory::Physics,
        icon: "\u{e9d9}",
        default_name: "Rigid Body",
        components: &["rigid_body", "box_collider"],
        priority: 0,
    },
    EntityPreset {
        id: "static_body",
        display_name: "Static Body",
        category: PresetCategory::Physics,
        icon: "\u{e9d9}",
        default_name: "Static Body",
        components: &["rigid_body", "box_collider"], // Will configure as static
        priority: 1,
    },
    // 2D Objects
    EntityPreset {
        id: "sprite_2d",
        display_name: "Sprite",
        category: PresetCategory::Objects2D,
        icon: "\u{e9ce}",
        default_name: "Sprite",
        components: &["sprite_2d"],
        priority: 0,
    },
    // UI
    EntityPreset {
        id: "ui_panel",
        display_name: "Panel",
        category: PresetCategory::UI,
        icon: "\u{e922}",
        default_name: "Panel",
        components: &["ui_panel"],
        priority: 0,
    },
    EntityPreset {
        id: "ui_label",
        display_name: "Label",
        category: PresetCategory::UI,
        icon: "\u{e8ed}",
        default_name: "Label",
        components: &["ui_label"],
        priority: 1,
    },
    EntityPreset {
        id: "ui_button",
        display_name: "Button",
        category: PresetCategory::UI,
        icon: "\u{e9ca}",
        default_name: "Button",
        components: &["ui_button"],
        priority: 2,
    },
    EntityPreset {
        id: "ui_image",
        display_name: "Image",
        category: PresetCategory::UI,
        icon: "\u{e9ce}",
        default_name: "Image",
        components: &["ui_image"],
        priority: 3,
    },
];

/// Get presets by category
pub fn get_presets_by_category(category: PresetCategory) -> Vec<&'static EntityPreset> {
    let mut presets: Vec<_> = PRESETS
        .iter()
        .filter(|p| p.category == category)
        .collect();
    presets.sort_by_key(|p| p.priority);
    presets
}

/// Get a preset by ID
pub fn get_preset(id: &str) -> Option<&'static EntityPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// Spawn an entity from a preset
pub fn spawn_preset(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    registry: &ComponentRegistry,
    preset: &EntityPreset,
    parent: Option<Entity>,
) -> Entity {
    use crate::core::{EditorEntity, SceneNode};

    // Spawn base entity
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: preset.default_name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
    ));

    // Add parent if specified
    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    let entity = entity_commands.id();

    // Add components from preset
    for component_id in preset.components {
        if let Some(def) = registry.get(component_id) {
            (def.add_fn)(commands, entity, meshes, materials);
        }
    }

    entity
}

/// Infer the icon for an entity based on its components
pub fn infer_entity_icon(world: &World, entity: Entity, registry: &ComponentRegistry) -> &'static str {
    // Check in priority order - first match wins

    // Cameras (highest priority)
    if let Some(def) = registry.get("camera_3d") {
        if (def.has_fn)(world, entity) {
            return "\u{e918}"; // Camera
        }
    }
    if let Some(def) = registry.get("camera_rig") {
        if (def.has_fn)(world, entity) {
            return "\u{e918}"; // Camera
        }
    }
    if let Some(def) = registry.get("camera_2d") {
        if (def.has_fn)(world, entity) {
            return "\u{e918}"; // Camera
        }
    }

    // Lights
    if let Some(def) = registry.get("point_light") {
        if (def.has_fn)(world, entity) {
            return "\u{e90f}"; // Lightbulb
        }
    }
    if let Some(def) = registry.get("directional_light") {
        if (def.has_fn)(world, entity) {
            return "\u{e9b3}"; // Sun
        }
    }
    if let Some(def) = registry.get("spot_light") {
        if (def.has_fn)(world, entity) {
            return "\u{e91a}"; // Flashlight
        }
    }

    // 3D Meshes
    if let Some(def) = registry.get("mesh_renderer") {
        if (def.has_fn)(world, entity) {
            return "\u{e9a2}"; // Cube
        }
    }

    // 2D
    if let Some(def) = registry.get("sprite_2d") {
        if (def.has_fn)(world, entity) {
            return "\u{e9ce}"; // Image
        }
    }

    // UI
    if let Some(def) = registry.get("ui_panel") {
        if (def.has_fn)(world, entity) {
            return "\u{e922}"; // Layout
        }
    }
    if let Some(def) = registry.get("ui_label") {
        if (def.has_fn)(world, entity) {
            return "\u{e8ed}"; // Text
        }
    }
    if let Some(def) = registry.get("ui_button") {
        if (def.has_fn)(world, entity) {
            return "\u{e9ca}"; // Button
        }
    }
    if let Some(def) = registry.get("ui_image") {
        if (def.has_fn)(world, entity) {
            return "\u{e9ce}"; // Image
        }
    }

    // Physics
    if let Some(def) = registry.get("rigid_body") {
        if (def.has_fn)(world, entity) {
            return "\u{e9d9}"; // Atom
        }
    }

    // Default - empty circle
    "\u{e9a1}"
}
