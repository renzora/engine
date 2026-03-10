//! Material node type definitions and registry.
//!
//! Each node type declares its pins, category, and display info.
//! The WGSL codegen uses node_type strings to dispatch code generation.

use crate::graph::{PinTemplate, PinType, PinValue};

// ── Node type definition ────────────────────────────────────────────────────

pub struct MaterialNodeDef {
    pub node_type: &'static str,
    pub display_name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub pins: fn() -> Vec<PinTemplate>,
    /// RGB header color for the node in the graph editor.
    pub color: [u8; 3],
}

// ── Category constants ──────────────────────────────────────────────────────

pub const CAT_INPUT: &str = "Input";
pub const CAT_TEXTURE: &str = "Texture";
pub const CAT_MATH: &str = "Math";
pub const CAT_VECTOR: &str = "Vector";
pub const CAT_COLOR: &str = "Color";
pub const CAT_PROCEDURAL: &str = "Procedural";
pub const CAT_ANIMATION: &str = "Animation";
pub const CAT_UTILITY: &str = "Utility";
pub const CAT_OUTPUT: &str = "Output";

// ── Color constants for categories ──────────────────────────────────────────

const CLR_INPUT: [u8; 3] = [100, 150, 220];
const CLR_TEXTURE: [u8; 3] = [200, 150, 120];
const CLR_MATH: [u8; 3] = [120, 120, 120];
const CLR_VECTOR: [u8; 3] = [127, 204, 25];
const CLR_COLOR: [u8; 3] = [220, 170, 80];
const CLR_PROCEDURAL: [u8; 3] = [160, 100, 200];
const CLR_ANIMATION: [u8; 3] = [80, 200, 180];
const CLR_UTILITY: [u8; 3] = [140, 140, 160];
const CLR_OUTPUT: [u8; 3] = [200, 60, 60];

// =============================================================================
// INPUT NODES
// =============================================================================

pub static UV: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/uv",
    display_name: "UV",
    category: CAT_INPUT,
    description: "Texture coordinates (0-1)",
    pins: || vec![
        PinTemplate::output("uv", "UV", PinType::Vec2),
        PinTemplate::output("u", "U", PinType::Float),
        PinTemplate::output("v", "V", PinType::Float),
    ],
    color: CLR_INPUT,
};

pub static WORLD_POSITION: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/world_position",
    display_name: "World Position",
    category: CAT_INPUT,
    description: "Fragment world-space position",
    pins: || vec![
        PinTemplate::output("position", "Position", PinType::Vec3),
        PinTemplate::output("x", "X", PinType::Float),
        PinTemplate::output("y", "Y", PinType::Float),
        PinTemplate::output("z", "Z", PinType::Float),
    ],
    color: CLR_INPUT,
};

pub static WORLD_NORMAL: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/world_normal",
    display_name: "World Normal",
    category: CAT_INPUT,
    description: "Fragment world-space normal",
    pins: || vec![
        PinTemplate::output("normal", "Normal", PinType::Vec3),
        PinTemplate::output("x", "X", PinType::Float),
        PinTemplate::output("y", "Y", PinType::Float),
        PinTemplate::output("z", "Z", PinType::Float),
    ],
    color: CLR_INPUT,
};

pub static VIEW_DIRECTION: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/view_direction",
    display_name: "View Direction",
    category: CAT_INPUT,
    description: "Direction from fragment to camera",
    pins: || vec![PinTemplate::output("direction", "Direction", PinType::Vec3)],
    color: CLR_INPUT,
};

pub static TIME: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/time",
    display_name: "Time",
    category: CAT_INPUT,
    description: "Time values for animation",
    pins: || vec![
        PinTemplate::output("time", "Time", PinType::Float),
        PinTemplate::output("sin_time", "Sin(Time)", PinType::Float),
        PinTemplate::output("cos_time", "Cos(Time)", PinType::Float),
    ],
    color: CLR_INPUT,
};

