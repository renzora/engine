//! Material node type definitions and registry.
//!
//! Each node type declares its pins, category, and display info.
//! The WGSL codegen uses node_type strings to dispatch code generation.
//!
//! One module per category, mirroring `codegen::emit` — the definition of a node
//! and the code that emits it sit in files of the same name. Every definition is
//! re-exported flat from here, so `nodes::ADD` keeps working regardless of which
//! module it lives in.

use super::graph::PinTemplate;

pub mod animation;
pub mod color;
pub mod control;
pub mod custom;
pub mod functions;
pub mod inputs;
pub mod math;
pub mod output;
pub mod parameters;
pub mod procedural;
pub mod scene;
pub mod textures;
pub mod utility;
pub mod vector;

pub use animation::*;
pub use color::*;
pub use control::*;
pub use custom::*;
pub use functions::*;
pub use inputs::*;
pub use math::*;
pub use output::*;
pub use parameters::*;
pub use procedural::*;
pub use scene::*;
pub use textures::*;
pub use utility::*;
pub use vector::*;

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
pub const CAT_PARAMETER: &str = "Parameter";
pub const CAT_TEXTURE: &str = "Texture";
pub const CAT_MATH: &str = "Math";
pub const CAT_VECTOR: &str = "Vector";
pub const CAT_COLOR: &str = "Color";
pub const CAT_PROCEDURAL: &str = "Procedural";
pub const CAT_ANIMATION: &str = "Animation";
pub const CAT_UTILITY: &str = "Utility";
pub const CAT_CONTROL: &str = "Control";
pub const CAT_SCENE: &str = "Scene";
pub const CAT_CUSTOM: &str = "Custom";
pub const CAT_OUTPUT: &str = "Output";

// ── Color constants for categories ──────────────────────────────────────────

const CLR_INPUT: [u8; 3] = [100, 150, 220];
const CLR_PARAMETER: [u8; 3] = [180, 110, 200];
const CLR_TEXTURE: [u8; 3] = [200, 150, 120];
const CLR_MATH: [u8; 3] = [120, 120, 120];
const CLR_VECTOR: [u8; 3] = [127, 204, 25];
const CLR_COLOR: [u8; 3] = [220, 170, 80];
const CLR_PROCEDURAL: [u8; 3] = [160, 100, 200];
const CLR_ANIMATION: [u8; 3] = [80, 200, 180];
const CLR_UTILITY: [u8; 3] = [140, 140, 160];
const CLR_CONTROL: [u8; 3] = [200, 200, 80];
const CLR_SCENE: [u8; 3] = [100, 180, 220];
const CLR_CUSTOM: [u8; 3] = [210, 90, 90];
const CLR_OUTPUT: [u8; 3] = [200, 60, 60];

// =============================================================================
// REGISTRY
// =============================================================================

