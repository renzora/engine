//! Rendering nodes
//!
//! Nodes for meshes, materials, lights, visibility, and visual effects.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// MESH OPERATIONS
// =============================================================================

/// Spawn mesh
pub static SPAWN_MESH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/spawn_mesh",
    display_name: "Spawn Mesh",
    category: "Rendering",
    description: "Spawn an entity with a mesh",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("mesh", "Mesh", PinType::Asset),
        Pin::input("material", "Material", PinType::Asset),
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Set mesh
pub static SET_MESH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_mesh",
    display_name: "Set Mesh",
    category: "Rendering",
    description: "Change the mesh of an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("mesh", "Mesh", PinType::Asset),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Spawn primitive
pub static SPAWN_PRIMITIVE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/spawn_primitive",
    display_name: "Spawn Primitive",
    category: "Rendering",
    description: "Spawn a primitive shape (cube, sphere, etc.)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("shape", "Shape", PinType::String).with_default(PinValue::String("cube".into())),
        Pin::input("size", "Size", PinType::Vec3).with_default(PinValue::Vec3([1.0, 1.0, 1.0])),
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// MATERIAL OPERATIONS
// =============================================================================

/// Set material
pub static SET_MATERIAL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_material",
    display_name: "Set Material",
    category: "Rendering",
    description: "Change the material of an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("material", "Material", PinType::Asset),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Set color
pub static SET_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_color",
    display_name: "Set Color",
    category: "Rendering",
    description: "Set the base color of an entity's material",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Get color
pub static GET_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/get_color",
    display_name: "Get Color",
    category: "Rendering",
    description: "Get the base color of an entity's material",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("color", "Color", PinType::Color),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Set emissive
