//! Entity presets for the Create menu
//!
//! Presets are templates for creating common entity configurations.
//! Each preset creates an entity with a specific set of components.

#![allow(dead_code)]

use bevy::prelude::*;

use super::{ComponentDefinition, ComponentRegistry};

// Phosphor icons
use egui_phosphor::regular::{
    CUBE, SPHERE, CYLINDER, SQUARE, LIGHTBULB, SUN, FLASHLIGHT,
    VIDEO_CAMERA, ATOM, IMAGE, STACK, TEXTBOX, CURSOR_CLICK,
    GLOBE, SPEAKER_HIGH, CIRCLE, MOUNTAINS, SPARKLE,
    TRIANGLE, POLYGON, DIAMOND,
};

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
    /// Environment settings
    Environment,
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
            PresetCategory::Environment => "Environment",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            PresetCategory::Empty => CIRCLE,
            PresetCategory::Objects3D => CUBE,
            PresetCategory::Lights => LIGHTBULB,
            PresetCategory::Cameras => VIDEO_CAMERA,
            PresetCategory::Physics => ATOM,
            PresetCategory::Objects2D => IMAGE,
            PresetCategory::UI => STACK,
            PresetCategory::Environment => GLOBE,
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
            PresetCategory::Environment,
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
        icon: CIRCLE,
        default_name: "Entity",
        components: &[],
        priority: 0,
    },
    // 3D Objects
    EntityPreset {
        id: "cube",
        display_name: "Cube",
        category: PresetCategory::Objects3D,
        icon: CUBE,
        default_name: "Cube",
        components: &["mesh_renderer"], // Will add with Cube mesh type
        priority: 0,
    },
    EntityPreset {
        id: "sphere",
        display_name: "Sphere",
        category: PresetCategory::Objects3D,
        icon: SPHERE,
        default_name: "Sphere",
        components: &["mesh_renderer"], // Will add with Sphere mesh type
        priority: 1,
    },
    EntityPreset {
        id: "cylinder",
        display_name: "Cylinder",
        category: PresetCategory::Objects3D,
        icon: CYLINDER,
        default_name: "Cylinder",
        components: &["mesh_renderer"], // Will add with Cylinder mesh type
        priority: 2,
    },
    EntityPreset {
        id: "plane",
        display_name: "Plane",
        category: PresetCategory::Objects3D,
        icon: SQUARE,
        default_name: "Plane",
        components: &["mesh_renderer"], // Will add with Plane mesh type
        priority: 3,
    },
    EntityPreset {
        id: "cone",
        display_name: "Cone",
        category: PresetCategory::Objects3D,
        icon: TRIANGLE,
        default_name: "Cone",
        components: &["mesh_renderer"],
        priority: 4,
    },
    EntityPreset {
        id: "torus",
        display_name: "Torus",
        category: PresetCategory::Objects3D,
        icon: CIRCLE,
        default_name: "Torus",
        components: &["mesh_renderer"],
        priority: 5,
    },
    EntityPreset {
        id: "capsule",
        display_name: "Capsule",
        category: PresetCategory::Objects3D,
        icon: CYLINDER,
        default_name: "Capsule",
        components: &["mesh_renderer"],
        priority: 6,
    },
    EntityPreset {
        id: "wedge",
        display_name: "Wedge",
        category: PresetCategory::Objects3D,
        icon: TRIANGLE,
        default_name: "Wedge",
        components: &["mesh_renderer"],
        priority: 7,
    },
    EntityPreset {
        id: "stairs",
        display_name: "Stairs",
        category: PresetCategory::Objects3D,
        icon: POLYGON,
        default_name: "Stairs",
        components: &["mesh_renderer"],
        priority: 8,
    },
    EntityPreset {
        id: "arch",
        display_name: "Arch",
        category: PresetCategory::Objects3D,
        icon: CIRCLE,
        default_name: "Arch",
        components: &["mesh_renderer"],
        priority: 9,
    },
    EntityPreset {
        id: "half_cylinder",
        display_name: "Half Cylinder",
        category: PresetCategory::Objects3D,
        icon: CYLINDER,
        default_name: "Half Cylinder",
        components: &["mesh_renderer"],
        priority: 10,
    },
    EntityPreset {
        id: "quarter_pipe",
        display_name: "Quarter Pipe",
        category: PresetCategory::Objects3D,
        icon: POLYGON,
        default_name: "Quarter Pipe",
        components: &["mesh_renderer"],
        priority: 11,
    },
    EntityPreset {
        id: "corner",
        display_name: "Corner",
        category: PresetCategory::Objects3D,
        icon: POLYGON,
        default_name: "Corner",
        components: &["mesh_renderer"],
        priority: 12,
    },
    EntityPreset {
        id: "prism",
        display_name: "Prism",
        category: PresetCategory::Objects3D,
        icon: TRIANGLE,
        default_name: "Prism",
        components: &["mesh_renderer"],
        priority: 13,
    },
    EntityPreset {
        id: "pyramid",
        display_name: "Pyramid",
        category: PresetCategory::Objects3D,
        icon: DIAMOND,
        default_name: "Pyramid",
        components: &["mesh_renderer"],
        priority: 14,
    },
    EntityPreset {
        id: "pipe",
        display_name: "Pipe",
        category: PresetCategory::Objects3D,
        icon: CIRCLE,
        default_name: "Pipe",
        components: &["mesh_renderer"],
        priority: 15,
    },
    EntityPreset {
        id: "ring",
        display_name: "Ring",
        category: PresetCategory::Objects3D,
        icon: CIRCLE,
        default_name: "Ring",
        components: &["mesh_renderer"],
        priority: 16,
    },
    EntityPreset {
        id: "wall",
        display_name: "Wall",
        category: PresetCategory::Objects3D,
        icon: SQUARE,
        default_name: "Wall",
        components: &["mesh_renderer"],
        priority: 17,
    },
    EntityPreset {
        id: "ramp",
        display_name: "Ramp",
        category: PresetCategory::Objects3D,
        icon: TRIANGLE,
        default_name: "Ramp",
        components: &["mesh_renderer"],
        priority: 18,
    },
    EntityPreset {
        id: "hemisphere",
        display_name: "Hemisphere",
        category: PresetCategory::Objects3D,
        icon: SPHERE,
        default_name: "Hemisphere",
        components: &["mesh_renderer"],
        priority: 19,
    },
    EntityPreset {
        id: "curved_wall",
        display_name: "Curved Wall",
        category: PresetCategory::Objects3D,
        icon: POLYGON,
        default_name: "Curved Wall",
        components: &["mesh_renderer"],
        priority: 20,
    },
    EntityPreset {
        id: "doorway",
        display_name: "Doorway",
        category: PresetCategory::Objects3D,
        icon: SQUARE,
        default_name: "Doorway",
        components: &["mesh_renderer"],
        priority: 21,
    },
    EntityPreset {
        id: "window_wall",
        display_name: "Window Wall",
        category: PresetCategory::Objects3D,
        icon: SQUARE,
        default_name: "Window Wall",
        components: &["mesh_renderer"],
        priority: 22,
    },
    EntityPreset {
        id: "l_shape",
        display_name: "L-Shape",
        category: PresetCategory::Objects3D,
        icon: POLYGON,
        default_name: "L-Shape",
        components: &["mesh_renderer"],
        priority: 23,
    },
    EntityPreset {
        id: "t_shape",
        display_name: "T-Shape",
        category: PresetCategory::Objects3D,
        icon: POLYGON,
        default_name: "T-Shape",
        components: &["mesh_renderer"],
        priority: 24,
    },
    EntityPreset {
        id: "cross_shape",
        display_name: "Cross",
        category: PresetCategory::Objects3D,
        icon: POLYGON,
        default_name: "Cross",
        components: &["mesh_renderer"],
        priority: 25,
    },
    EntityPreset {
        id: "funnel",
        display_name: "Funnel",
        category: PresetCategory::Objects3D,
        icon: TRIANGLE,
        default_name: "Funnel",
        components: &["mesh_renderer"],
        priority: 26,
    },
    EntityPreset {
        id: "gutter",
        display_name: "Gutter",
        category: PresetCategory::Objects3D,
        icon: CYLINDER,
        default_name: "Gutter",
        components: &["mesh_renderer"],
        priority: 27,
    },
    EntityPreset {
        id: "spiral_stairs",
        display_name: "Spiral Stairs",
        category: PresetCategory::Objects3D,
        icon: POLYGON,
        default_name: "Spiral Stairs",
        components: &["mesh_renderer"],
        priority: 28,
    },
    EntityPreset {
        id: "pillar",
        display_name: "Pillar",
        category: PresetCategory::Objects3D,
        icon: CYLINDER,
        default_name: "Pillar",
        components: &["mesh_renderer"],
        priority: 29,
    },
    // Lights
    EntityPreset {
        id: "point_light",
        display_name: "Point Light",
        category: PresetCategory::Lights,
        icon: LIGHTBULB,
        default_name: "Point Light",
        components: &["point_light"],
        priority: 0,
    },
    EntityPreset {
        id: "directional_light",
        display_name: "Directional Light",
        category: PresetCategory::Lights,
        icon: SUN,
        default_name: "Directional Light",
        components: &["directional_light"],
        priority: 1,
    },
    EntityPreset {
        id: "spot_light",
        display_name: "Spot Light",
        category: PresetCategory::Lights,
        icon: FLASHLIGHT,
        default_name: "Spot Light",
        components: &["spot_light"],
        priority: 2,
    },
    EntityPreset {
        id: "solari_lighting",
        display_name: "Solari Lighting",
        category: PresetCategory::Lights,
        icon: SPARKLE,
        default_name: "Solari Lighting",
        components: &["solari_lighting"],
        priority: 3,
    },
    // Cameras
    EntityPreset {
        id: "camera_3d",
        display_name: "Camera 3D",
        category: PresetCategory::Cameras,
        icon: VIDEO_CAMERA,
        default_name: "Camera",
        components: &["camera_3d"],
        priority: 0,
    },
    EntityPreset {
        id: "camera_rig",
        display_name: "Camera Rig",
        category: PresetCategory::Cameras,
        icon: VIDEO_CAMERA,
        default_name: "Camera Rig",
        components: &["camera_rig"],
        priority: 1,
    },
    EntityPreset {
        id: "camera_2d",
        display_name: "Camera 2D",
        category: PresetCategory::Cameras,
        icon: VIDEO_CAMERA,
        default_name: "Camera 2D",
        components: &["camera_2d"],
        priority: 2,
    },
    // 2D Objects
    EntityPreset {
        id: "sprite_2d",
        display_name: "Sprite",
        category: PresetCategory::Objects2D,
        icon: IMAGE,
        default_name: "Sprite",
        components: &["sprite_2d"],
        priority: 0,
    },
    // UI
    EntityPreset {
        id: "ui_panel",
        display_name: "Panel",
        category: PresetCategory::UI,
        icon: STACK,
        default_name: "Panel",
        components: &["ui_panel"],
        priority: 0,
    },
    EntityPreset {
        id: "ui_label",
        display_name: "Label",
        category: PresetCategory::UI,
        icon: TEXTBOX,
        default_name: "Label",
        components: &["ui_label"],
        priority: 1,
    },
    EntityPreset {
        id: "ui_button",
        display_name: "Button",
        category: PresetCategory::UI,
        icon: CURSOR_CLICK,
        default_name: "Button",
        components: &["ui_button"],
        priority: 2,
    },
    EntityPreset {
        id: "ui_image",
        display_name: "Image",
        category: PresetCategory::UI,
        icon: IMAGE,
        default_name: "Image",
        components: &["ui_image"],
        priority: 3,
    },
    // Physics
    EntityPreset {
        id: "rigid_body_cube",
        display_name: "Rigid Cube",
        category: PresetCategory::Physics,
        icon: CUBE,
        default_name: "Rigid Cube",
        components: &["mesh_renderer", "rigid_body", "box_collider"],
        priority: 0,
    },
    EntityPreset {
        id: "rigid_body_sphere",
        display_name: "Rigid Sphere",
        category: PresetCategory::Physics,
        icon: SPHERE,
        default_name: "Rigid Sphere",
        components: &["mesh_renderer", "rigid_body", "sphere_collider"],
        priority: 1,
    },
    EntityPreset {
        id: "static_floor",
        display_name: "Static Floor",
        category: PresetCategory::Physics,
        icon: SQUARE,
        default_name: "Static Floor",
        components: &["mesh_renderer", "rigid_body", "box_collider"],
        priority: 2,
    },
    // Environment
    EntityPreset {
        id: "world_environment",
        display_name: "World Environment",
        category: PresetCategory::Environment,
        icon: GLOBE,
        default_name: "World Environment",
        components: &["world_environment"],
        priority: 0,
    },
    EntityPreset {
        id: "terrain",
        display_name: "Terrain",
        category: PresetCategory::Environment,
        icon: MOUNTAINS,
        default_name: "Terrain",
        components: &["terrain"],
        priority: 1,
    },
    EntityPreset {
        id: "audio_listener",
        display_name: "Audio Listener",
        category: PresetCategory::Environment,
        icon: SPEAKER_HIGH,
        default_name: "Audio Listener",
        components: &["audio_listener"],
        priority: 2,
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

/// Get the set of component type_ids already covered by presets
pub fn preset_component_ids() -> std::collections::HashSet<&'static str> {
    PRESETS.iter()
        .flat_map(|p| p.components.iter().copied())
        .collect()
}

