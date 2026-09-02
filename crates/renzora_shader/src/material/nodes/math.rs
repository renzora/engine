//! Math nodes — scalar arithmetic, rounding, interpolation and the trig /
//! exponential library.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_MATH, CLR_MATH};

pub static ADD: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/add",
    display_name: "Add",
    category: CAT_MATH,
    description: "A + B",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static SUBTRACT: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/subtract",
    display_name: "Subtract",
    category: CAT_MATH,
    description: "A - B",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static MULTIPLY: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/multiply",
    display_name: "Multiply",
    category: CAT_MATH,
    description: "A * B",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static DIVIDE: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/divide",
    display_name: "Divide",
    category: CAT_MATH,
    description: "A / B",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static POWER: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/power",
    display_name: "Power",
    category: CAT_MATH,
    description: "Base ^ Exponent",
    pins: || {
        vec![
            PinTemplate::input("base", "Base", PinType::Float).with_default(PinValue::Float(2.0)),
            PinTemplate::input("exp", "Exponent", PinType::Float)
                .with_default(PinValue::Float(2.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static ABS: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/abs",
    display_name: "Abs",
    category: CAT_MATH,
    description: "Absolute value",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static NEGATE: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/negate",
    display_name: "Negate",
    category: CAT_MATH,
    description: "-Value",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static ONE_MINUS: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/one_minus",
    display_name: "One Minus",
    category: CAT_MATH,
    description: "1.0 - Value",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static FRACT: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/fract",
    display_name: "Fract",
    category: CAT_MATH,
    description: "Fractional part",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static FLOOR: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/floor",
    display_name: "Floor",
    category: CAT_MATH,
    description: "Round down to integer",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static CEIL: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/ceil",
    display_name: "Ceil",
    category: CAT_MATH,
    description: "Round up to integer",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static MIN: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/min",
    display_name: "Min",
    category: CAT_MATH,
    description: "Minimum of A and B",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static MAX: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/max",
    display_name: "Max",
    category: CAT_MATH,
    description: "Maximum of A and B",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static CLAMP: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/clamp",
    display_name: "Clamp",
    category: CAT_MATH,
    description: "Clamp value between min and max",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
            PinTemplate::input("min", "Min", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("max", "Max", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static LERP: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/lerp",
    display_name: "Lerp",
    category: CAT_MATH,
    description: "Linear interpolation: mix(A, B, T)",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::input("t", "T", PinType::Float).with_default(PinValue::Float(0.5)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static SMOOTHSTEP: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/smoothstep",
    display_name: "Smoothstep",
    category: CAT_MATH,
    description: "Hermite interpolation between edge0 and edge1",
    pins: || {
        vec![
            PinTemplate::input("edge0", "Edge 0", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::input("edge1", "Edge 1", PinType::Float)
                .with_default(PinValue::Float(1.0)),
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static STEP: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/step",
    display_name: "Step",
    category: CAT_MATH,
    description: "0.0 if value < edge, 1.0 otherwise",
    pins: || {
        vec![
            PinTemplate::input("edge", "Edge", PinType::Float).with_default(PinValue::Float(0.5)),
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static REMAP: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/remap",
    display_name: "Remap",
    category: CAT_MATH,
    description: "Remap value from [in_min, in_max] to [out_min, out_max]",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
            PinTemplate::input("in_min", "In Min", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::input("in_max", "In Max", PinType::Float)
                .with_default(PinValue::Float(1.0)),
            PinTemplate::input("out_min", "Out Min", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::input("out_max", "Out Max", PinType::Float)
                .with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static SIN: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/sin",
    display_name: "Sin",
    category: CAT_MATH,
    description: "Sine function",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static COS: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/cos",
    display_name: "Cos",
    category: CAT_MATH,
    description: "Cosine function",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static SATURATE: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/saturate",
    display_name: "Saturate",
    category: CAT_MATH,
    description: "Clamp to 0.0 - 1.0",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static MODULO: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/modulo",
    display_name: "Modulo",
    category: CAT_MATH,
    description: "A mod B (floating-point remainder)",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static SIGN: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/sign",
    display_name: "Sign",
    category: CAT_MATH,
    description: "-1 / 0 / +1 based on sign of value",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static ATAN2: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/atan2",
    display_name: "Atan2",
    category: CAT_MATH,
    description: "Two-argument arctangent: atan2(y, x) in radians",
    pins: || {
        vec![
            PinTemplate::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("x", "X", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static TRUNC: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/trunc",
    display_name: "Trunc",
    category: CAT_MATH,
    description: "Truncate toward zero",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static ROUND: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/round",
    display_name: "Round",
    category: CAT_MATH,
    description: "Round to nearest integer",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static EXP: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/exp",
    display_name: "Exp",
    category: CAT_MATH,
    description: "Natural exponential: e^x",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static LOG: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/log",
    display_name: "Log",
    category: CAT_MATH,
    description: "Natural logarithm: ln(x)",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static SQRT: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/sqrt",
    display_name: "Sqrt",
    category: CAT_MATH,
    description: "Square root",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static RECIPROCAL: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/reciprocal",
    display_name: "Reciprocal",
    category: CAT_MATH,
    description: "1 / value",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static TAN: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/tan",
    display_name: "Tan",
    category: CAT_MATH,
    description: "Tangent",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static ASIN: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/asin",
    display_name: "Asin",
    category: CAT_MATH,
    description: "Arcsine in radians",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static ACOS: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/acos",
    display_name: "Acos",
    category: CAT_MATH,
    description: "Arccosine in radians",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static RADIANS: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/radians",
    display_name: "To Radians",
    category: CAT_MATH,
    description: "Convert degrees → radians",
    pins: || {
        vec![
            PinTemplate::input("value", "Degrees", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Radians", PinType::Float),
        ]
    },
    color: CLR_MATH,
};

pub static DEGREES: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/degrees",
    display_name: "To Degrees",
    category: CAT_MATH,
    description: "Convert radians → degrees",
    pins: || {
        vec![
            PinTemplate::input("value", "Radians", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Degrees", PinType::Float),
        ]
    },
    color: CLR_MATH,
};
