//! Math operation nodes

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

/// Add two values
pub static ADD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/add",
    display_name: "Add",
    category: "Math",
    description: "Add two values together (A + B)",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100], // Green for math
    is_event: false,
    is_comment: false,
};

/// Subtract two values
pub static SUBTRACT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/subtract",
    display_name: "Subtract",
    category: "Math",
    description: "Subtract B from A (A - B)",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Multiply two values
pub static MULTIPLY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/multiply",
    display_name: "Multiply",
    category: "Math",
    description: "Multiply two values (A * B)",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Divide two values
pub static DIVIDE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/divide",
    display_name: "Divide",
    category: "Math",
    description: "Divide A by B (A / B)",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Linearly interpolate between two values
pub static LERP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/lerp",
    display_name: "Lerp",
    category: "Math",
    description: "Linear interpolation between A and B by T",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Clamp a value between min and max
pub static CLAMP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/clamp",
    display_name: "Clamp",
    category: "Math",
    description: "Clamp value between min and max",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("min", "Min", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("max", "Max", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Absolute value
pub static ABS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/abs",
    display_name: "Abs",
    category: "Math",
    description: "Absolute value of the input",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Minimum of two values
pub static MIN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/min",
    display_name: "Min",
    category: "Math",
    description: "Returns the smaller of two values",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Maximum of two values
pub static MAX: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/max",
    display_name: "Max",
    category: "Math",
    description: "Returns the larger of two values",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Sine function
pub static SIN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/sin",
    display_name: "Sin",
    category: "Math",
    description: "Sine of the input (radians)",
    create_pins: || vec![
        Pin::input("value", "Radians", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Cosine function
pub static COS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/cos",
    display_name: "Cos",
    category: "Math",
    description: "Cosine of the input (radians)",
    create_pins: || vec![
        Pin::input("value", "Radians", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Tangent function
pub static TAN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/tan",
    display_name: "Tan",
    category: "Math",
    description: "Tangent of the input (radians)",
    create_pins: || vec![
        Pin::input("value", "Radians", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Arc sine function
pub static ASIN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/asin",
    display_name: "Asin",
    category: "Math",
    description: "Arc sine of the input (returns radians)",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Arc cosine function
pub static ACOS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/acos",
    display_name: "Acos",
    category: "Math",
    description: "Arc cosine of the input (returns radians)",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Arc tangent function
pub static ATAN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/atan",
    display_name: "Atan",
    category: "Math",
    description: "Arc tangent of the input (returns radians)",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Arc tangent of y/x with correct quadrant
pub static ATAN2: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/atan2",
    display_name: "Atan2",
    category: "Math",
    description: "Arc tangent of y/x with correct quadrant (returns radians)",
    create_pins: || vec![
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Floor - round down to nearest integer
pub static FLOOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/floor",
    display_name: "Floor",
    category: "Math",
    description: "Round down to nearest integer",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Ceil - round up to nearest integer
pub static CEIL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/ceil",
    display_name: "Ceil",
    category: "Math",
    description: "Round up to nearest integer",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Round to nearest integer
pub static ROUND: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/round",
    display_name: "Round",
    category: "Math",
    description: "Round to nearest integer",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Square root
pub static SQRT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/sqrt",
    display_name: "Sqrt",
    category: "Math",
    description: "Square root of the input",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Power function
pub static POW: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/pow",
    display_name: "Power",
    category: "Math",
    description: "Raise base to exponent power",
    create_pins: || vec![
        Pin::input("base", "Base", PinType::Float).with_default(PinValue::Float(2.0)),
        Pin::input("exponent", "Exponent", PinType::Float).with_default(PinValue::Float(2.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Natural logarithm
pub static LOG: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/log",
    display_name: "Log",
    category: "Math",
    description: "Natural logarithm (ln) of the input",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Exponential function (e^x)
pub static EXP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/exp",
    display_name: "Exp",
    category: "Math",
    description: "Exponential function (e^x)",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Sign of value (-1, 0, or 1)
pub static SIGN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/sign",
    display_name: "Sign",
    category: "Math",
    description: "Returns -1, 0, or 1 based on sign of input",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Modulo (remainder after division)
pub static MOD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/mod",
    display_name: "Modulo",
    category: "Math",
    description: "Remainder after division (A % B)",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Fractional part
pub static FRACT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/fract",
    display_name: "Fract",
    category: "Math",
    description: "Fractional part of the input (value - floor(value))",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Negate value
pub static NEGATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/negate",
    display_name: "Negate",
    category: "Math",
    description: "Negate the input value (-x)",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// One minus (1 - x)
pub static ONE_MINUS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/one_minus",
    display_name: "One Minus",
    category: "Math",
    description: "Returns 1 - x",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Reciprocal (1/x)
pub static RECIPROCAL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/reciprocal",
    display_name: "Reciprocal",
    category: "Math",
    description: "Returns 1/x",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Smoothstep interpolation
pub static SMOOTHSTEP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/smoothstep",
    display_name: "Smoothstep",
    category: "Math",
    description: "Smooth Hermite interpolation between 0 and 1",
    create_pins: || vec![
        Pin::input("edge0", "Edge 0", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("edge1", "Edge 1", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Step function (0 if x < edge, else 1)
pub static STEP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/step",
    display_name: "Step",
    category: "Math",
    description: "Returns 0 if x < edge, else 1",
    create_pins: || vec![
        Pin::input("edge", "Edge", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Random float 0-1
pub static RANDOM: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/random",
    display_name: "Random",
    category: "Math",
    description: "Generate a random float between 0 and 1",
    create_pins: || vec![
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Random float in range
pub static RANDOM_RANGE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/random_range",
    display_name: "Random Range",
    category: "Math",
    description: "Generate a random float between min and max",
    create_pins: || vec![
        Pin::input("min", "Min", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("max", "Max", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Random integer in range
pub static RANDOM_INT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/random_int",
    display_name: "Random Int",
    category: "Math",
    description: "Generate a random integer between min and max (inclusive)",
    create_pins: || vec![
        Pin::input("min", "Min", PinType::Int).with_default(PinValue::Int(0)),
        Pin::input("max", "Max", PinType::Int).with_default(PinValue::Int(100)),
        Pin::output("result", "Result", PinType::Int),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Map value from one range to another
pub static MAP_RANGE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/map_range",
    display_name: "Map Range",
    category: "Math",
    description: "Map a value from one range to another",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("in_min", "In Min", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("in_max", "In Max", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("out_min", "Out Min", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("out_max", "Out Max", PinType::Float).with_default(PinValue::Float(100.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Degrees to radians
pub static DEG_TO_RAD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/deg_to_rad",
    display_name: "Deg to Rad",
    category: "Math",
    description: "Convert degrees to radians",
    create_pins: || vec![
        Pin::input("degrees", "Degrees", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("radians", "Radians", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Radians to degrees
pub static RAD_TO_DEG: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/rad_to_deg",
    display_name: "Rad to Deg",
    category: "Math",
    description: "Convert radians to degrees",
    create_pins: || vec![
        Pin::input("radians", "Radians", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("degrees", "Degrees", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// VECTOR MATH
// =============================================================================

/// Dot product of two vectors
pub static DOT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/dot",
    display_name: "Dot Product",
    category: "Math Vector",
    description: "Dot product of two Vec3 values",
    create_pins: || vec![
        Pin::input("a_x", "A.X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("a_y", "A.Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("a_z", "A.Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b_x", "B.X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b_y", "B.Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b_z", "B.Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Cross product of two vectors
pub static CROSS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/cross",
    display_name: "Cross Product",
    category: "Math Vector",
    description: "Cross product of two Vec3 values",
    create_pins: || vec![
        Pin::input("a_x", "A.X", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("a_y", "A.Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("a_z", "A.Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b_x", "B.X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b_y", "B.Y", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("b_z", "B.Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("z", "Z", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Normalize a vector
pub static NORMALIZE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/normalize",
    display_name: "Normalize",
    category: "Math Vector",
    description: "Normalize a vector to unit length",
    create_pins: || vec![
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("z", "Z", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Length of a vector
pub static LENGTH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/length",
    display_name: "Length",
    category: "Math Vector",
    description: "Get the length (magnitude) of a vector",
    create_pins: || vec![
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("length", "Length", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Distance between two points
pub static DISTANCE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/distance",
    display_name: "Distance",
    category: "Math Vector",
    description: "Get the distance between two points",
    create_pins: || vec![
        Pin::input("a_x", "A.X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("a_y", "A.Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("a_z", "A.Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b_x", "B.X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b_y", "B.Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b_z", "B.Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("distance", "Distance", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Direction from A to B (normalized)
pub static DIRECTION_TO: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/direction_to",
    display_name: "Direction To",
    category: "Math Vector",
    description: "Get the normalized direction from point A to point B",
    create_pins: || vec![
        Pin::input("a_x", "A.X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("a_y", "A.Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("a_z", "A.Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b_x", "B.X", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("b_y", "B.Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b_z", "B.Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("z", "Z", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Angle between two vectors
pub static ANGLE_BETWEEN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/angle_between",
    display_name: "Angle Between",
    category: "Math Vector",
    description: "Get the angle between two vectors in radians",
    create_pins: || vec![
        Pin::input("a_x", "A.X", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("a_y", "A.Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("a_z", "A.Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b_x", "B.X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b_y", "B.Y", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("b_z", "B.Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("radians", "Radians", PinType::Float),
        Pin::output("degrees", "Degrees", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Reflect vector around normal
pub static REFLECT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/reflect",
    display_name: "Reflect",
    category: "Math Vector",
    description: "Reflect a vector around a normal",
    create_pins: || vec![
        Pin::input("dir_x", "Dir.X", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("dir_y", "Dir.Y", PinType::Float).with_default(PinValue::Float(-1.0)),
        Pin::input("dir_z", "Dir.Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("normal_x", "Normal.X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("normal_y", "Normal.Y", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("normal_z", "Normal.Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("z", "Z", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Lerp vector
pub static LERP_VEC3: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/lerp_vec3",
    display_name: "Lerp Vec3",
    category: "Math Vector",
    description: "Linear interpolation between two Vec3 values",
    create_pins: || vec![
        Pin::input("a_x", "A.X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("a_y", "A.Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("a_z", "A.Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b_x", "B.X", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("b_y", "B.Y", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("b_z", "B.Z", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("z", "Z", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Make Vec3 from components
pub static MAKE_VEC3: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/make_vec3",
    display_name: "Make Vec3",
    category: "Math Vector",
    description: "Create a Vec3 from X, Y, Z components",
    create_pins: || vec![
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("vec3", "Vec3", PinType::Vec3),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Break Vec3 into components
pub static BREAK_VEC3: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/break_vec3",
    display_name: "Break Vec3",
    category: "Math Vector",
    description: "Split a Vec3 into X, Y, Z components",
    create_pins: || vec![
        Pin::input("vec3", "Vec3", PinType::Vec3),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("z", "Z", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Random Vec3
pub static RANDOM_VEC3: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/random_vec3",
    display_name: "Random Vec3",
    category: "Math Vector",
    description: "Generate a random Vec3 with each component between min and max",
    create_pins: || vec![
        Pin::input("min", "Min", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("max", "Max", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("z", "Z", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Random unit vector (on unit sphere)
pub static RANDOM_DIRECTION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/random_direction",
    display_name: "Random Direction",
    category: "Math Vector",
    description: "Generate a random unit vector (point on unit sphere)",
    create_pins: || vec![
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("z", "Z", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// EXTENDED MATH NODES
// =============================================================================

/// Smootherstep (Ken Perlin's improved version)
pub static SMOOTHERSTEP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/smootherstep",
    display_name: "Smootherstep",
    category: "Math",
    description: "Smootherstep interpolation (6t^5 - 15t^4 + 10t^3)",
    create_pins: || vec![
        Pin::input("edge0", "Edge 0", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("edge1", "Edge 1", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Move Towards - move a value towards a target at max speed
pub static MOVE_TOWARDS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/move_towards",
    display_name: "Move Towards",
    category: "Math",
    description: "Move a value towards target by at most max_delta per step",
    create_pins: || vec![
        Pin::input("current", "Current", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("target", "Target", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("max_delta", "Max Delta", PinType::Float).with_default(PinValue::Float(0.1)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Normalize Angle - normalize angle to 0..360 range
pub static NORMALIZE_ANGLE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/normalize_angle",
    display_name: "Normalize Angle",
    category: "Math",
    description: "Normalize an angle to the 0..360 degree range",
    create_pins: || vec![
        Pin::input("angle", "Angle", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Angle Difference - shortest difference between two angles
pub static ANGLE_DIFFERENCE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/angle_difference",
    display_name: "Angle Difference",
    category: "Math",
    description: "Shortest signed difference between two angles in degrees",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Lerp Angle - linearly interpolate between two angles
pub static LERP_ANGLE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/lerp_angle",
    display_name: "Lerp Angle",
    category: "Math",
    description: "Linearly interpolate between two angles (handles wrapping)",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("t", "T", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Inverse Lerp - determine where a value falls between two endpoints
pub static INVERSE_LERP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/inverse_lerp",
    display_name: "Inverse Lerp",
    category: "Math",
    description: "Returns the t value that produces the given value between a and b",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Pi constant
pub static PI: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/pi",
    display_name: "Pi",
    category: "Math",
    description: "The constant Pi (3.14159...)",
    create_pins: || vec![
        Pin::output("value", "Value", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Tau constant (2*Pi)
pub static TAU: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/tau",
    display_name: "Tau",
    category: "Math",
    description: "The constant Tau (2*Pi = 6.28318...)",
    create_pins: || vec![
        Pin::output("value", "Value", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// E constant
pub static E: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/e",
    display_name: "E",
    category: "Math",
    description: "Euler's number (2.71828...)",
    create_pins: || vec![
        Pin::output("value", "Value", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Truncate - remove fractional part
pub static TRUNC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/trunc",
    display_name: "Truncate",
    category: "Math",
    description: "Remove the fractional part of a float (round towards zero)",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Log base 10
pub static LOG10: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/log10",
    display_name: "Log10",
    category: "Math",
    description: "Base-10 logarithm",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Log base 2
pub static LOG2: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/log2",
    display_name: "Log2",
    category: "Math",
    description: "Base-2 logarithm",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// 2D Distance
pub static DISTANCE_2D: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/distance_2d",
    display_name: "Distance 2D",
    category: "Math Vector",
    description: "Distance between two 2D points",
    create_pins: || vec![
        Pin::input("x1", "X1", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("y1", "Y1", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("x2", "X2", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("y2", "Y2", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};

/// 2D Vector Length
pub static LENGTH_2D: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "math/length_2d",
    display_name: "Length 2D",
    category: "Math Vector",
    description: "Length of a 2D vector",
    create_pins: || vec![
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("result", "Result", PinType::Float),
    ],
    color: [100, 200, 100],
    is_event: false,
    is_comment: false,
};
