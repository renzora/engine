//! Material node type definitions and registry.
//!
//! Each node type declares its pins, category, and display info.
//! The WGSL codegen uses node_type strings to dispatch code generation.

use super::graph::{PinTemplate, PinType, PinValue};

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
pub const CAT_CONTROL: &str = "Control";
pub const CAT_SCENE: &str = "Scene";
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
const CLR_CONTROL: [u8; 3] = [200, 200, 80];
const CLR_SCENE: [u8; 3] = [100, 180, 220];
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

pub static UV_SCALE: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/uv_scale",
    display_name: "UV Scale",
    category: CAT_INPUT,
    description: "Scale and offset UV coordinates for tiling",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Vec2).with_default(PinValue::Vec2([2.0, 2.0])),
        PinTemplate::input("offset", "Offset", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::output("uv", "UV", PinType::Vec2),
    ],
    color: CLR_INPUT,
};

pub static UV_POLAR: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/uv_polar",
    display_name: "Polar UV",
    category: CAT_INPUT,
    description: "Convert Cartesian UV to polar (x=angle [0..1], y=radius). Used for radial effects, spirals, pies.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("center", "Center", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
        PinTemplate::output("uv", "Polar UV", PinType::Vec2),
        PinTemplate::output("angle", "Angle", PinType::Float),
        PinTemplate::output("radius", "Radius", PinType::Float),
    ],
    color: CLR_INPUT,
};

pub static UV_ROTATOR: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/uv_rotator",
    display_name: "UV Rotator",
    category: CAT_INPUT,
    description: "Rotate UV coordinates around a center point (angle in radians)",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("angle", "Angle", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("center", "Center", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
        PinTemplate::output("uv", "UV", PinType::Vec2),
    ],
    color: CLR_INPUT,
};

pub static UV_PANNER: MaterialNodeDef = MaterialNodeDef {
    node_type: "input/uv_panner",
    display_name: "UV Panner",
    category: CAT_INPUT,
    description: "Time-driven UV pan with an arbitrary direction (matches Unreal's Panner node)",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("speed", "Speed", PinType::Vec2).with_default(PinValue::Vec2([0.1, 0.0])),
        PinTemplate::input("time_offset", "Time Offset", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("uv", "UV", PinType::Vec2),
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

pub static SAMPLE_TEXTURE_LOD: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/sample_lod",
    display_name: "Sample Texture LOD",
    category: CAT_TEXTURE,
    description: "Sample a 2D texture at an explicit mip level (textureSampleLevel). Use to blur reflections/refractions with a Roughness-driven LOD, or to sample at mip 0 from inside a loop/branch where automatic derivatives aren't valid.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("lod", "LOD", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
        PinTemplate::output("r", "R", PinType::Float),
        PinTemplate::output("g", "G", PinType::Float),
        PinTemplate::output("b", "B", PinType::Float),
        PinTemplate::output("a", "Alpha", PinType::Float),
    ],
    color: CLR_TEXTURE,
};

pub static SAMPLE_TEXTURE_GRAD: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/sample_grad",
    display_name: "Sample Texture Grad",
    category: CAT_TEXTURE,
    description: "Sample a 2D texture with explicit UV derivatives (textureSampleGrad). Fixes mip-selection bias when UVs are rotated, polar-warped, or otherwise transformed in ways that fool the default derivatives — produces crisp anisotropic filtering.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("ddx", "dUV/dx", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("ddy", "dUV/dy", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
        PinTemplate::output("r", "R", PinType::Float),
        PinTemplate::output("g", "G", PinType::Float),
        PinTemplate::output("b", "B", PinType::Float),
        PinTemplate::output("a", "Alpha", PinType::Float),
    ],
    color: CLR_TEXTURE,
};

pub static SAMPLE_CUBEMAP: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/sample_cubemap",
    display_name: "Sample Cubemap",
    category: CAT_TEXTURE,
    description: "Sample a user-supplied cubemap texture along a direction vector. Separate from Environment Map Sample — this goes to a material-local cube slot so one graph can reference its own skybox / reflection-probe / stylized sky without disturbing the scene's IBL. LOD controls glossiness (0 = sharpest mip).",
    pins: || vec![
        PinTemplate::input("direction", "Direction", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
        PinTemplate::input("lod", "LOD", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
        PinTemplate::output("a", "Alpha", PinType::Float),
    ],
    color: CLR_TEXTURE,
};

pub static SAMPLE_2D_ARRAY: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/sample_2d_array",
    display_name: "Sample 2D Array",
    category: CAT_TEXTURE,
    description: "Sample a layered 2D texture array — UV picks the in-layer position, Layer Index picks which layer. Use for terrain layer stacks, asset-variant atlases (e.g. same character body with multiple skins), paletted materials, mask banks. Layer Index is rounded to the nearest integer layer.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("layer", "Layer", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
        PinTemplate::output("r", "R", PinType::Float),
        PinTemplate::output("g", "G", PinType::Float),
        PinTemplate::output("b", "B", PinType::Float),
        PinTemplate::output("a", "Alpha", PinType::Float),
    ],
    color: CLR_TEXTURE,
};

