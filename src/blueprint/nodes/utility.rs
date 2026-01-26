//! Utility nodes (Print, Sequence, Time, Variables, Comment)

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

/// Print - output a message to the console
pub static PRINT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "utility/print",
    display_name: "Print",
    category: "Utility",
    description: "Print a message to the console",
    create_pins: || vec![
        Pin::input("exec", "", PinType::Flow),
        Pin::input("message", "Message", PinType::String).with_default(PinValue::String("Hello".to_string())),
        Pin::output("exec", "", PinType::Flow),
    ],
    color: [150, 150, 150], // Gray for utility
    is_event: false,
    is_comment: false,
};

/// Sequence - execute multiple outputs in order
pub static SEQUENCE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "utility/sequence",
    display_name: "Sequence",
    category: "Utility",
    description: "Execute multiple flow paths in order",
    create_pins: || vec![
        Pin::input("exec", "", PinType::Flow),
        Pin::output("then_0", "Then 0", PinType::Flow),
        Pin::output("then_1", "Then 1", PinType::Flow),
        Pin::output("then_2", "Then 2", PinType::Flow),
        Pin::output("then_3", "Then 3", PinType::Flow),
    ],
    color: [150, 150, 150],
    is_event: false,
    is_comment: false,
};

/// Comment - non-functional note for documentation
pub static COMMENT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "utility/comment",
    display_name: "Comment",
    category: "Utility",
    description: "A non-functional note for documentation",
    create_pins: || vec![],
    color: [100, 100, 100], // Dark gray for comments
    is_event: false,
    is_comment: true,
};

/// Get Delta Time - get the frame delta time
pub static GET_DELTA: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "utility/get_delta",
    display_name: "Get Delta Time",
    category: "Time",
    description: "Get the time since last frame in seconds",
    create_pins: || vec![
        Pin::output("delta", "Delta", PinType::Float),
    ],
    color: [150, 150, 200], // Light blue for time
    is_event: false,
    is_comment: false,
};

/// Get Elapsed Time - get the total elapsed time
pub static GET_ELAPSED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "utility/get_elapsed",
    display_name: "Get Elapsed Time",
    category: "Time",
    description: "Get the total elapsed time since start",
    create_pins: || vec![
        Pin::output("elapsed", "Elapsed", PinType::Float),
    ],
    color: [150, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Get Variable - read a graph variable
pub static GET_VARIABLE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "variable/get",
    display_name: "Get Variable",
    category: "Variables",
    description: "Read a graph variable value",
    create_pins: || vec![
        // Variable name stored in input_values as "var_name"
        Pin::output("value", "Value", PinType::Any),
    ],
    color: [100, 200, 200], // Cyan for variables
    is_event: false,
    is_comment: false,
};

/// Set Variable - write to a graph variable
pub static SET_VARIABLE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "variable/set",
    display_name: "Set Variable",
    category: "Variables",
    description: "Write a value to a graph variable",
    create_pins: || vec![
        Pin::input("exec", "", PinType::Flow),
        // Variable name stored in input_values as "var_name"
        Pin::input("value", "Value", PinType::Any),
        Pin::output("exec", "", PinType::Flow),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};
