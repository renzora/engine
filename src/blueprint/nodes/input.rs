//! Input handling nodes

use super::{NodeTypeDefinition, Pin, PinType};

/// Get Input Axis - get movement input (-1 to 1 for WASD/arrows)
pub static GET_INPUT_AXIS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "input/get_axis",
    display_name: "Get Input Axis",
    category: "Input",
    description: "Get movement input axis (-1 to 1)",
    create_pins: || vec![
        Pin::output("x", "X (A/D)", PinType::Float),
        Pin::output("y", "Y (W/S)", PinType::Float),
    ],
    color: [200, 200, 100], // Yellow for input
    is_event: false,
    is_comment: false,
};

/// Is Key Pressed - check if a specific key is pressed
pub static IS_KEY_PRESSED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "input/is_key_pressed",
    display_name: "Is Key Pressed",
    category: "Input",
    description: "Check if a specific key is currently pressed",
    create_pins: || vec![
        // Key name stored in input_values as "key" (string: "Space", "Shift", etc.)
        Pin::output("pressed", "Pressed", PinType::Bool),
    ],
    color: [200, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Get Mouse Position - get the mouse cursor position
pub static GET_MOUSE_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "input/get_mouse_position",
    display_name: "Get Mouse Position",
    category: "Input",
    description: "Get the mouse cursor position in screen coordinates",
    create_pins: || vec![
        Pin::output("position", "Position", PinType::Vec2),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
    ],
    color: [200, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Get Mouse Delta - get the mouse movement since last frame
pub static GET_MOUSE_DELTA: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "input/get_mouse_delta",
    display_name: "Get Mouse Delta",
    category: "Input",
    description: "Get the mouse movement since last frame",
    create_pins: || vec![
        Pin::output("delta", "Delta", PinType::Vec2),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
    ],
    color: [200, 200, 100],
    is_event: false,
    is_comment: false,
};
