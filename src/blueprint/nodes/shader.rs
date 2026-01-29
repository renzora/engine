//! Shader/Material nodes for visual material creation
//!
//! These nodes are used in Material blueprints to create custom shaders.
//! They compile to WGSL instead of Rhai.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// SHADER INPUT NODES
// =============================================================================

/// UV coordinates input
pub static UV: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/uv",
    display_name: "UV",
    category: "Shader Input",
    description: "Get UV texture coordinates (0-1 range)",
    create_pins: || vec![
        Pin::output("uv", "UV", PinType::Vec2),
        Pin::output("u", "U", PinType::Float),
        Pin::output("v", "V", PinType::Float),
    ],
    color: [100, 150, 220], // Blue for shader inputs
    is_event: false,
    is_comment: false,
};

/// World position input
pub static WORLD_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/world_position",
    display_name: "World Position",
    category: "Shader Input",
    description: "Get world-space position of the fragment",
    create_pins: || vec![
        Pin::output("position", "Position", PinType::Vec3),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("z", "Z", PinType::Float),
    ],
    color: [100, 150, 220],
    is_event: false,
    is_comment: false,
};

/// World normal input
pub static WORLD_NORMAL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/world_normal",
    display_name: "World Normal",
    category: "Shader Input",
    description: "Get world-space surface normal",
    create_pins: || vec![
        Pin::output("normal", "Normal", PinType::Vec3),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("z", "Z", PinType::Float),
    ],
    color: [100, 150, 220],
    is_event: false,
    is_comment: false,
};

/// View direction input
pub static VIEW_DIRECTION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/view_direction",
    display_name: "View Direction",
    category: "Shader Input",
    description: "Get direction from fragment to camera",
    create_pins: || vec![
        Pin::output("direction", "Direction", PinType::Vec3),
    ],
    color: [100, 150, 220],
    is_event: false,
    is_comment: false,
};

/// Time input for animated shaders
pub static TIME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/time",
    display_name: "Time",
    category: "Shader Input",
    description: "Get time values for animation",
    create_pins: || vec![
        Pin::output("time", "Time", PinType::Float),
        Pin::output("sin_time", "Sin(Time)", PinType::Float),
        Pin::output("cos_time", "Cos(Time)", PinType::Float),
    ],
    color: [100, 150, 220],
    is_event: false,
    is_comment: false,
};

/// Vertex color input
pub static VERTEX_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/vertex_color",
    display_name: "Vertex Color",
    category: "Shader Input",
    description: "Get vertex color (if mesh has vertex colors)",
    create_pins: || vec![
        Pin::output("color", "Color", PinType::Color),
        Pin::output("r", "R", PinType::Float),
        Pin::output("g", "G", PinType::Float),
        Pin::output("b", "B", PinType::Float),
        Pin::output("a", "A", PinType::Float),
    ],
    color: [100, 150, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// TEXTURE NODES
// =============================================================================
// Note: Texture nodes store their path in input_values["path"] but don't show a pin for it.
// The path is set via drag-drop from the assets panel and displayed as a preview image.

/// Albedo/Color texture (sRGB color space)
pub static TEXTURE_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/texture_color",
    display_name: "Color Texture",
    category: "Shader Texture",
    description: "Albedo/diffuse color texture (sRGB)",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::output("color", "Color", PinType::Color),
        Pin::output("rgb", "RGB", PinType::Vec3),
        Pin::output("a", "Alpha", PinType::Float),
    ],
    color: [200, 150, 120], // Warm orange for color textures
    is_event: false,
    is_comment: false,
};

/// Normal map texture (DirectX format: Y+ down)
pub static TEXTURE_NORMAL_DX: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/texture_normal_dx",
    display_name: "Normal Map (DX)",
    category: "Shader Texture",
    description: "Normal map in DirectX format (green channel Y+ points down)",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("normal", "Normal", PinType::Vec3),
    ],
    color: [120, 120, 200], // Blue for normal maps
    is_event: false,
    is_comment: false,
};