/// All available material node types.
pub static ALL_NODES: &[&MaterialNodeDef] = &[
    // Input
    &UV,
    &UV_SCALE,
    &UV_POLAR,
    &UV_ROTATOR,
    &UV_PANNER,
    &WORLD_POSITION,
    &WORLD_NORMAL,
    &VIEW_DIRECTION,
    &TIME,
    &VERTEX_COLOR,
    &CAMERA_POSITION,
    &OBJECT_POSITION,
    // Parameter
    &PARAM_FLOAT,
    &PARAM_COLOR,
    &PARAM_VEC2,
    &PARAM_VEC3,
    &PARAM_VEC4,
    &PARAM_BOOL,
    // Texture
    &SAMPLE_TEXTURE,
    &SAMPLE_NORMAL,
    &TRIPLANAR_SAMPLE,
    &SAMPLE_TEXTURE_LOD,
    &SAMPLE_TEXTURE_GRAD,
    &SAMPLE_CUBEMAP,
    &SAMPLE_2D_ARRAY,
    &SAMPLE_3D,
    // Math
    &ADD,
    &SUBTRACT,
    &MULTIPLY,
    &DIVIDE,
    &POWER,
    &ABS,
    &NEGATE,
    &ONE_MINUS,
    &FRACT,
    &FLOOR,
    &CEIL,
    &MIN,
    &MAX,
    &CLAMP,
    &LERP,
    &SMOOTHSTEP,
    &STEP,
    &REMAP,
    &SIN,
    &COS,
    &SATURATE,
    &MODULO,
    &SIGN,
    &ATAN2,
    &TRUNC,
    &ROUND,
    &EXP,
    &LOG,
    &SQRT,
    &RECIPROCAL,
    &TAN,
    &ASIN,
    &ACOS,
    &RADIANS,
    &DEGREES,
    // Vector
    &SPLIT_VEC2,
    &SPLIT_VEC3,
    &COMBINE_VEC2,
    &COMBINE_VEC3,
    &COMBINE_VEC4,
    &DOT,
    &CROSS,
    &NORMALIZE,
    &DISTANCE,
    &LENGTH,
    &REFLECT,
    &REFRACT,
    &SWIZZLE,
    // Color
    &COLOR_CONSTANT,
    &FLOAT_CONSTANT,
    &VEC2_CONSTANT,
    &VEC3_CONSTANT,
    &COLOR_LERP,
    &COSINE_PALETTE,
    &FRESNEL,
    &SRGB_TO_LINEAR,
    &LINEAR_TO_SRGB,
    &RGB_TO_HSV,
    &HSV_TO_RGB,
    &HUE_SHIFT,
    &LUMINANCE,
    &GAMMA,
    &BRIGHTNESS_CONTRAST,
    &SATURATION,
    &BLEND,
    // Procedural
    &NOISE_PERLIN,
    &NOISE_SIMPLEX,
    &NOISE_VORONOI,
    &NOISE_FBM,
    &CHECKERBOARD,
    &GRADIENT,
    &BRICK,
    &NORMAL_FROM_HEIGHT,
    &WORLD_NORMAL_FROM_HEIGHT,
    &DOMAIN_WARP,
    &NOISE_RIDGED,
    &NOISE_TURBULENCE,
    &NOISE_BILLOW,
    &NOISE_WHITE,
    &NOISE_CURL,
    &GRADIENT_RADIAL,
    &GRADIENT_LINEAR,
    &GRADIENT_ANGULAR,
    &GRADIENT_DIAMOND,
    &BUMP_OFFSET,
    &NOISE_TRIPLANAR_FBM,
    &NOISE_TRIPLANAR_RIDGED,
    &NOISE_TRIPLANAR_TURBULENCE,
    &NOISE_TRIPLANAR_BILLOW,
    &NOISE_TRIPLANAR_VORONOI,
    &HEX_TILE,
    // Animation
    &UV_SCROLL,
    &FLOW_MAP,
    &SINE_WAVE,
    &PING_PONG,
    &WIND,
    &FLIPBOOK_UV,
    // Utility
    &WORLD_POSITION_MASK,
    &SLOPE_MASK,
    &DEPTH_FADE,
    &DPDX,
    &DPDY,
    &FWIDTH,
    &DITHER,
    &HASH,
    // Control
    &IF_NODE,
    &STATIC_SWITCH,
    &COMPONENT_MASK,
    &GREATER_THAN,
    &LESS_THAN,
    &EQUAL,
    &NOT_EQUAL,
    &AND_NODE,
    &OR_NODE,
    &NOT_NODE,
    // Functions
    &FUNCTION_INPUT_POINT,
    &FUNCTION_OUTPUT_POINT,
    &FUNCTION_CALL,
    // Scene
    &PIXEL_DEPTH,
    &SCENE_DEPTH,
    &SCENE_DEPTH_FADE,
    &SCENE_NORMAL,
    &MOTION_VECTOR,
    &REFRACTION_UV_OFFSET,
    &SCREEN_UV,
    &SCENE_COLOR_STUB,
    &ENV_MAP_SAMPLE,
    &ENV_MAP_REFLECT,
    // Custom
    &CUSTOM_CODE,
    // Output
    &OUTPUT_SURFACE,
    &OUTPUT_TERRAIN_LAYER,
    &OUTPUT_VEGETATION,
    &OUTPUT_UNLIT,
];

/// Get all unique categories in display order.
pub fn categories() -> Vec<&'static str> {
    vec![
        CAT_INPUT,
        CAT_PARAMETER,
        CAT_TEXTURE,
        CAT_MATH,
        CAT_VECTOR,
        CAT_COLOR,
        CAT_PROCEDURAL,
        CAT_ANIMATION,
        CAT_UTILITY,
        CAT_CONTROL,
        CAT_SCENE,
        CAT_CUSTOM,
        CAT_OUTPUT,
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