pub static SET_EMISSIVE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_emissive",
    display_name: "Set Emissive",
    category: "Rendering",
    description: "Set the emissive color of an entity's material",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([0.0, 0.0, 0.0, 1.0])),
        Pin::input("intensity", "Intensity", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Set metallic/roughness
pub static SET_PBR_PROPERTIES: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_pbr",
    display_name: "Set PBR Properties",
    category: "Rendering",
    description: "Set PBR material properties",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("metallic", "Metallic", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("roughness", "Roughness", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Set texture
pub static SET_TEXTURE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_texture",
    display_name: "Set Texture",
    category: "Rendering",
    description: "Set a texture on an entity's material",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("texture", "Texture", PinType::Asset),
        Pin::input("slot", "Slot", PinType::String).with_default(PinValue::String("base_color".into())),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// VISIBILITY
// =============================================================================

/// Set visibility
pub static SET_VISIBILITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_visibility",
    display_name: "Set Visibility",
    category: "Rendering",
    description: "Set entity visibility",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("visible", "Visible", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Get visibility
pub static GET_VISIBILITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/get_visibility",
    display_name: "Get Visibility",
    category: "Rendering",
    description: "Get entity visibility",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("visible", "Visible", PinType::Bool),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Toggle visibility
pub static TOGGLE_VISIBILITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/toggle_visibility",
    display_name: "Toggle Visibility",
    category: "Rendering",
    description: "Toggle entity visibility",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("visible", "New State", PinType::Bool),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// LIGHTS
// =============================================================================

/// Spawn point light
pub static SPAWN_POINT_LIGHT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/spawn_point_light",
    display_name: "Spawn Point Light",
    category: "Lights",
    description: "Spawn a point light",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 5.0, 0.0])),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("intensity", "Intensity", PinType::Float).with_default(PinValue::Float(1000.0)),
        Pin::input("range", "Range", PinType::Float).with_default(PinValue::Float(20.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [220, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Spawn spot light
pub static SPAWN_SPOT_LIGHT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/spawn_spot_light",
    display_name: "Spawn Spot Light",
    category: "Lights",
    description: "Spawn a spot light",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 5.0, 0.0])),
        Pin::input("direction", "Direction", PinType::Vec3).with_default(PinValue::Vec3([0.0, -1.0, 0.0])),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("intensity", "Intensity", PinType::Float).with_default(PinValue::Float(1000.0)),
        Pin::input("range", "Range", PinType::Float).with_default(PinValue::Float(20.0)),
        Pin::input("inner_angle", "Inner Angle", PinType::Float).with_default(PinValue::Float(30.0)),
        Pin::input("outer_angle", "Outer Angle", PinType::Float).with_default(PinValue::Float(45.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [220, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Spawn directional light
pub static SPAWN_DIRECTIONAL_LIGHT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/spawn_directional_light",
    display_name: "Spawn Directional Light",
    category: "Lights",
    description: "Spawn a directional light (sun)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("direction", "Direction", PinType::Vec3).with_default(PinValue::Vec3([-0.5, -1.0, -0.5])),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("intensity", "Intensity", PinType::Float).with_default(PinValue::Float(10000.0)),
        Pin::input("shadows", "Shadows", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [220, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Set light color
pub static SET_LIGHT_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_light_color",
    display_name: "Set Light Color",
    category: "Lights",
    description: "Set the color of a light",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Set light intensity
pub static SET_LIGHT_INTENSITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_light_intensity",
    display_name: "Set Light Intensity",
    category: "Lights",
    description: "Set the intensity of a light",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("intensity", "Intensity", PinType::Float).with_default(PinValue::Float(1000.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Set light range
pub static SET_LIGHT_RANGE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_light_range",
    display_name: "Set Light Range",
    category: "Lights",
    description: "Set the range of a point/spot light",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("range", "Range", PinType::Float).with_default(PinValue::Float(20.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Set light shadows
pub static SET_LIGHT_SHADOWS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_light_shadows",
    display_name: "Set Light Shadows",
    category: "Lights",
    description: "Enable or disable shadows for a light",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("enabled", "Enabled", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 200, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// ENVIRONMENT
// =============================================================================

/// Set ambient light
pub static SET_AMBIENT_LIGHT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_ambient",
    display_name: "Set Ambient Light",
    category: "Lights",
    description: "Set the global ambient light color and intensity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([0.1, 0.1, 0.1, 1.0])),
        Pin::input("brightness", "Brightness", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Set fog
pub static SET_FOG: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_fog",
    display_name: "Set Fog",
    category: "Rendering",
    description: "Configure distance fog",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([0.5, 0.5, 0.5, 1.0])),
        Pin::input("near", "Near", PinType::Float).with_default(PinValue::Float(10.0)),
        Pin::input("far", "Far", PinType::Float).with_default(PinValue::Float(100.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

/// Set skybox
pub static SET_SKYBOX: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_skybox",
    display_name: "Set Skybox",
    category: "Rendering",
    description: "Set the skybox texture",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("texture", "Texture", PinType::Asset),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [160, 120, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// 2D SPRITES
// =============================================================================

/// Spawn sprite
pub static SPAWN_SPRITE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/spawn_sprite",
    display_name: "Spawn Sprite",
    category: "2D",
    description: "Spawn a 2D sprite",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("texture", "Texture", PinType::Asset),
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [180, 140, 100],
    is_event: false,
    is_comment: false,
};

/// Set sprite
pub static SET_SPRITE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_sprite",
    display_name: "Set Sprite",
    category: "2D",
    description: "Change the texture of a sprite",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("texture", "Texture", PinType::Asset),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 100],
    is_event: false,
    is_comment: false,
};

/// Set sprite color
pub static SET_SPRITE_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_sprite_color",
    display_name: "Set Sprite Color",
    category: "2D",
    description: "Set sprite tint color",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 100],
    is_event: false,
    is_comment: false,
};

/// Set sprite flip
pub static SET_SPRITE_FLIP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/set_sprite_flip",
    display_name: "Set Sprite Flip",
    category: "2D",
    description: "Flip sprite horizontally or vertically",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("flip_x", "Flip X", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::input("flip_y", "Flip Y", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// PARTICLES (Future)
// =============================================================================

/// Spawn particle system
pub static SPAWN_PARTICLES: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/spawn_particles",
    display_name: "Spawn Particle System",
    category: "Particles",
    description: "Spawn a particle system",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("effect", "Effect", PinType::Asset),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Play particles
pub static PLAY_PARTICLES: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/play_particles",
    display_name: "Play Particles",
    category: "Particles",
    description: "Start playing a particle system",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Stop particles
pub static STOP_PARTICLES: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "rendering/stop_particles",
    display_name: "Stop Particles",
    category: "Particles",
    description: "Stop a particle system",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("clear", "Clear", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};