/// Normal map texture (OpenGL format: Y+ up)
pub static TEXTURE_NORMAL_GL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/texture_normal_gl",
    display_name: "Normal Map (GL)",
    category: "Shader Texture",
    description: "Normal map in OpenGL format (green channel Y+ points up)",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("normal", "Normal", PinType::Vec3),
    ],
    color: [100, 140, 200], // Lighter blue for GL normals
    is_event: false,
    is_comment: false,
};

/// Roughness texture (linear grayscale)
pub static TEXTURE_ROUGHNESS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/texture_roughness",
    display_name: "Roughness",
    category: "Shader Texture",
    description: "Roughness/smoothness texture (linear grayscale)",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("invert", "Invert", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("roughness", "Roughness", PinType::Float),
    ],
    color: [140, 140, 140], // Gray for roughness
    is_event: false,
    is_comment: false,
};

/// Metallic texture (linear grayscale)
pub static TEXTURE_METALLIC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/texture_metallic",
    display_name: "Metallic",
    category: "Shader Texture",
    description: "Metallic texture (linear grayscale)",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::output("metallic", "Metallic", PinType::Float),
    ],
    color: [180, 180, 200], // Silver-ish for metallic
    is_event: false,
    is_comment: false,
};

/// Displacement/Height texture
pub static TEXTURE_DISPLACEMENT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/texture_displacement",
    display_name: "Displacement",
    category: "Shader Texture",
    description: "Height/displacement map (linear grayscale)",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("midlevel", "Mid Level", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("height", "Height", PinType::Float),
        Pin::output("vector", "Vector", PinType::Vec3),
    ],
    color: [160, 120, 100], // Brown for displacement
    is_event: false,
    is_comment: false,
};

/// Ambient Occlusion texture
pub static TEXTURE_AO: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/texture_ao",
    display_name: "Ambient Occlusion",
    category: "Shader Texture",
    description: "Ambient occlusion texture (linear grayscale)",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("ao", "AO", PinType::Float),
    ],
    color: [100, 100, 100], // Dark gray for AO
    is_event: false,
    is_comment: false,
};

/// Emissive texture (sRGB, HDR capable)
pub static TEXTURE_EMISSIVE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/texture_emissive",
    display_name: "Emissive",
    category: "Shader Texture",
    description: "Emissive/glow texture",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("intensity", "Intensity", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("emission", "Emission", PinType::Vec3),
    ],
    color: [220, 180, 100], // Yellow/gold for emissive
    is_event: false,
    is_comment: false,
};

/// Opacity/Alpha texture
pub static TEXTURE_OPACITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/texture_opacity",
    display_name: "Opacity",
    category: "Shader Texture",
    description: "Opacity/alpha texture (linear grayscale)",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("invert", "Invert", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("opacity", "Opacity", PinType::Float),
    ],
    color: [180, 180, 180], // Light gray for opacity
    is_event: false,
    is_comment: false,
};

/// Generic texture (for custom use)
pub static TEXTURE_GENERIC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/texture",
    display_name: "Texture",
    category: "Shader Texture",
    description: "Generic texture sampler",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::output("color", "Color", PinType::Color),
        Pin::output("rgb", "RGB", PinType::Vec3),
        Pin::output("r", "R", PinType::Float),
        Pin::output("g", "G", PinType::Float),
        Pin::output("b", "B", PinType::Float),
        Pin::output("a", "A", PinType::Float),
    ],
    color: [150, 120, 200], // Purple for generic textures
    is_event: false,
    is_comment: false,
};

// =============================================================================
// SHADER MATH NODES
// =============================================================================

/// Dot product of two vectors
pub static DOT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/dot",
    display_name: "Dot Product",
    category: "Shader Math",
    description: "Dot product of two vectors",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("b", "B", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [120, 180, 120], // Green for math
    is_event: false,
    is_comment: false,
};