/// Spawn a new entity from a component definition (node = entity + component)
pub fn spawn_component_as_node(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    registry: &ComponentRegistry,
    def: &ComponentDefinition,
    parent: Option<Entity>,
) -> Entity {
    use crate::core::{EditorEntity, SceneNode};

    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: def.display_name.to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    let entity = entity_commands.id();

    (def.add_fn)(commands, entity, meshes, materials);
    entity
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
    use crate::component_system::{MeshNodeData, MeshPrimitiveType};
    use crate::spawn::meshes::create_mesh_for_type;

    // Spawn base entity
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: preset.default_name.to_string(),
            tag: String::new(),
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
        // Handle mesh_renderer specially based on preset ID to create correct mesh type
        if *component_id == "mesh_renderer" {
            let mesh_type = match preset.id {
                "cube" | "rigid_body_cube" => MeshPrimitiveType::Cube,
                "sphere" | "rigid_body_sphere" => MeshPrimitiveType::Sphere,
                "cylinder" => MeshPrimitiveType::Cylinder,
                "plane" | "static_floor" => MeshPrimitiveType::Plane,
                "cone" => MeshPrimitiveType::Cone,
                "torus" => MeshPrimitiveType::Torus,
                "capsule" => MeshPrimitiveType::Capsule,
                "wedge" => MeshPrimitiveType::Wedge,
                "stairs" => MeshPrimitiveType::Stairs,
                "arch" => MeshPrimitiveType::Arch,
                "half_cylinder" => MeshPrimitiveType::HalfCylinder,
                "quarter_pipe" => MeshPrimitiveType::QuarterPipe,
                "corner" => MeshPrimitiveType::Corner,
                "prism" => MeshPrimitiveType::Prism,
                "pyramid" => MeshPrimitiveType::Pyramid,
                "pipe" => MeshPrimitiveType::Pipe,
                "ring" => MeshPrimitiveType::Ring,
                "wall" => MeshPrimitiveType::Wall,
                "ramp" => MeshPrimitiveType::Ramp,
                "hemisphere" => MeshPrimitiveType::Hemisphere,
                "curved_wall" => MeshPrimitiveType::CurvedWall,
                "doorway" => MeshPrimitiveType::Doorway,
                "window_wall" => MeshPrimitiveType::WindowWall,
                "l_shape" => MeshPrimitiveType::LShape,
                "t_shape" => MeshPrimitiveType::TShape,
                "cross_shape" => MeshPrimitiveType::CrossShape,
                "funnel" => MeshPrimitiveType::Funnel,
                "gutter" => MeshPrimitiveType::Gutter,
                "spiral_stairs" => MeshPrimitiveType::SpiralStairs,
                "pillar" => MeshPrimitiveType::Pillar,
                _ => MeshPrimitiveType::Cube,
            };

            let mesh = match preset.id {
                "static_floor" => meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(10.0))),
                _ => create_mesh_for_type(meshes, mesh_type),
            };

            // Offset Y so the mesh bottom sits on the ground (Y=0)
            let ground_offset = match preset.id {
                "static_floor" => 0.0,
                _ => 0.5, // default half-unit offset
            };

            // Physics presets: position dynamic objects above the ground for drop testing
            let y_offset = match preset.id {
                "rigid_body_cube" | "rigid_body_sphere" => 5.0,
                _ => ground_offset,
            };
            commands.entity(entity).insert(
                Transform::from_xyz(0.0, y_offset, 0.0),
            );

            let material = materials.add(StandardMaterial {
                base_color: match preset.id {
                    "static_floor" => Color::srgb(0.45, 0.55, 0.42),
                    _ => Color::srgb(0.8, 0.7, 0.6),
                },
                ..default()
            });

            // Note: RaytracingMesh3d is managed by sync_rendering_settings based on Solari state
            commands.entity(entity).insert((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                MeshNodeData { mesh_type },
            ));
        } else if *component_id == "rigid_body" && preset.id == "static_floor" {
            // Static floor gets a static body instead of the default dynamic body
            use crate::component_system::PhysicsBodyData;
            commands.entity(entity).insert(PhysicsBodyData::static_body());
        } else if *component_id == "box_collider" && preset.id == "static_floor" {
            // Static floor gets a large flat collider
            use crate::component_system::{CollisionShapeData, CollisionShapeType};
            commands.entity(entity).insert(CollisionShapeData {
                shape_type: CollisionShapeType::Box,
                half_extents: Vec3::new(10.0, 0.05, 10.0),
                friction: 0.8,
                ..Default::default()
            });
        } else if let Some(def) = registry.get(component_id) {
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
