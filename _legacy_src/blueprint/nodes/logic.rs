//! Logic and control flow nodes

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

/// If/Branch - execute different paths based on condition
pub static IF_BRANCH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "logic/if",
    display_name: "Branch",
    category: "Logic",
    description: "Execute True or False path based on condition",
    create_pins: || vec![
        Pin::input("exec", "", PinType::Flow),
        Pin::input("condition", "Condition", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("true", "True", PinType::Flow),
        Pin::output("false", "False", PinType::Flow),
    ],
    color: [100, 150, 200], // Blue for logic
    is_event: false,
    is_comment: false,
};

/// Compare two values
pub static COMPARE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "logic/compare",
    display_name: "Compare",
    category: "Logic",
    description: "Compare two values (==, !=, >, <, >=, <=)",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        // Comparison mode stored in input_values as "mode" (string: "==", "!=", ">", "<", ">=", "<=")
        Pin::output("result", "Result", PinType::Bool),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Logical AND
pub static AND: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "logic/and",
    display_name: "And",
    category: "Logic",
    description: "Logical AND (A && B)",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::input("b", "B", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("result", "Result", PinType::Bool),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Logical OR
pub static OR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "logic/or",
    display_name: "Or",
    category: "Logic",
    description: "Logical OR (A || B)",
    create_pins: || vec![
        Pin::input("a", "A", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::input("b", "B", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("result", "Result", PinType::Bool),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Logical NOT
pub static NOT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "logic/not",
    display_name: "Not",
    category: "Logic",
    description: "Logical NOT (!A)",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("result", "Result", PinType::Bool),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};