/// Cross product of two vectors
pub static CROSS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/cross",
    display_name: "Cross Product",
    category: "Shader Math",
    description: "Cross product of two vectors",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Vec3).with_default(PinValue::Vec3([1.0, 0.0, 0.0])),
        Pin::input("b", "B", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
        Pin::output("result", "Result", PinType::Vec3),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

/// Normalize a vector
pub static NORMALIZE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/normalize",
    display_name: "Normalize",
    category: "Shader Math",
    description: "Normalize a vector to unit length",
    create_pins: || vec![
        Pin::input("v", "Vector", PinType::Vec3).with_default(PinValue::Vec3([1.0, 0.0, 0.0])),
        Pin::output("result", "Result", PinType::Vec3),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

/// Length of a vector
pub static LENGTH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/length",
    display_name: "Length",
    category: "Shader Math",
    description: "Get the length of a vector",
    create_pins: || vec![
        Pin::input("v", "Vector", PinType::Vec3).with_default(PinValue::Vec3([1.0, 0.0, 0.0])),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

/// Distance between two points
pub static DISTANCE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/distance",
    display_name: "Distance",
    category: "Shader Math",
    description: "Get the distance between two points",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("b", "B", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

/// Reflect vector
pub static REFLECT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/reflect",
    display_name: "Reflect",
    category: "Shader Math",
    description: "Reflect incident vector around normal",
    create_pins: || vec![
        Pin::input("incident", "Incident", PinType::Vec3).with_default(PinValue::Vec3([0.0, -1.0, 0.0])),
        Pin::input("normal", "Normal", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
        Pin::output("result", "Result", PinType::Vec3),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

/// Fresnel effect
pub static FRESNEL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/fresnel",
    display_name: "Fresnel",
    category: "Shader Math",
    description: "Calculate fresnel effect (rim lighting)",
    create_pins: || vec![
        Pin::input("normal", "Normal", PinType::Vec3),
        Pin::input("view", "View Dir", PinType::Vec3),
        Pin::input("power", "Power", PinType::Float).with_default(PinValue::Float(5.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

/// Power function
pub static POW: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/pow",
    display_name: "Power",
    category: "Shader Math",
    description: "Raise base to exponent power",
    create_pins: || vec![
        Pin::input("base", "Base", PinType::Float).with_default(PinValue::Float(2.0)),
        Pin::input("exp", "Exponent", PinType::Float).with_default(PinValue::Float(2.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

/// Smoothstep interpolation
pub static SMOOTHSTEP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/smoothstep",
    display_name: "Smoothstep",
    category: "Shader Math",
    description: "Smooth Hermite interpolation between 0 and 1",
    create_pins: || vec![
        Pin::input("edge0", "Edge 0", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("edge1", "Edge 1", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

/// Step function
pub static STEP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/step",
    display_name: "Step",
    category: "Shader Math",
    description: "Returns 0 if x < edge, else 1",
    create_pins: || vec![
        Pin::input("edge", "Edge", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

/// Fract - fractional part
pub static FRACT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/fract",
    display_name: "Fract",
    category: "Shader Math",
    description: "Get fractional part of a value",
    create_pins: || vec![
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

/// Floor function
pub static FLOOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/floor",
    display_name: "Floor",
    category: "Shader Math",
    description: "Round down to nearest integer",
    create_pins: || vec![
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

/// Ceil function
pub static CEIL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/ceil",
    display_name: "Ceil",
    category: "Shader Math",
    description: "Round up to nearest integer",
    create_pins: || vec![
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

/// One minus (1 - x)
pub static ONE_MINUS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/one_minus",
    display_name: "One Minus",
    category: "Shader Math",
    description: "Returns 1 - x (useful for inverting values)",
    create_pins: || vec![
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

/// Saturate (clamp 0-1)
pub static SATURATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/saturate",
    display_name: "Saturate",
    category: "Shader Math",
    description: "Clamp value between 0 and 1",
    create_pins: || vec![
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// VECTOR OPERATION NODES
// =============================================================================

/// Make Vec2 from components
pub static MAKE_VEC2: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/make_vec2",
    display_name: "Make Vec2",
    category: "Shader Vector",
    description: "Create a Vec2 from X and Y components",
    create_pins: || vec![
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("v", "Vector", PinType::Vec2),
    ],
    color: [200, 180, 100], // Yellow/orange for vectors
    is_event: false,
    is_comment: false,
};

/// Make Vec3 from components
pub static MAKE_VEC3: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/make_vec3",
    display_name: "Make Vec3",
    category: "Shader Vector",
    description: "Create a Vec3 from X, Y, Z components",
    create_pins: || vec![
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("v", "Vector", PinType::Vec3),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Make Vec4 from components
pub static MAKE_VEC4: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/make_vec4",
    display_name: "Make Vec4",
    category: "Shader Vector",
    description: "Create a Vec4 from X, Y, Z, W components",
    create_pins: || vec![
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("w", "W", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("v", "Vector", PinType::Vec4),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Make Color from components
pub static MAKE_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/make_color",
    display_name: "Make Color",
    category: "Shader Vector",
    description: "Create a Color from R, G, B, A components",
    create_pins: || vec![
        Pin::input("r", "R", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("g", "G", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("color", "Color", PinType::Color),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Split Vec2 into components
pub static SPLIT_VEC2: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/split_vec2",
    display_name: "Split Vec2",
    category: "Shader Vector",
    description: "Split a Vec2 into X and Y components",
    create_pins: || vec![
        Pin::input("v", "Vector", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Split Vec3 into components
pub static SPLIT_VEC3: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/split_vec3",
    display_name: "Split Vec3",
    category: "Shader Vector",
    description: "Split a Vec3 into X, Y, Z components",
    create_pins: || vec![
        Pin::input("v", "Vector", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("z", "Z", PinType::Float),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Split Color into components
pub static SPLIT_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/split_color",
    display_name: "Split Color",
    category: "Shader Vector",
    description: "Split a Color into R, G, B, A components",
    create_pins: || vec![
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::output("r", "R", PinType::Float),
        Pin::output("g", "G", PinType::Float),
        Pin::output("b", "B", PinType::Float),
        Pin::output("a", "A", PinType::Float),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Constant color value
pub static COLOR_CONSTANT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/color",
    display_name: "Color",
    category: "Shader Vector",
    description: "A constant color value",
    create_pins: || vec![
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::output("color", "Color", PinType::Color),
        Pin::output("rgb", "RGB", PinType::Vec3),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Constant float value
pub static FLOAT_CONSTANT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/float",
    display_name: "Float",
    category: "Shader Vector",
    description: "A constant float value",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("value", "Value", PinType::Float),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// OUTPUT NODES
// =============================================================================

/// PBR material output
pub static PBR_OUTPUT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/pbr_output",
    display_name: "PBR Output",
    category: "Shader Output",
    description: "Physically-based rendering material output",
    create_pins: || vec![
        Pin::input("base_color", "Base Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("metallic", "Metallic", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("roughness", "Roughness", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("normal", "Normal", PinType::Vec3),
        Pin::input("emissive", "Emissive", PinType::Color).with_default(PinValue::Color([0.0, 0.0, 0.0, 1.0])),
        Pin::input("ao", "Ambient Occlusion", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("alpha", "Alpha", PinType::Float).with_default(PinValue::Float(1.0)),
    ],
    color: [220, 80, 80], // Red for output
    is_event: false,
    is_comment: false,
};

/// Unlit material output (no lighting calculations)
pub static UNLIT_OUTPUT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/unlit_output",
    display_name: "Unlit Output",
    category: "Shader Output",
    description: "Unlit material output (no lighting, just color)",
    create_pins: || vec![
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("alpha", "Alpha", PinType::Float).with_default(PinValue::Float(1.0)),
    ],
    color: [220, 80, 80],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// NOISE/PROCEDURAL NODES
// =============================================================================

/// Simple noise
pub static NOISE_SIMPLE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/noise_simple",
    display_name: "Simple Noise",
    category: "Shader Noise",
    description: "Simple pseudo-random noise",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(10.0)),
        Pin::output("value", "Value", PinType::Float),
    ],
    color: [180, 140, 200], // Purple for procedural
    is_event: false,
    is_comment: false,
};

/// Gradient noise (Perlin-like)
pub static NOISE_GRADIENT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/noise_gradient",
    display_name: "Gradient Noise",
    category: "Shader Noise",
    description: "Smooth gradient noise (Perlin-like)",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(10.0)),
        Pin::output("value", "Value", PinType::Float),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Voronoi/Cellular noise
pub static NOISE_VORONOI: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/noise_voronoi",
    display_name: "Voronoi Noise",
    category: "Shader Noise",
    description: "Cellular/Voronoi noise pattern",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(5.0)),
        Pin::output("distance", "Distance", PinType::Float),
        Pin::output("cell", "Cell ID", PinType::Float),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Checkerboard pattern
pub static CHECKERBOARD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/checkerboard",
    display_name: "Checkerboard",
    category: "Shader Noise",
    description: "Checkerboard pattern",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(2.0)),
        Pin::output("value", "Value", PinType::Float),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Linear gradient
pub static GRADIENT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/gradient",
    display_name: "Gradient",
    category: "Shader Noise",
    description: "Linear gradient along UV direction",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("direction", "Direction", PinType::Vec2).with_default(PinValue::Vec2([0.0, 1.0])),
        Pin::output("value", "Value", PinType::Float),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// COLOR MANIPULATION NODES
// =============================================================================

/// RGB to HSV conversion
pub static RGB_TO_HSV: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/rgb_to_hsv",
    display_name: "RGB to HSV",
    category: "Shader Color",
    description: "Convert RGB color to HSV (Hue, Saturation, Value)",
    create_pins: || vec![
        Pin::input("rgb", "RGB", PinType::Vec3).with_default(PinValue::Vec3([1.0, 1.0, 1.0])),
        Pin::output("hsv", "HSV", PinType::Vec3),
        Pin::output("h", "Hue", PinType::Float),
        Pin::output("s", "Saturation", PinType::Float),
        Pin::output("v", "Value", PinType::Float),
    ],
    color: [220, 120, 180], // Pink for color nodes
    is_event: false,
    is_comment: false,
};

/// HSV to RGB conversion
pub static HSV_TO_RGB: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/hsv_to_rgb",
    display_name: "HSV to RGB",
    category: "Shader Color",
    description: "Convert HSV color to RGB",
    create_pins: || vec![
        Pin::input("h", "Hue", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("s", "Saturation", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("v", "Value", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("rgb", "RGB", PinType::Vec3),
    ],
    color: [220, 120, 180],
    is_event: false,
    is_comment: false,
};

/// Hue shift
pub static HUE_SHIFT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/hue_shift",
    display_name: "Hue Shift",
    category: "Shader Color",
    description: "Shift the hue of a color (0-1 range, wraps around)",
    create_pins: || vec![
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("shift", "Shift", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Color),
    ],
    color: [220, 120, 180],
    is_event: false,
    is_comment: false,
};

/// Saturation adjustment
pub static SATURATION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/saturation",
    display_name: "Saturation",
    category: "Shader Color",
    description: "Adjust color saturation (0 = grayscale, 1 = original, >1 = oversaturated)",
    create_pins: || vec![
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("amount", "Amount", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Color),
    ],
    color: [220, 120, 180],
    is_event: false,
    is_comment: false,
};

/// Brightness adjustment
pub static BRIGHTNESS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/brightness",
    display_name: "Brightness",
    category: "Shader Color",
    description: "Adjust color brightness (additive)",
    create_pins: || vec![
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("amount", "Amount", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Color),
    ],
    color: [220, 120, 180],
    is_event: false,
    is_comment: false,
};

/// Contrast adjustment
pub static CONTRAST: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/contrast",
    display_name: "Contrast",
    category: "Shader Color",
    description: "Adjust color contrast (1 = original, <1 = less contrast, >1 = more contrast)",
    create_pins: || vec![
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("amount", "Amount", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Color),
    ],
    color: [220, 120, 180],
    is_event: false,
    is_comment: false,
};

/// Desaturate (grayscale)
pub static DESATURATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/desaturate",
    display_name: "Desaturate",
    category: "Shader Color",
    description: "Convert color to grayscale using luminance weights",
    create_pins: || vec![
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("amount", "Amount", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Color),
        Pin::output("luminance", "Luminance", PinType::Float),
    ],
    color: [220, 120, 180],
    is_event: false,
    is_comment: false,
};

/// Invert color
pub static INVERT_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/invert_color",
    display_name: "Invert Color",
    category: "Shader Color",
    description: "Invert color (1 - color)",
    create_pins: || vec![
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::output("result", "Result", PinType::Color),
    ],
    color: [220, 120, 180],
    is_event: false,
    is_comment: false,
};

/// Lerp between two colors
pub static LERP_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/lerp_color",
    display_name: "Lerp Color",
    category: "Shader Color",
    description: "Linear interpolation between two colors by T (0 = A, 1 = B)",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Color).with_default(PinValue::Color([0.0, 0.0, 0.0, 1.0])),
        Pin::input("b", "B", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("result", "Result", PinType::Color),
    ],
    color: [220, 120, 180],
    is_event: false,
    is_comment: false,
};

/// Lerp between two Vec3 values
pub static LERP_VEC3: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/lerp_vec3",
    display_name: "Lerp Vec3",
    category: "Shader Math",
    description: "Linear interpolation between two Vec3 values by T (0 = A, 1 = B)",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("b", "B", PinType::Vec3).with_default(PinValue::Vec3([1.0, 1.0, 1.0])),
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("result", "Result", PinType::Vec3),
    ],
    color: [120, 180, 120],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// UV MANIPULATION NODES
// =============================================================================

/// UV Tiling
pub static UV_TILING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/uv_tiling",
    display_name: "UV Tiling",
    category: "Shader UV",
    description: "Tile UV coordinates (repeat texture)",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("tile_x", "Tile X", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("tile_y", "Tile Y", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("uv", "UV", PinType::Vec2),
    ],
    color: [120, 200, 150], // Green for UV nodes
    is_event: false,
    is_comment: false,
};

/// UV Offset
pub static UV_OFFSET: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/uv_offset",
    display_name: "UV Offset",
    category: "Shader UV",
    description: "Offset UV coordinates",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("offset_x", "Offset X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("offset_y", "Offset Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("uv", "UV", PinType::Vec2),
    ],
    color: [120, 200, 150],
    is_event: false,
    is_comment: false,
};

/// UV Rotation
pub static UV_ROTATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/uv_rotate",
    display_name: "UV Rotate",
    category: "Shader UV",
    description: "Rotate UV coordinates around center",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("angle", "Angle", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("center_x", "Center X", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("center_y", "Center Y", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("uv", "UV", PinType::Vec2),
    ],
    color: [120, 200, 150],
    is_event: false,
    is_comment: false,
};

/// UV Flipbook (sprite sheet animation)
pub static UV_FLIPBOOK: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/uv_flipbook",
    display_name: "UV Flipbook",
    category: "Shader UV",
    description: "Animate UV through a sprite sheet grid",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("columns", "Columns", PinType::Float).with_default(PinValue::Float(4.0)),
        Pin::input("rows", "Rows", PinType::Float).with_default(PinValue::Float(4.0)),
        Pin::input("frame", "Frame", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("uv", "UV", PinType::Vec2),
    ],
    color: [120, 200, 150],
    is_event: false,
    is_comment: false,
};

/// Triplanar Mapping
pub static TRIPLANAR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/triplanar",
    display_name: "Triplanar Mapping",
    category: "Shader UV",
    description: "Project texture from 3 planes based on surface normal",
    create_pins: || vec![
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("normal", "Normal", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
        Pin::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("blend", "Blend", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("uv_x", "UV X", PinType::Vec2),
        Pin::output("uv_y", "UV Y", PinType::Vec2),
        Pin::output("uv_z", "UV Z", PinType::Vec2),
        Pin::output("weights", "Weights", PinType::Vec3),
    ],
    color: [120, 200, 150],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// ADVANCED NOISE NODES
// =============================================================================

/// Fractal Brownian Motion noise
pub static NOISE_FBM: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/noise_fbm",
    display_name: "FBM Noise",
    category: "Shader Noise",
    description: "Fractal Brownian Motion - multiple octaves of noise",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("octaves", "Octaves", PinType::Float).with_default(PinValue::Float(4.0)),
        Pin::input("frequency", "Frequency", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("amplitude", "Amplitude", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("lacunarity", "Lacunarity", PinType::Float).with_default(PinValue::Float(2.0)),
        Pin::input("persistence", "Persistence", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("value", "Value", PinType::Float),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Turbulence noise (absolute value FBM)
pub static NOISE_TURBULENCE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/noise_turbulence",
    display_name: "Turbulence",
    category: "Shader Noise",
    description: "Turbulence - FBM with absolute values for more variety",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("octaves", "Octaves", PinType::Float).with_default(PinValue::Float(4.0)),
        Pin::input("frequency", "Frequency", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("amplitude", "Amplitude", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("value", "Value", PinType::Float),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Ridge noise (inverted FBM for mountain-like patterns)
pub static NOISE_RIDGED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/noise_ridged",
    display_name: "Ridged Noise",
    category: "Shader Noise",
    description: "Ridged multifractal noise - sharp peaks like mountains",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("octaves", "Octaves", PinType::Float).with_default(PinValue::Float(4.0)),
        Pin::input("frequency", "Frequency", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("sharpness", "Sharpness", PinType::Float).with_default(PinValue::Float(2.0)),
        Pin::output("value", "Value", PinType::Float),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Domain warping
pub static DOMAIN_WARP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/domain_warp",
    display_name: "Domain Warp",
    category: "Shader Noise",
    description: "Warp UV coordinates using noise for organic distortion",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("frequency", "Frequency", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("amplitude", "Amplitude", PinType::Float).with_default(PinValue::Float(0.1)),
        Pin::input("iterations", "Iterations", PinType::Float).with_default(PinValue::Float(2.0)),
        Pin::output("uv", "UV", PinType::Vec2),
        Pin::output("value", "Value", PinType::Float),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// EFFECT NODES
// =============================================================================

/// Rim lighting effect
pub static RIM_LIGHT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/rim_light",
    display_name: "Rim Light",
    category: "Shader Effect",
    description: "Edge/rim lighting effect based on view angle",
    create_pins: || vec![
        Pin::input("normal", "Normal", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 1.0])),
        Pin::input("view_dir", "View Dir", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 1.0])),
        Pin::input("power", "Power", PinType::Float).with_default(PinValue::Float(2.0)),
        Pin::input("intensity", "Intensity", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("rim", "Rim", PinType::Float),
    ],
    color: [200, 180, 100], // Gold for effect nodes
    is_event: false,
    is_comment: false,
};

/// Parallax mapping
pub static PARALLAX: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/parallax",
    display_name: "Parallax Mapping",
    category: "Shader Effect",
    description: "Offset UV based on height map for depth illusion",
    create_pins: || vec![
        Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("height", "Height", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("view_dir", "View Dir", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 1.0])),
        Pin::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(0.05)),
        Pin::output("uv", "UV", PinType::Vec2),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Normal blend
pub static NORMAL_BLEND: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/normal_blend",
    display_name: "Blend Normals",
    category: "Shader Effect",
    description: "Blend two normal maps together (Reoriented Normal Mapping)",
    create_pins: || vec![
        Pin::input("base", "Base", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 1.0])),
        Pin::input("detail", "Detail", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 1.0])),
        Pin::output("result", "Result", PinType::Vec3),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Detail texture blending
pub static DETAIL_BLEND: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/detail_blend",
    display_name: "Detail Blend",
    category: "Shader Effect",
    description: "Blend detail texture over base using overlay mode",
    create_pins: || vec![
        Pin::input("base", "Base", PinType::Vec3).with_default(PinValue::Vec3([0.5, 0.5, 0.5])),
        Pin::input("detail", "Detail", PinType::Vec3).with_default(PinValue::Vec3([0.5, 0.5, 0.5])),
        Pin::input("amount", "Amount", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Vec3),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Posterize effect
pub static POSTERIZE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "shader/posterize",
    display_name: "Posterize",
    category: "Shader Effect",
    description: "Reduce color levels for a posterized look",
    create_pins: || vec![
        Pin::input("color", "Color", PinType::Vec3).with_default(PinValue::Vec3([1.0, 1.0, 1.0])),
        Pin::input("levels", "Levels", PinType::Float).with_default(PinValue::Float(4.0)),
        Pin::output("result", "Result", PinType::Vec3),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};
