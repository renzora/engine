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