pub static VERTEX_COLOR: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/vertex_color",
    display_name: "Vertex Color",
    category: CAT_INPUT,
    description: "Per-vertex color attribute",
    pins: || vec![
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("r", "R", PinType::Float),
        PinTemplate::output("g", "G", PinType::Float),
        PinTemplate::output("b", "B", PinType::Float),
        PinTemplate::output("a", "A", PinType::Float),
    ],
    color: CLR_INPUT,
};

pub static CAMERA_POSITION: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/camera_position",
    display_name: "Camera Position",
    category: CAT_INPUT,
    description: "World-space camera position",
    pins: || vec![PinTemplate::output("position", "Position", PinType::Vec3)],
    color: CLR_INPUT,
};

pub static OBJECT_POSITION: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/object_position",
    display_name: "Object Position",
    category: CAT_INPUT,
    description: "Object pivot world position (for wind anchoring, etc.)",
    pins: || vec![PinTemplate::output("position", "Position", PinType::Vec3)],
    color: CLR_INPUT,
};

// =============================================================================
// TEXTURE NODES
// =============================================================================

pub static SAMPLE_TEXTURE: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/sample",
    display_name: "Sample Texture",
    category: CAT_TEXTURE,
    description: "Sample a 2D texture at UV coordinates",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
        PinTemplate::output("r", "R", PinType::Float),
        PinTemplate::output("g", "G", PinType::Float),
        PinTemplate::output("b", "B", PinType::Float),
        PinTemplate::output("a", "Alpha", PinType::Float),
    ],
    color: CLR_TEXTURE,
};

pub static SAMPLE_NORMAL: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/sample_normal",
    display_name: "Sample Normal Map",
    category: CAT_TEXTURE,
    description: "Sample and decode a normal map texture",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("normal", "Normal", PinType::Vec3),
    ],
    color: [120, 120, 200],
};

pub static TRIPLANAR_SAMPLE: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/triplanar",
    display_name: "Triplanar Sample",
    category: CAT_TEXTURE,
    description: "Sample texture using triplanar projection (no UV seams)",
    pins: || vec![
        PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("sharpness", "Sharpness", PinType::Float).with_default(PinValue::Float(2.0)),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
    ],
    color: CLR_TEXTURE,
};

// =============================================================================
// MATH NODES
// =============================================================================