pub static SAMPLE_3D: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/sample_3d",
    display_name: "Sample 3D Texture",
    category: CAT_TEXTURE,
    description: "Sample a volumetric (3D) texture at a UVW coordinate. Use for volume-fog density, caustic LUTs, precomputed scattering tables, 3D noise bakes, LUT color-grading tables. UVW in [0..1]^3 indexes directly into the volume.",
    pins: || vec![
        PinTemplate::input("uvw", "UVW", PinType::Vec3).with_default(PinValue::Vec3([0.5, 0.5, 0.5])),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
        PinTemplate::output("r", "R", PinType::Float),
        PinTemplate::output("g", "G", PinType::Float),
        PinTemplate::output("b", "B", PinType::Float),
        PinTemplate::output("a", "Alpha", PinType::Float),
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

pub static MODULO: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/modulo",
    display_name: "Modulo",
    category: CAT_MATH,
    description: "A mod B (floating-point remainder)",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static SIGN: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/sign",
    display_name: "Sign",
    category: CAT_MATH,
    description: "-1 / 0 / +1 based on sign of value",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static ATAN2: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/atan2",
    display_name: "Atan2",
    category: CAT_MATH,
    description: "Two-argument arctangent: atan2(y, x) in radians",
    pins: || vec![
        PinTemplate::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("x", "X", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static TRUNC: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/trunc",
    display_name: "Trunc",
    category: CAT_MATH,
    description: "Truncate toward zero",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static ROUND: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/round",
    display_name: "Round",
    category: CAT_MATH,
    description: "Round to nearest integer",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static EXP: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/exp",
    display_name: "Exp",
    category: CAT_MATH,
    description: "Natural exponential: e^x",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static LOG: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/log",
    display_name: "Log",
    category: CAT_MATH,
    description: "Natural logarithm: ln(x)",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static SQRT: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/sqrt",
    display_name: "Sqrt",
    category: CAT_MATH,
    description: "Square root",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static RECIPROCAL: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/reciprocal",
    display_name: "Reciprocal",
    category: CAT_MATH,
    description: "1 / value",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static TAN: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/tan",
    display_name: "Tan",
    category: CAT_MATH,
    description: "Tangent",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static ASIN: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/asin",
    display_name: "Asin",
    category: CAT_MATH,
    description: "Arcsine in radians",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static ACOS: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/acos",
    display_name: "Acos",
    category: CAT_MATH,
    description: "Arccosine in radians",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static RADIANS: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/radians",
    display_name: "To Radians",
    category: CAT_MATH,
    description: "Convert degrees → radians",
    pins: || vec![
        PinTemplate::input("value", "Degrees", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Radians", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static DEGREES: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/degrees",
    display_name: "To Degrees",
    category: CAT_MATH,
    description: "Convert radians → degrees",
    pins: || vec![
        PinTemplate::input("value", "Radians", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Degrees", PinType::Float),
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

pub static REFRACT: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/refract",
    display_name: "Refract",
    category: CAT_VECTOR,
    description: "Refract incident vector through surface with index-of-refraction ratio",
    pins: || vec![
        PinTemplate::input("incident", "Incident", PinType::Vec3),
        PinTemplate::input("normal", "Normal", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
        PinTemplate::input("eta", "IOR Ratio", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Vec3),
    ],
    color: CLR_VECTOR,
};

pub static SWIZZLE: MaterialNodeDef = MaterialNodeDef {
    node_type: "vector/swizzle",
    display_name: "Swizzle",
    category: CAT_VECTOR,
    description: "Rearrange vec4 components. Pick 0=X, 1=Y, 2=Z, 3=W, 4=zero, 5=one for each output channel.",
    pins: || vec![
        PinTemplate::input("vector", "Vector", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 1.0])),
        PinTemplate::input("out_x", "Out X", PinType::Float).with_default(PinValue::Int(0)),
        PinTemplate::input("out_y", "Out Y", PinType::Float).with_default(PinValue::Int(1)),
        PinTemplate::input("out_z", "Out Z", PinType::Float).with_default(PinValue::Int(2)),
        PinTemplate::input("out_w", "Out W", PinType::Float).with_default(PinValue::Int(3)),
        PinTemplate::output("vector", "Vector", PinType::Vec4),
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

pub static SRGB_TO_LINEAR: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/srgb_to_linear",
    display_name: "sRGB → Linear",
    category: CAT_COLOR,
    description: "Convert sRGB-encoded color to linear (piecewise)",
    pins: || vec![
        PinTemplate::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        PinTemplate::output("result", "Result", PinType::Color),
    ],
    color: CLR_COLOR,
};

pub static LINEAR_TO_SRGB: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/linear_to_srgb",
    display_name: "Linear → sRGB",
    category: CAT_COLOR,
    description: "Convert linear color to sRGB (piecewise)",
    pins: || vec![
        PinTemplate::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        PinTemplate::output("result", "Result", PinType::Color),
    ],
    color: CLR_COLOR,
};

pub static RGB_TO_HSV: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/rgb_to_hsv",
    display_name: "RGB → HSV",
    category: CAT_COLOR,
    description: "Convert RGB to HSV (hue/saturation/value)",
    pins: || vec![
        PinTemplate::input("rgb", "RGB", PinType::Vec3).with_default(PinValue::Vec3([1.0, 0.0, 0.0])),
        PinTemplate::output("hsv", "HSV", PinType::Vec3),
        PinTemplate::output("h", "H", PinType::Float),
        PinTemplate::output("s", "S", PinType::Float),
        PinTemplate::output("v", "V", PinType::Float),
    ],
    color: CLR_COLOR,
};

pub static HSV_TO_RGB: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/hsv_to_rgb",
    display_name: "HSV → RGB",
    category: CAT_COLOR,
    description: "Convert HSV to RGB",
    pins: || vec![
        PinTemplate::input("hsv", "HSV", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 1.0])),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
    ],
    color: CLR_COLOR,
};

pub static HUE_SHIFT: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/hue_shift",
    display_name: "Hue Shift",
    category: CAT_COLOR,
    description: "Rotate the hue of an RGB color by a given amount (0-1)",
    pins: || vec![
        PinTemplate::input("rgb", "RGB", PinType::Vec3).with_default(PinValue::Vec3([1.0, 0.0, 0.0])),
        PinTemplate::input("shift", "Shift", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
    ],
    color: CLR_COLOR,
};

pub static LUMINANCE: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/luminance",
    display_name: "Luminance",
    category: CAT_COLOR,
    description: "Rec.709 luminance of an RGB color",
    pins: || vec![
        PinTemplate::input("rgb", "RGB", PinType::Vec3).with_default(PinValue::Vec3([1.0, 1.0, 1.0])),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_COLOR,
};

pub static GAMMA: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/gamma",
    display_name: "Gamma",
    category: CAT_COLOR,
    description: "Apply pow(color, gamma) per channel",
    pins: || vec![
        PinTemplate::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        PinTemplate::input("gamma", "Gamma", PinType::Float).with_default(PinValue::Float(2.2)),
        PinTemplate::output("result", "Result", PinType::Color),
    ],
    color: CLR_COLOR,
};

