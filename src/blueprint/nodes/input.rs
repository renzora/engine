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

/// Is Key Just Pressed - check if a key was pressed this frame
pub static IS_KEY_JUST_PRESSED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "input/is_key_just_pressed",
    display_name: "Is Key Just Pressed",
    category: "Input",
    description: "Check if a key was pressed this frame (single press detection)",
    create_pins: || vec![
        // Key name stored in input_values as "key" (string: "Space", "Shift", etc.)
        Pin::output("pressed", "Pressed", PinType::Bool),
    ],
    color: [200, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Is Key Just Released - check if a key was released this frame
pub static IS_KEY_JUST_RELEASED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "input/is_key_just_released",
    display_name: "Is Key Just Released",
    category: "Input",
    description: "Check if a key was released this frame",
    create_pins: || vec![
        // Key name stored in input_values as "key" (string: "Space", "Shift", etc.)
        Pin::output("released", "Released", PinType::Bool),
    ],
    color: [200, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Is Mouse Button Pressed - check if a mouse button is pressed
pub static IS_MOUSE_BUTTON_PRESSED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "input/is_mouse_button_pressed",
    display_name: "Is Mouse Button Pressed",
    category: "Input",
    description: "Check if a mouse button is currently pressed",
    create_pins: || vec![
        // Button stored in input_values as "button" (string: "Left", "Right", "Middle")
        Pin::output("pressed", "Pressed", PinType::Bool),
    ],
    color: [200, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Get Mouse Scroll - get the mouse scroll wheel delta
pub static GET_MOUSE_SCROLL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "input/get_mouse_scroll",
    display_name: "Get Mouse Scroll",
    category: "Input",
    description: "Get the mouse scroll wheel delta this frame",
    create_pins: || vec![
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
    ],
    color: [200, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Get Gamepad Left Stick - get left stick axis values
pub static GET_GAMEPAD_LEFT_STICK: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "input/get_gamepad_left_stick",
    display_name: "Get Gamepad Left Stick",
    category: "Input",
    description: "Get the left analog stick axis values (-1 to 1)",
    create_pins: || vec![
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
    ],
    color: [200, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Get Gamepad Right Stick - get right stick axis values
pub static GET_GAMEPAD_RIGHT_STICK: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "input/get_gamepad_right_stick",
    display_name: "Get Gamepad Right Stick",
    category: "Input",
    description: "Get the right analog stick axis values (-1 to 1)",
    create_pins: || vec![
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
    ],
    color: [200, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Is Gamepad Button Pressed - check if a gamepad button is pressed
pub static IS_GAMEPAD_BUTTON_PRESSED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "input/is_gamepad_button_pressed",
    display_name: "Is Gamepad Button Pressed",
    category: "Input",
    description: "Check if a gamepad button is currently pressed",
    create_pins: || vec![
        // Button stored in input_values as "button" (string: "South", "East", "West", "North", etc.)
        Pin::output("pressed", "Pressed", PinType::Bool),
    ],
    color: [200, 200, 100],
    is_event: false,
    is_comment: false,
};