pub static ADD: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/add",
    display_name: "Add",
    category: CAT_MATH,
    description: "A + B",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static SUBTRACT: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/subtract",
    display_name: "Subtract",
    category: CAT_MATH,
    description: "A - B",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static MULTIPLY: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/multiply",
    display_name: "Multiply",
    category: CAT_MATH,
    description: "A * B",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static DIVIDE: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/divide",
    display_name: "Divide",
    category: CAT_MATH,
    description: "A / B",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static POWER: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/power",
    display_name: "Power",
    category: CAT_MATH,
    description: "Base ^ Exponent",
    pins: || vec![
        PinTemplate::input("base", "Base", PinType::Float).with_default(PinValue::Float(2.0)),
        PinTemplate::input("exp", "Exponent", PinType::Float).with_default(PinValue::Float(2.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static ABS: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/abs",
    display_name: "Abs",
    category: CAT_MATH,
    description: "Absolute value",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static NEGATE: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/negate",
    display_name: "Negate",
    category: CAT_MATH,
    description: "-Value",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static ONE_MINUS: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/one_minus",
    display_name: "One Minus",
    category: CAT_MATH,
    description: "1.0 - Value",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static FRACT: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/fract",
    display_name: "Fract",
    category: CAT_MATH,
    description: "Fractional part",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static FLOOR: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/floor",
    display_name: "Floor",
    category: CAT_MATH,
    description: "Round down to integer",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static CEIL: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/ceil",
    display_name: "Ceil",
    category: CAT_MATH,
    description: "Round up to integer",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static MIN: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/min",
    display_name: "Min",
    category: CAT_MATH,
    description: "Minimum of A and B",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static MAX: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/max",
    display_name: "Max",
    category: CAT_MATH,
    description: "Maximum of A and B",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static CLAMP: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/clamp",
    display_name: "Clamp",
    category: CAT_MATH,
    description: "Clamp value between min and max",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("min", "Min", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("max", "Max", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static LERP: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/lerp",
    display_name: "Lerp",
    category: CAT_MATH,
    description: "Linear interpolation: mix(A, B, T)",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("t", "T", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static SMOOTHSTEP: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/smoothstep",
    display_name: "Smoothstep",
    category: CAT_MATH,
    description: "Hermite interpolation between edge0 and edge1",
    pins: || vec![
        PinTemplate::input("edge0", "Edge 0", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("edge1", "Edge 1", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static STEP: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/step",
    display_name: "Step",
    category: CAT_MATH,
    description: "0.0 if value < edge, 1.0 otherwise",
    pins: || vec![
        PinTemplate::input("edge", "Edge", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static REMAP: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/remap",
    display_name: "Remap",
    category: CAT_MATH,
    description: "Remap value from [in_min, in_max] to [out_min, out_max]",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("in_min", "In Min", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("in_max", "In Max", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("out_min", "Out Min", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("out_max", "Out Max", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static SIN: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/sin",
    display_name: "Sin",
    category: CAT_MATH,
    description: "Sine function",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static COS: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/cos",
    display_name: "Cos",
    category: CAT_MATH,
    description: "Cosine function",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static SATURATE: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/saturate",
    display_name: "Saturate",
    category: CAT_MATH,
    description: "Clamp to 0.0 - 1.0",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

// =============================================================================
// VECTOR NODES
// =============================================================================

pub static SPLIT_VEC2: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/split_vec2",
    display_name: "Split Vec2",
    category: CAT_VECTOR,
    description: "Split Vec2 into components",
    pins: || vec![
        PinTemplate::input("vector", "Vector", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::output("x", "X", PinType::Float),
        PinTemplate::output("y", "Y", PinType::Float),
    ],
    color: CLR_VECTOR,
};

pub static SPLIT_VEC3: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/split_vec3",
    display_name: "Split Vec3",
    category: CAT_VECTOR,
    description: "Split Vec3 into components",
    pins: || vec![
        PinTemplate::input("vector", "Vector", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::output("x", "X", PinType::Float),
        PinTemplate::output("y", "Y", PinType::Float),
        PinTemplate::output("z", "Z", PinType::Float),
    ],
    color: CLR_VECTOR,
};

pub static COMBINE_VEC2: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/combine_vec2",
    display_name: "Combine Vec2",
    category: CAT_VECTOR,
    description: "Create Vec2 from components",
    pins: || vec![
        PinTemplate::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("vector", "Vector", PinType::Vec2),
    ],
    color: CLR_VECTOR,
};

pub static COMBINE_VEC3: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/combine_vec3",
    display_name: "Combine Vec3",
    category: CAT_VECTOR,
    description: "Create Vec3 from components",
    pins: || vec![
        PinTemplate::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("vector", "Vector", PinType::Vec3),
    ],
    color: CLR_VECTOR,
};

pub static COMBINE_VEC4: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/combine_vec4",
    display_name: "Combine Vec4",
    category: CAT_VECTOR,
    description: "Create Vec4 from components",
    pins: || vec![
        PinTemplate::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("w", "W", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("vector", "Vector", PinType::Vec4),
    ],
    color: CLR_VECTOR,
};

pub static DOT: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/dot",
    display_name: "Dot Product",
    category: CAT_VECTOR,
    description: "Dot product of two vectors",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::input("b", "B", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_VECTOR,
};

pub static CROSS: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/cross",
    display_name: "Cross Product",
    category: CAT_VECTOR,
    description: "Cross product of two Vec3",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Vec3).with_default(PinValue::Vec3([1.0, 0.0, 0.0])),
        PinTemplate::input("b", "B", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
        PinTemplate::output("result", "Result", PinType::Vec3),
    ],
    color: CLR_VECTOR,
};

pub static NORMALIZE: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/normalize",
    display_name: "Normalize",
    category: CAT_VECTOR,
    description: "Normalize vector to unit length",
    pins: || vec![
        PinTemplate::input("vector", "Vector", PinType::Vec3).with_default(PinValue::Vec3([1.0, 0.0, 0.0])),
        PinTemplate::output("result", "Result", PinType::Vec3),
    ],
    color: CLR_VECTOR,
};

pub static DISTANCE: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/distance",
    display_name: "Distance",
    category: CAT_VECTOR,
    description: "Distance between two points",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::input("b", "B", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_VECTOR,
};

pub static LENGTH: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/length",
    display_name: "Length",
    category: CAT_VECTOR,
    description: "Vector magnitude",
    pins: || vec![
        PinTemplate::input("vector", "Vector", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_VECTOR,
};

pub static REFLECT: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/reflect",
    display_name: "Reflect",
    category: CAT_VECTOR,
    description: "Reflect vector about normal",
    pins: || vec![
        PinTemplate::input("incident", "Incident", PinType::Vec3),
        PinTemplate::input("normal", "Normal", PinType::Vec3),
        PinTemplate::output("result", "Result", PinType::Vec3),
    ],
    color: CLR_VECTOR,
};

// =============================================================================
// COLOR NODES
// =============================================================================

pub static COLOR_CONSTANT: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/constant",
    display_name: "Color",
    category: CAT_COLOR,
    description: "Constant color value",
    pins: || vec![
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
        PinTemplate::output("r", "R", PinType::Float),
        PinTemplate::output("g", "G", PinType::Float),
        PinTemplate::output("b", "B", PinType::Float),
        PinTemplate::output("a", "A", PinType::Float),
    ],
    color: CLR_COLOR,
};

pub static FLOAT_CONSTANT: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/float",
    display_name: "Float",
    category: CAT_COLOR,
    description: "Constant float value",
    pins: || vec![
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_COLOR,
};

pub static VEC2_CONSTANT: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/vec2",
    display_name: "Vec2",
    category: CAT_COLOR,
    description: "Constant Vec2 value",
    pins: || vec![
        PinTemplate::output("value", "Value", PinType::Vec2),
    ],
    color: CLR_COLOR,
};

pub static VEC3_CONSTANT: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/vec3",
    display_name: "Vec3",
    category: CAT_COLOR,
    description: "Constant Vec3 value",
    pins: || vec![
        PinTemplate::output("value", "Value", PinType::Vec3),
    ],
    color: CLR_COLOR,
};

pub static COLOR_LERP: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/lerp",
    display_name: "Color Lerp",
    category: CAT_COLOR,
    description: "Blend between two colors",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Color).with_default(PinValue::Color([0.0, 0.0, 0.0, 1.0])),
        PinTemplate::input("b", "B", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        PinTemplate::input("t", "T", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::output("color", "Color", PinType::Color),
    ],
    color: CLR_COLOR,
};

pub static COSINE_PALETTE: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/cosine_palette",
    display_name: "Cosine Palette",
    category: CAT_COLOR,
    description: "IQ cosine color palette: a + b * cos(2π(c*t + d))",
    pins: || vec![
        PinTemplate::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("a", "Bias", PinType::Vec3).with_default(PinValue::Vec3([0.5, 0.5, 0.5])),
        PinTemplate::input("b", "Amplitude", PinType::Vec3).with_default(PinValue::Vec3([0.5, 0.5, 0.5])),
        PinTemplate::input("c", "Frequency", PinType::Vec3).with_default(PinValue::Vec3([1.0, 1.0, 1.0])),
        PinTemplate::input("d", "Phase", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.33, 0.67])),
        PinTemplate::output("color", "Color", PinType::Vec3),
    ],
    color: CLR_COLOR,
};

pub static FRESNEL: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/fresnel",
    display_name: "Fresnel",
    category: CAT_COLOR,
    description: "View-angle dependent effect (water edges, rim light)",
    pins: || vec![
        PinTemplate::input("power", "Power", PinType::Float).with_default(PinValue::Float(5.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_COLOR,
};

// =============================================================================
// PROCEDURAL NODES
// =============================================================================

pub static NOISE_PERLIN: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_perlin",
    display_name: "Perlin Noise",
    category: CAT_PROCEDURAL,
    description: "Smooth gradient noise",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(10.0)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_SIMPLEX: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_simplex",
    display_name: "Simplex Noise",
    category: CAT_PROCEDURAL,
    description: "Fast gradient noise with fewer directional artifacts",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(10.0)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_VORONOI: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_voronoi",
    display_name: "Voronoi",
    category: CAT_PROCEDURAL,
    description: "Cell/Worley noise",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(5.0)),
        PinTemplate::output("distance", "Distance", PinType::Float),
        PinTemplate::output("cell_id", "Cell ID", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_FBM: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_fbm",
    display_name: "FBM Noise",
    category: CAT_PROCEDURAL,
    description: "Fractal Brownian Motion (layered noise)",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(5.0)),
        PinTemplate::input("octaves", "Octaves", PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::input("lacunarity", "Lacunarity", PinType::Float).with_default(PinValue::Float(2.0)),
        PinTemplate::input("persistence", "Persistence", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static CHECKERBOARD: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/checkerboard",
    display_name: "Checkerboard",
    category: CAT_PROCEDURAL,
    description: "Alternating pattern",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(8.0)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static GRADIENT: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/gradient",
    display_name: "Gradient",
    category: CAT_PROCEDURAL,
    description: "Linear gradient (0-1) along U or V",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::output("u", "U", PinType::Float),
        PinTemplate::output("v", "V", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static BRICK: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/brick",
    display_name: "Brick",
    category: CAT_PROCEDURAL,
    description: "Brick/tile pattern with mortar",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Vec2).with_default(PinValue::Vec2([4.0, 8.0])),
        PinTemplate::input("mortar", "Mortar Width", PinType::Float).with_default(PinValue::Float(0.05)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static NORMAL_FROM_HEIGHT: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/normal_from_height",
    display_name: "Normal From Height",
    category: CAT_PROCEDURAL,
    description: "Derive normal vector from a height/noise value via screen-space derivatives",
    pins: || vec![
        PinTemplate::input("height", "Height", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("normal", "Normal", PinType::Vec3),
    ],
    color: CLR_PROCEDURAL,
};

// =============================================================================
// ANIMATION NODES
// =============================================================================

pub static UV_SCROLL: MaterialNodeDef = MaterialNodeDef {
    node_type: "animation/uv_scroll",
    display_name: "UV Scroll",
    category: CAT_ANIMATION,
    description: "Scroll UV coordinates over time",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("speed", "Speed", PinType::Vec2).with_default(PinValue::Vec2([0.1, 0.0])),
        PinTemplate::output("uv", "UV", PinType::Vec2),
    ],
    color: CLR_ANIMATION,
};

pub static FLOW_MAP: MaterialNodeDef = MaterialNodeDef {
    node_type: "animation/flow_map",
    display_name: "Flow Map",
    category: CAT_ANIMATION,
    description: "Two-phase UV distortion with crossfade (realistic water flow)",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("flow", "Flow Direction", PinType::Vec2).with_default(PinValue::Vec2([0.1, 0.0])),
        PinTemplate::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(0.1)),
        PinTemplate::output("uv1", "UV Phase 1", PinType::Vec2),
        PinTemplate::output("uv2", "UV Phase 2", PinType::Vec2),
        PinTemplate::output("blend", "Blend", PinType::Float),
    ],
    color: CLR_ANIMATION,
};

pub static SINE_WAVE: MaterialNodeDef = MaterialNodeDef {
    node_type: "animation/sine_wave",
    display_name: "Sine Wave",
    category: CAT_ANIMATION,
    description: "Animated sine oscillation",
    pins: || vec![
        PinTemplate::input("frequency", "Frequency", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("amplitude", "Amplitude", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("offset", "Offset", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_ANIMATION,
};

pub static PING_PONG: MaterialNodeDef = MaterialNodeDef {
    node_type: "animation/ping_pong",
    display_name: "Ping Pong",
    category: CAT_ANIMATION,
    description: "Triangular wave (0→1→0 repeat)",
    pins: || vec![
        PinTemplate::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_ANIMATION,
};

pub static WIND: MaterialNodeDef = MaterialNodeDef {
    node_type: "animation/wind",
    display_name: "Wind",
    category: CAT_ANIMATION,
    description: "Wind displacement for vegetation (vertex domain)",
    pins: || vec![
        PinTemplate::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(0.3)),
        PinTemplate::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("direction", "Direction", PinType::Vec2).with_default(PinValue::Vec2([1.0, 0.0])),
        PinTemplate::input("turbulence", "Turbulence", PinType::Float).with_default(PinValue::Float(0.2)),
        PinTemplate::input("mask", "Mask", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("displacement", "Displacement", PinType::Vec3),
    ],
    color: CLR_ANIMATION,
};

// =============================================================================
// UTILITY NODES
// =============================================================================

pub static WORLD_POSITION_MASK: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/world_pos_mask",
    display_name: "World Position Mask",
    category: CAT_UTILITY,
    description: "Mask by world Y height (snow on peaks, etc.)",
    pins: || vec![
        PinTemplate::input("height", "Height", PinType::Float).with_default(PinValue::Float(10.0)),
        PinTemplate::input("falloff", "Falloff", PinType::Float).with_default(PinValue::Float(2.0)),
        PinTemplate::output("mask", "Mask", PinType::Float),
    ],
    color: CLR_UTILITY,
};

pub static SLOPE_MASK: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/slope_mask",
    display_name: "Slope Mask",
    category: CAT_UTILITY,
    description: "Mask by surface slope angle (cliffs vs flat ground)",
    pins: || vec![
        PinTemplate::input("threshold", "Threshold", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("falloff", "Falloff", PinType::Float).with_default(PinValue::Float(0.2)),
        PinTemplate::output("mask", "Mask", PinType::Float),
    ],
    color: CLR_UTILITY,
};

pub static DEPTH_FADE: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/depth_fade",
    display_name: "Depth Fade",
    category: CAT_UTILITY,
    description: "Fade based on scene depth difference (water shore foam)",
    pins: || vec![
        PinTemplate::input("distance", "Distance", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("fade", "Fade", PinType::Float),
    ],
    color: CLR_UTILITY,
};

// =============================================================================
// OUTPUT NODES
// =============================================================================

pub static OUTPUT_SURFACE: MaterialNodeDef = MaterialNodeDef {
    node_type: "output/surface",
    display_name: "Surface Output",
    category: CAT_OUTPUT,
    description: "PBR surface material output",
    pins: || vec![
        PinTemplate::input("base_color", "Base Color", PinType::Color).with_default(PinValue::Color([0.8, 0.8, 0.8, 1.0])),
        PinTemplate::input("metallic", "Metallic", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("roughness", "Roughness", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("normal", "Normal", PinType::Vec3),
        PinTemplate::input("emissive", "Emissive", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::input("ao", "AO", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("alpha", "Alpha", PinType::Float).with_default(PinValue::Float(1.0)),
    ],
    color: CLR_OUTPUT,
};

pub static OUTPUT_TERRAIN_LAYER: MaterialNodeDef = MaterialNodeDef {
    node_type: "output/terrain_layer",
    display_name: "Terrain Layer Output",
    category: CAT_OUTPUT,
    description: "Terrain layer material (blended via splatmap)",
    pins: || vec![
        PinTemplate::input("base_color", "Base Color", PinType::Color).with_default(PinValue::Color([0.5, 0.5, 0.5, 1.0])),
        PinTemplate::input("metallic", "Metallic", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("roughness", "Roughness", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("normal", "Normal", PinType::Vec3),
        PinTemplate::input("height", "Height", PinType::Float).with_default(PinValue::Float(0.5)),
    ],
    color: CLR_OUTPUT,
};

pub static OUTPUT_VEGETATION: MaterialNodeDef = MaterialNodeDef {
    node_type: "output/vegetation",
    display_name: "Vegetation Output",
    category: CAT_OUTPUT,
    description: "PBR surface + vertex displacement",
    pins: || vec![
        PinTemplate::input("base_color", "Base Color", PinType::Color).with_default(PinValue::Color([0.2, 0.5, 0.1, 1.0])),
        PinTemplate::input("metallic", "Metallic", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("roughness", "Roughness", PinType::Float).with_default(PinValue::Float(0.7)),
        PinTemplate::input("normal", "Normal", PinType::Vec3),
        PinTemplate::input("emissive", "Emissive", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::input("ao", "AO", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("alpha", "Alpha", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("vertex_offset", "Vertex Offset", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
    ],
    color: CLR_OUTPUT,
};

pub static OUTPUT_UNLIT: MaterialNodeDef = MaterialNodeDef {
    node_type: "output/unlit",
    display_name: "Unlit Output",
    category: CAT_OUTPUT,
    description: "Unlit color output (no lighting)",
    pins: || vec![
        PinTemplate::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        PinTemplate::input("alpha", "Alpha", PinType::Float).with_default(PinValue::Float(1.0)),
    ],
    color: CLR_OUTPUT,
};

// =============================================================================
// REGISTRY
// =============================================================================

/// All available material node types.
pub static ALL_NODES: &[&MaterialNodeDef] = &[
    // Input
    &UV, &WORLD_POSITION, &WORLD_NORMAL, &VIEW_DIRECTION, &TIME, &VERTEX_COLOR,
    &CAMERA_POSITION, &OBJECT_POSITION,
    // Texture
    &SAMPLE_TEXTURE, &SAMPLE_NORMAL, &TRIPLANAR_SAMPLE,
    // Math
    &ADD, &SUBTRACT, &MULTIPLY, &DIVIDE, &POWER, &ABS, &NEGATE, &ONE_MINUS,
    &FRACT, &FLOOR, &CEIL, &MIN, &MAX, &CLAMP, &LERP, &SMOOTHSTEP, &STEP,
    &REMAP, &SIN, &COS, &SATURATE,
    // Vector
    &SPLIT_VEC2, &SPLIT_VEC3, &COMBINE_VEC2, &COMBINE_VEC3, &COMBINE_VEC4,
    &DOT, &CROSS, &NORMALIZE, &DISTANCE, &LENGTH, &REFLECT,
    // Color
    &COLOR_CONSTANT, &FLOAT_CONSTANT, &VEC2_CONSTANT, &VEC3_CONSTANT,
    &COLOR_LERP, &COSINE_PALETTE, &FRESNEL,
    // Procedural
    &NOISE_PERLIN, &NOISE_SIMPLEX, &NOISE_VORONOI, &NOISE_FBM,
    &CHECKERBOARD, &GRADIENT, &BRICK, &NORMAL_FROM_HEIGHT,
    // Animation
    &UV_SCROLL, &FLOW_MAP, &SINE_WAVE, &PING_PONG, &WIND,
    // Utility
    &WORLD_POSITION_MASK, &SLOPE_MASK, &DEPTH_FADE,
    // Output
    &OUTPUT_SURFACE, &OUTPUT_TERRAIN_LAYER, &OUTPUT_VEGETATION, &OUTPUT_UNLIT,
];

/// Get all unique categories in display order.
pub fn categories() -> Vec<&'static str> {
    vec![
        CAT_INPUT, CAT_TEXTURE, CAT_MATH, CAT_VECTOR, CAT_COLOR,
        CAT_PROCEDURAL, CAT_ANIMATION, CAT_UTILITY, CAT_OUTPUT,
    ]
}

/// Get all node definitions in a category.
pub fn nodes_in_category(category: &str) -> Vec<&'static MaterialNodeDef> {
    ALL_NODES
        .iter()
        .copied()
        .filter(|n| n.category == category)
        .collect()
}

/// Look up a node definition by type string.
pub fn node_def(node_type: &str) -> Option<&'static MaterialNodeDef> {
    ALL_NODES.iter().copied().find(|n| n.node_type == node_type)
}