pub static BRIGHTNESS_CONTRAST: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/brightness_contrast",
    display_name: "Brightness / Contrast",
    category: CAT_COLOR,
    description: "Adjust brightness (additive) and contrast (around 0.5 gray)",
    pins: || vec![
        PinTemplate::input("color", "Color", PinType::Color).with_default(PinValue::Color([0.5, 0.5, 0.5, 1.0])),
        PinTemplate::input("brightness", "Brightness", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("contrast", "Contrast", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Color),
    ],
    color: CLR_COLOR,
};

pub static SATURATION: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/saturation",
    display_name: "Saturation",
    category: CAT_COLOR,
    description: "Adjust saturation (0 = greyscale, 1 = original, >1 = supersaturated)",
    pins: || vec![
        PinTemplate::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        PinTemplate::input("saturation", "Saturation", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Color),
    ],
    color: CLR_COLOR,
};

pub static BLEND: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/blend",
    display_name: "Blend",
    category: CAT_COLOR,
    description: "Blend mode composite. Mode: 0=normal, 1=multiply, 2=screen, 3=overlay, 4=add, 5=subtract, 6=soft-light, 7=hard-light, 8=difference, 9=divide",
    pins: || vec![
        PinTemplate::input("base", "Base", PinType::Color).with_default(PinValue::Color([0.5, 0.5, 0.5, 1.0])),
        PinTemplate::input("blend", "Blend", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        PinTemplate::input("opacity", "Opacity", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("mode", "Mode", PinType::Float).with_default(PinValue::Int(0)),
        PinTemplate::output("result", "Result", PinType::Color),
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
    description: "Cell/Worley noise with F1, F2, edge-distance and cell-id outputs",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(5.0)),
        PinTemplate::output("distance", "F1 (nearest)", PinType::Float),
        PinTemplate::output("f2", "F2 (2nd nearest)", PinType::Float),
        PinTemplate::output("edge", "Edge Distance", PinType::Float),
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
    display_name: "Normal From Height (tangent)",
    category: CAT_PROCEDURAL,
    description: "Derive a tangent-space normal from a height value via screen-space derivatives. Output is in tangent frame (Z = up). For materials that plug into a pbr_input.N hook and expect world-space, use `world_normal_from_height` instead.",
    pins: || vec![
        PinTemplate::input("height", "Height", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("normal", "Normal", PinType::Vec3),
    ],
    color: CLR_PROCEDURAL,
};

pub static WORLD_NORMAL_FROM_HEIGHT: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/world_normal_from_height",
    display_name: "World Normal From Height",
    category: CAT_PROCEDURAL,
    description: "Derive a WORLD-space perturbed normal from a height value, reconstructing a tangent frame per-fragment from the screen-space derivatives of world_position. Works on any surface orientation (horizontal lake, tilted river, sculpted terrain). Feed directly into Surface Output's `normal` pin for water / stone / procedural displacement.",
    pins: || vec![
        PinTemplate::input("height", "Height", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("normal", "Normal", PinType::Vec3),
    ],
    color: CLR_PROCEDURAL,
};

pub static DOMAIN_WARP: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/domain_warp",
    display_name: "Domain Warp",
    category: CAT_PROCEDURAL,
    description: "Distort UV coordinates using FBM noise as an offset vector. Produces organic cloud / marble / fluid shapes.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Noise Scale", PinType::Float).with_default(PinValue::Float(1.5)),
        PinTemplate::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(0.35)),
        PinTemplate::input("offset", "Offset", PinType::Vec2).with_default(PinValue::Vec2([5.2, 1.3])),
        PinTemplate::output("uv", "Warped UV", PinType::Vec2),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_RIDGED: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_ridged",
    display_name: "Ridged FBM",
    category: CAT_PROCEDURAL,
    description: "Ridged multifractal — sharp crests for cumulus billows, mountain ridges, cracks",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::input("octaves", "Octaves", PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::input("lacunarity", "Lacunarity", PinType::Float).with_default(PinValue::Float(2.0)),
        PinTemplate::input("persistence", "Persistence", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_TURBULENCE: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_turbulence",
    display_name: "Turbulence",
    category: CAT_PROCEDURAL,
    description: "Fire / smoke / turbulent flow (|noise| accumulated across octaves)",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::input("octaves", "Octaves", PinType::Float).with_default(PinValue::Float(5.0)),
        PinTemplate::input("lacunarity", "Lacunarity", PinType::Float).with_default(PinValue::Float(2.0)),
        PinTemplate::input("persistence", "Persistence", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_BILLOW: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_billow",
    display_name: "Billow Noise",
    category: CAT_PROCEDURAL,
    description: "Puffy, rounded shapes (|noise|² accumulated) — great for cumulus clouds and stone pores",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::input("octaves", "Octaves", PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::input("lacunarity", "Lacunarity", PinType::Float).with_default(PinValue::Float(2.0)),
        PinTemplate::input("persistence", "Persistence", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_WHITE: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_white",
    display_name: "White Noise",
    category: CAT_PROCEDURAL,
    description: "Uncorrelated random values at every UV coordinate (grain, sparkle)",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(50.0)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_CURL: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_curl",
    display_name: "Curl Noise",
    category: CAT_PROCEDURAL,
    description: "Divergence-free 2D flow field — ideal for fluid-like advection and swirly UV distortion",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(3.0)),
        PinTemplate::input("epsilon", "Epsilon", PinType::Float).with_default(PinValue::Float(0.01)),
        PinTemplate::output("flow", "Flow", PinType::Vec2),
    ],
    color: CLR_PROCEDURAL,
};

pub static GRADIENT_RADIAL: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/gradient_radial",
    display_name: "Radial Gradient",
    category: CAT_PROCEDURAL,
    description: "0 at center → 1 at `radius`, with soft falloff",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
        PinTemplate::input("center", "Center", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
        PinTemplate::input("radius", "Radius", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("softness", "Softness", PinType::Float).with_default(PinValue::Float(0.3)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static GRADIENT_LINEAR: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/gradient_linear",
    display_name: "Linear Gradient",
    category: CAT_PROCEDURAL,
    description: "Gradient along a direction (angle in radians, 0 = +x)",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
        PinTemplate::input("angle", "Angle", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("center", "Center", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static GRADIENT_ANGULAR: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/gradient_angular",
    display_name: "Angular Gradient",
    category: CAT_PROCEDURAL,
    description: "0-1 sweeping around a center point (for pie / compass / clock effects)",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
        PinTemplate::input("center", "Center", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
        PinTemplate::input("offset", "Start Offset", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static GRADIENT_DIAMOND: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/gradient_diamond",
    display_name: "Diamond Gradient",
    category: CAT_PROCEDURAL,
    description: "Diamond-shaped falloff (L1 / Manhattan distance) around a center",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
        PinTemplate::input("center", "Center", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
        PinTemplate::input("size", "Size", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static BUMP_OFFSET: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/bump_offset",
    display_name: "Bump Offset",
    category: CAT_PROCEDURAL,
    description: "Simple parallax: displace UVs along view vector by a height value. Cheap fake depth.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("height", "Height", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("reference", "Reference", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(0.05)),
        PinTemplate::output("uv", "Offset UV", PinType::Vec2),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_TRIPLANAR_FBM: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_triplanar_fbm",
    display_name: "Triplanar FBM",
    category: CAT_PROCEDURAL,
    description: "World-space FBM projected onto X/Y/Z planes and blended by world normal. No UV, no seams, works on any mesh topology (spheres, terrain, sculpts).",
    pins: || vec![
        PinTemplate::input("scale",       "Scale",       PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("octaves",     "Octaves",     PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::input("lacunarity",  "Lacunarity",  PinType::Float).with_default(PinValue::Float(2.0)),
        PinTemplate::input("persistence", "Persistence", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("sharpness",   "Sharpness",   PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_TRIPLANAR_RIDGED: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_triplanar_ridged",
    display_name: "Triplanar Ridged",
    category: CAT_PROCEDURAL,
    description: "Ridged FBM sampled triplanar — seamless mountain/cumulus ridges on any topology.",
    pins: || vec![
        PinTemplate::input("scale",       "Scale",       PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("octaves",     "Octaves",     PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::input("lacunarity",  "Lacunarity",  PinType::Float).with_default(PinValue::Float(2.0)),
        PinTemplate::input("persistence", "Persistence", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("sharpness",   "Sharpness",   PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_TRIPLANAR_TURBULENCE: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_triplanar_turbulence",
    display_name: "Triplanar Turbulence",
    category: CAT_PROCEDURAL,
    description: "Turbulence noise sampled triplanar — seamless fire/smoke/flow on any topology.",
    pins: || vec![
        PinTemplate::input("scale",       "Scale",       PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("octaves",     "Octaves",     PinType::Float).with_default(PinValue::Float(5.0)),
        PinTemplate::input("lacunarity",  "Lacunarity",  PinType::Float).with_default(PinValue::Float(2.0)),
        PinTemplate::input("persistence", "Persistence", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("sharpness",   "Sharpness",   PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_TRIPLANAR_BILLOW: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_triplanar_billow",
    display_name: "Triplanar Billow",
    category: CAT_PROCEDURAL,
    description: "Billow noise sampled triplanar — seamless puffy cumulus / stone-pore shapes on any topology.",
    pins: || vec![
        PinTemplate::input("scale",       "Scale",       PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("octaves",     "Octaves",     PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::input("lacunarity",  "Lacunarity",  PinType::Float).with_default(PinValue::Float(2.0)),
        PinTemplate::input("persistence", "Persistence", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("sharpness",   "Sharpness",   PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_TRIPLANAR_VORONOI: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_triplanar_voronoi",
    display_name: "Triplanar Voronoi",
    category: CAT_PROCEDURAL,
    description: "Voronoi cells sampled triplanar — seamless cracked-surface / cell pattern on any mesh.",
    pins: || vec![
        PinTemplate::input("scale",     "Scale",     PinType::Float).with_default(PinValue::Float(3.0)),
        PinTemplate::input("sharpness", "Sharpness", PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::output("distance", "F1", PinType::Float),
        PinTemplate::output("cell_id", "Cell ID", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static HEX_TILE: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/hex_tile",
    display_name: "Hex Tile UV",
    category: CAT_PROCEDURAL,
    description: "Hexagonal anti-tiling: decomposes UV space into hex cells, randomly rotates each cell's UV, and blends three overlapping hex samples together. Feed the output UV into a Sample Texture node to break up visible repetition on a single tiled texture. The `variation` pin controls random rotation strength (0 = plain tiling, 1 = maximum scramble).",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("variation", "Variation", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("uv1", "UV A", PinType::Vec2),
        PinTemplate::output("uv2", "UV B", PinType::Vec2),
        PinTemplate::output("uv3", "UV C", PinType::Vec2),
        PinTemplate::output("w1", "Weight A", PinType::Float),
        PinTemplate::output("w2", "Weight B", PinType::Float),
        PinTemplate::output("w3", "Weight C", PinType::Float),
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

pub static FLIPBOOK_UV: MaterialNodeDef = MaterialNodeDef {
    node_type: "animation/flipbook_uv",
    display_name: "Flipbook UV",
    category: CAT_ANIMATION,
    description: "Compute the sub-rectangle UV for a single frame of a sprite-sheet/flipbook texture laid out on a `cols × rows` grid. Output feeds a Sample Texture node. Drive `frame` by time*fps for animated sprites, or by an integer to pick a specific tile.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("frame", "Frame", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("cols", "Columns", PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::input("rows", "Rows", PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::output("uv", "UV", PinType::Vec2),
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

pub static DPDX: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/dpdx",
    display_name: "DDX",
    category: CAT_UTILITY,
    description: "Screen-space derivative along X (rate of change horizontally)",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_UTILITY,
};

pub static DPDY: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/dpdy",
    display_name: "DDY",
    category: CAT_UTILITY,
    description: "Screen-space derivative along Y (rate of change vertically)",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_UTILITY,
};

pub static FWIDTH: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/fwidth",
    display_name: "FWidth",
    category: CAT_UTILITY,
    description: "abs(dpdx) + abs(dpdy) — pixel footprint for anti-aliasing",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_UTILITY,
};

pub static DITHER: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/dither",
    display_name: "Dither",
    category: CAT_UTILITY,
    description: "Screen-space Bayer dither (4x4) for transparency-to-coverage",
    pins: || vec![
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_UTILITY,
};

pub static HASH: MaterialNodeDef = MaterialNodeDef {
    node_type: "utility/hash",
    display_name: "Hash",
    category: CAT_UTILITY,
    description: "Deterministic 0-1 hash of a vec2 input (white-noise style)",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_UTILITY,
};

// =============================================================================
// CONTROL NODES
// =============================================================================

pub static IF_NODE: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/if",
    display_name: "If",
    category: CAT_CONTROL,
    description: "Runtime branch: when `condition > threshold`, outputs `if_true`, else `if_false`. Both branches always execute (use Static Switch for permutation-style branching).",
    pins: || vec![
        PinTemplate::input("condition", "Condition", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("threshold", "Threshold", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("if_true",  "True",  PinType::Vec4).with_default(PinValue::Vec4([1.0, 1.0, 1.0, 1.0])),
        PinTemplate::input("if_false", "False", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 1.0])),
        PinTemplate::output("result", "Result", PinType::Vec4),
    ],
    color: CLR_CONTROL,
};

pub static STATIC_SWITCH: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/static_switch",
    display_name: "Static Switch",
    category: CAT_CONTROL,
    description: "Compile-time branch. Only the selected input's nodes are emitted in the shader — the unused branch is stripped. Set `use_a` (Bool) in the node's input_values.",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Vec4).with_default(PinValue::Vec4([1.0, 1.0, 1.0, 1.0])),
        PinTemplate::input("b", "B", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 1.0])),
        PinTemplate::input("use_a", "Use A", PinType::Bool).with_default(PinValue::Bool(true)),
        PinTemplate::output("result", "Result", PinType::Vec4),
    ],
    color: CLR_CONTROL,
};

pub static COMPONENT_MASK: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/component_mask",
    display_name: "Component Mask",
    category: CAT_CONTROL,
    description: "Zero specific channels of a vec4. Toggle R/G/B/A booleans to keep or drop each channel. Equivalent to Unreal's ComponentMask.",
    pins: || vec![
        PinTemplate::input("vector", "Vector", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 1.0])),
        PinTemplate::input("keep_r", "R", PinType::Bool).with_default(PinValue::Bool(true)),
        PinTemplate::input("keep_g", "G", PinType::Bool).with_default(PinValue::Bool(true)),
        PinTemplate::input("keep_b", "B", PinType::Bool).with_default(PinValue::Bool(true)),
        PinTemplate::input("keep_a", "A", PinType::Bool).with_default(PinValue::Bool(false)),
        PinTemplate::output("vector", "Vector", PinType::Vec4),
    ],
    color: CLR_CONTROL,
};

pub static GREATER_THAN: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/greater_than",
    display_name: "Greater Than",
    category: CAT_CONTROL,
    description: "Returns 1.0 if A > B, else 0.0",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_CONTROL,
};

pub static LESS_THAN: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/less_than",
    display_name: "Less Than",
    category: CAT_CONTROL,
    description: "Returns 1.0 if A < B, else 0.0",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_CONTROL,
};

pub static EQUAL: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/equal",
    display_name: "Equal",
    category: CAT_CONTROL,
    description: "Returns 1.0 if |A - B| < epsilon, else 0.0",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("epsilon", "Epsilon", PinType::Float).with_default(PinValue::Float(0.001)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_CONTROL,
};

pub static NOT_EQUAL: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/not_equal",
    display_name: "Not Equal",
    category: CAT_CONTROL,
    description: "Returns 1.0 if |A - B| >= epsilon, else 0.0",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("epsilon", "Epsilon", PinType::Float).with_default(PinValue::Float(0.001)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_CONTROL,
};

pub static AND_NODE: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/and",
    display_name: "And",
    category: CAT_CONTROL,
    description: "Logical AND on float booleans: min(A, B) — returns 1.0 only if both A and B are 1.0",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_CONTROL,
};

pub static OR_NODE: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/or",
    display_name: "Or",
    category: CAT_CONTROL,
    description: "Logical OR on float booleans: max(A, B) — returns 1.0 if either A or B is 1.0",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_CONTROL,
};

pub static NOT_NODE: MaterialNodeDef = MaterialNodeDef {
    node_type: "control/not",
    display_name: "Not",
    category: CAT_CONTROL,
    description: "Logical NOT on float boolean: 1 - value",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_CONTROL,
};

// =============================================================================
// FUNCTION NODES  (material subgraphs)
// =============================================================================

pub static FUNCTION_INPUT_POINT: MaterialNodeDef = MaterialNodeDef {
    node_type: "function/input_point",
    display_name: "Function Inputs",
    category: CAT_CONTROL,
    description: "Inside a material function only: outputs in_0..in_3 (the function's call-site inputs as Vec4). Use split_vec* nodes to unpack scalars.",
    pins: || vec![
        PinTemplate::output("in_0", "In 0", PinType::Vec4),
        PinTemplate::output("in_1", "In 1", PinType::Vec4),
        PinTemplate::output("in_2", "In 2", PinType::Vec4),
        PinTemplate::output("in_3", "In 3", PinType::Vec4),
    ],
    color: CLR_CONTROL,
};

pub static FUNCTION_OUTPUT_POINT: MaterialNodeDef = MaterialNodeDef {
    node_type: "function/output_point",
    display_name: "Function Outputs",
    category: CAT_CONTROL,
    description: "Inside a material function only: receives out_0..out_3 (what the function returns at the call site).",
    pins: || vec![
        PinTemplate::input("out_0", "Out 0", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
        PinTemplate::input("out_1", "Out 1", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
        PinTemplate::input("out_2", "Out 2", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
        PinTemplate::input("out_3", "Out 3", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
    ],
    color: CLR_CONTROL,
};

pub static FUNCTION_CALL: MaterialNodeDef = MaterialNodeDef {
    node_type: "function/call",
    display_name: "Function Call",
    category: CAT_CONTROL,
    description: "Invoke a reusable material function by name. Set input_values[\"function\"] to the function's name (a file in assets/material_functions/).",
    pins: || vec![
        PinTemplate::input("in_0", "In 0", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
        PinTemplate::input("in_1", "In 1", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
        PinTemplate::input("in_2", "In 2", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
        PinTemplate::input("in_3", "In 3", PinType::Vec4).with_default(PinValue::Vec4([0.0, 0.0, 0.0, 0.0])),
        PinTemplate::output("out_0", "Out 0", PinType::Vec4),
        PinTemplate::output("out_1", "Out 1", PinType::Vec4),
        PinTemplate::output("out_2", "Out 2", PinType::Vec4),
        PinTemplate::output("out_3", "Out 3", PinType::Vec4),
    ],
    color: CLR_CONTROL,
};

// =============================================================================
// SCENE NODES  (depth/scene integration)
// =============================================================================

pub static PIXEL_DEPTH: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/pixel_depth",
    display_name: "Pixel Depth",
    category: CAT_SCENE,
    description: "Linear view-space depth of this fragment (distance from camera in scene units).",
    pins: || vec![
        PinTemplate::output("depth", "Depth", PinType::Float),
    ],
    color: CLR_SCENE,
};

pub static SCENE_DEPTH: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/scene_depth",
    display_name: "Scene Depth",
    category: CAT_SCENE,
    description: "Reads the opaque-pass depth buffer at this fragment (linear view space). Requires DepthPrepass on the camera; returns a large value when prepass is unavailable.",
    pins: || vec![
        PinTemplate::output("depth", "Depth", PinType::Float),
    ],
    color: CLR_SCENE,
};

pub static SCENE_DEPTH_FADE: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/depth_fade",
    display_name: "Scene Depth Fade",
    category: CAT_SCENE,
    description: "Proximity fade to nearest opaque surface: 0 at contact, 1 when scene is at least `distance` units behind. For shoreline foam, soft intersection, etc.",
    pins: || vec![
        PinTemplate::input("distance", "Distance", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("fade", "Fade", PinType::Float),
    ],
    color: CLR_SCENE,
};

pub static SCENE_NORMAL: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/scene_normal",
    display_name: "Scene Normal",
    category: CAT_SCENE,
    description: "Reads the world-space normal from Bevy's normal prepass at this fragment. Requires NormalPrepass on the camera; returns +Y when prepass is unavailable. Useful for wetness masks, surface-aware post-effects, and edge detection.",
    pins: || vec![
        PinTemplate::output("normal", "Normal", PinType::Vec3),
    ],
    color: CLR_SCENE,
};

pub static MOTION_VECTOR: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/motion_vector",
    display_name: "Motion Vector",
    category: CAT_SCENE,
    description: "Reads the per-fragment screen-space motion vector (Δ NDC since last frame) from Bevy's motion vector prepass. Requires MotionVectorPrepass on the camera. Magnitude drives motion-sensitive effects — motion blur masks, speed lines, velocity-warped distortion.",
    pins: || vec![
        PinTemplate::output("velocity", "Velocity", PinType::Vec2),
        PinTemplate::output("speed", "Speed", PinType::Float),
    ],
    color: CLR_SCENE,
};

pub static REFRACTION_UV_OFFSET: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/refraction_uv_offset",
    display_name: "Refraction UV Offset",
    category: CAT_SCENE,
    description: "Compute a screen-UV offset for refraction based on a distorting normal and strength. Feed into a Scene Color node (Phase D.2) or sample a custom render target.",
    pins: || vec![
        PinTemplate::input("normal", "Normal", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 1.0])),
        PinTemplate::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(0.05)),
        PinTemplate::output("offset", "UV Offset", PinType::Vec2),
    ],
    color: CLR_SCENE,
};

pub static SCREEN_UV: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/screen_uv",
    display_name: "Screen UV",
    category: CAT_SCENE,
    description: "Fragment's screen-space UV (0,0 top-left → 1,1 bottom-right). For screen-space effects.",
    pins: || vec![
        PinTemplate::output("uv", "UV", PinType::Vec2),
    ],
    color: CLR_SCENE,
};

pub static SCENE_COLOR_STUB: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/scene_color",
    display_name: "Scene Color (stub)",
    category: CAT_SCENE,
    description: "NOT IMPLEMENTED — Bevy doesn't expose a grab-pass texture to custom Material trait shaders without a custom render graph node. Returns magenta as a placeholder. Needs Phase D.2 render-graph work to enable.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
    ],
    color: CLR_SCENE,
};

pub static ENV_MAP_SAMPLE: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/env_map_sample",
    display_name: "Environment Map Sample",
    category: CAT_SCENE,
    description: "Sample the scene's environment cubemap at a given world-space direction and LOD. Works with both manually-loaded skyboxes (`Skybox` component) and Bevy's procedural atmosphere (dynamic sky baked to env map each frame). LOD 0 = sharpest mip, higher = blurrier (matches roughness-based reflections).",
    pins: || vec![
        PinTemplate::input("direction", "Direction", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
        PinTemplate::input("mip_level", "Mip Level", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
    ],
    color: CLR_SCENE,
};

pub static ENV_MAP_REFLECT: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/env_map_reflect",
    display_name: "Environment Map Reflect",
    category: CAT_SCENE,
    description: "Compute reflection from view direction off world_normal and sample the environment cubemap — classic mirror/glossy reflection. LOD controls glossiness (0 = perfect mirror, higher = matte).",
    pins: || vec![
        PinTemplate::input("normal", "Normal", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
        PinTemplate::input("mip_level", "Mip Level", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
    ],
    color: CLR_SCENE,
};

// =============================================================================
// OUTPUT NODES
// =============================================================================

pub static OUTPUT_SURFACE: MaterialNodeDef = MaterialNodeDef {
    node_type: "output/surface",
    display_name: "Surface Output",
    category: CAT_OUTPUT,
    description: "Full PBR surface material output — maps 1:1 onto StandardMaterial. Connect specular_transmission + ior for glass/water; clearcoat + clearcoat_roughness for car paint; anisotropy_strength + anisotropy_rotation for brushed metal / hair; diffuse_transmission + thickness for foliage / skin. Disconnected pins leave StandardMaterial defaults intact.",
    pins: || vec![
        // Core PBR
        PinTemplate::input("base_color", "Base Color", PinType::Color).with_default(PinValue::Color([0.8, 0.8, 0.8, 1.0])),
        PinTemplate::input("metallic", "Metallic", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("roughness", "Roughness", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("normal", "Normal", PinType::Vec3),
        PinTemplate::input("emissive", "Emissive", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::input("ao", "AO", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("alpha", "Alpha", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("reflectance", "Reflectance", PinType::Vec3).with_default(PinValue::Vec3([0.5, 0.5, 0.5])),

        // Transmission (refraction — connect for glass, water, ice)
        PinTemplate::input("specular_transmission", "Specular Transmission", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("diffuse_transmission", "Diffuse Transmission", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("thickness", "Thickness", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("ior", "Index of Refraction", PinType::Float).with_default(PinValue::Float(1.5)),
        PinTemplate::input("attenuation_distance", "Attenuation Distance", PinType::Float).with_default(PinValue::Float(1.0e37)),

        // Clearcoat (second specular layer — car paint, lacquer)
        PinTemplate::input("clearcoat", "Clearcoat", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("clearcoat_roughness", "Clearcoat Roughness", PinType::Float).with_default(PinValue::Float(0.5)),

        // Anisotropy (directional specular — brushed metal, hair)
        PinTemplate::input("anisotropy_strength", "Anisotropy Strength", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("anisotropy_rotation", "Anisotropy Rotation", PinType::Float).with_default(PinValue::Float(0.0)),
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
    &UV, &UV_SCALE, &UV_POLAR, &UV_ROTATOR, &UV_PANNER,
    &WORLD_POSITION, &WORLD_NORMAL, &VIEW_DIRECTION, &TIME, &VERTEX_COLOR,
    &CAMERA_POSITION, &OBJECT_POSITION,
    // Texture
    &SAMPLE_TEXTURE, &SAMPLE_NORMAL, &TRIPLANAR_SAMPLE,
    &SAMPLE_TEXTURE_LOD, &SAMPLE_TEXTURE_GRAD,
    &SAMPLE_CUBEMAP, &SAMPLE_2D_ARRAY, &SAMPLE_3D,
    // Math
    &ADD, &SUBTRACT, &MULTIPLY, &DIVIDE, &POWER, &ABS, &NEGATE, &ONE_MINUS,
    &FRACT, &FLOOR, &CEIL, &MIN, &MAX, &CLAMP, &LERP, &SMOOTHSTEP, &STEP,
    &REMAP, &SIN, &COS, &SATURATE,
    &MODULO, &SIGN, &ATAN2, &TRUNC, &ROUND, &EXP, &LOG, &SQRT, &RECIPROCAL,
    &TAN, &ASIN, &ACOS, &RADIANS, &DEGREES,
    // Vector
    &SPLIT_VEC2, &SPLIT_VEC3, &COMBINE_VEC2, &COMBINE_VEC3, &COMBINE_VEC4,
    &DOT, &CROSS, &NORMALIZE, &DISTANCE, &LENGTH, &REFLECT, &REFRACT, &SWIZZLE,
    // Color
    &COLOR_CONSTANT, &FLOAT_CONSTANT, &VEC2_CONSTANT, &VEC3_CONSTANT,
    &COLOR_LERP, &COSINE_PALETTE, &FRESNEL,
    &SRGB_TO_LINEAR, &LINEAR_TO_SRGB, &RGB_TO_HSV, &HSV_TO_RGB, &HUE_SHIFT,
    &LUMINANCE, &GAMMA, &BRIGHTNESS_CONTRAST, &SATURATION, &BLEND,
    // Procedural
    &NOISE_PERLIN, &NOISE_SIMPLEX, &NOISE_VORONOI, &NOISE_FBM,
    &CHECKERBOARD, &GRADIENT, &BRICK, &NORMAL_FROM_HEIGHT, &WORLD_NORMAL_FROM_HEIGHT,
    &DOMAIN_WARP, &NOISE_RIDGED, &NOISE_TURBULENCE, &NOISE_BILLOW,
    &NOISE_WHITE, &NOISE_CURL,
    &GRADIENT_RADIAL, &GRADIENT_LINEAR, &GRADIENT_ANGULAR, &GRADIENT_DIAMOND,
    &BUMP_OFFSET,
    &NOISE_TRIPLANAR_FBM, &NOISE_TRIPLANAR_RIDGED, &NOISE_TRIPLANAR_TURBULENCE,
    &NOISE_TRIPLANAR_BILLOW, &NOISE_TRIPLANAR_VORONOI,
    &HEX_TILE,
    // Animation
    &UV_SCROLL, &FLOW_MAP, &SINE_WAVE, &PING_PONG, &WIND, &FLIPBOOK_UV,
    // Utility
    &WORLD_POSITION_MASK, &SLOPE_MASK, &DEPTH_FADE,
    &DPDX, &DPDY, &FWIDTH, &DITHER, &HASH,
    // Control
    &IF_NODE, &STATIC_SWITCH, &COMPONENT_MASK,
    &GREATER_THAN, &LESS_THAN, &EQUAL, &NOT_EQUAL,
    &AND_NODE, &OR_NODE, &NOT_NODE,
    // Functions
    &FUNCTION_INPUT_POINT, &FUNCTION_OUTPUT_POINT, &FUNCTION_CALL,
    // Scene
    &PIXEL_DEPTH, &SCENE_DEPTH, &SCENE_DEPTH_FADE,
    &SCENE_NORMAL, &MOTION_VECTOR,
    &REFRACTION_UV_OFFSET, &SCREEN_UV, &SCENE_COLOR_STUB,
    &ENV_MAP_SAMPLE, &ENV_MAP_REFLECT,
    // Output
    &OUTPUT_SURFACE, &OUTPUT_TERRAIN_LAYER, &OUTPUT_VEGETATION, &OUTPUT_UNLIT,
];

/// Get all unique categories in display order.
pub fn categories() -> Vec<&'static str> {
    vec![
        CAT_INPUT, CAT_TEXTURE, CAT_MATH, CAT_VECTOR, CAT_COLOR,
        CAT_PROCEDURAL, CAT_ANIMATION, CAT_UTILITY, CAT_CONTROL, CAT_SCENE, CAT_OUTPUT,
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
