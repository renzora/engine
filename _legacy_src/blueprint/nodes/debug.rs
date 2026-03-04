//! Debug nodes
//!
//! Nodes for debugging, visualization, and development tools.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// DEBUG DRAWING
// =============================================================================

/// Draw debug line
pub static DEBUG_LINE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/line",
    display_name: "Debug Line",
    category: "Debug",
    description: "Draw a debug line in the world",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("start", "Start", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("end", "End", PinType::Vec3).with_default(PinValue::Vec3([1.0, 1.0, 1.0])),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 0.0, 0.0, 1.0])),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Draw debug ray
pub static DEBUG_RAY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/ray",
    display_name: "Debug Ray",
    category: "Debug",
    description: "Draw a debug ray from origin in direction",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("origin", "Origin", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("direction", "Direction", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
        Pin::input("length", "Length", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([0.0, 1.0, 0.0, 1.0])),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Draw debug sphere
pub static DEBUG_SPHERE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/sphere",
    display_name: "Debug Sphere",
    category: "Debug",
    description: "Draw a debug sphere wireframe",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("center", "Center", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("radius", "Radius", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([0.0, 0.0, 1.0, 1.0])),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Draw debug box
pub static DEBUG_BOX: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/box",
    display_name: "Debug Box",
    category: "Debug",
    description: "Draw a debug box wireframe",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("center", "Center", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("size", "Size", PinType::Vec3).with_default(PinValue::Vec3([1.0, 1.0, 1.0])),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 0.0, 1.0])),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Draw debug capsule
pub static DEBUG_CAPSULE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/capsule",
    display_name: "Debug Capsule",
    category: "Debug",
    description: "Draw a debug capsule wireframe",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("center", "Center", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("radius", "Radius", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("height", "Height", PinType::Float).with_default(PinValue::Float(2.0)),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 0.0, 1.0, 1.0])),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Draw debug point
pub static DEBUG_POINT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/point",
    display_name: "Debug Point",
    category: "Debug",
    description: "Draw a debug point marker",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("size", "Size", PinType::Float).with_default(PinValue::Float(0.1)),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Draw debug arrow
pub static DEBUG_ARROW: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/arrow",
    display_name: "Debug Arrow",
    category: "Debug",
    description: "Draw a debug arrow",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("start", "Start", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("end", "End", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([0.0, 1.0, 0.0, 1.0])),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Draw debug axes
pub static DEBUG_AXES: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/axes",
    display_name: "Debug Axes",
    category: "Debug",
    description: "Draw XYZ axes at a position",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("size", "Size", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Clear debug draws
pub static CLEAR_DEBUG_DRAWS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/clear",
    display_name: "Clear Debug Draws",
    category: "Debug",
    description: "Clear all debug drawings",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// DEBUG TEXT
// =============================================================================

/// Draw debug text 3D
pub static DEBUG_TEXT_3D: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/text_3d",
    display_name: "Debug Text 3D",
    category: "Debug",
    description: "Draw debug text at a world position",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("text", "Text", PinType::String),
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("size", "Size", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Draw debug text 2D
pub static DEBUG_TEXT_2D: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/text_2d",
    display_name: "Debug Text 2D",
    category: "Debug",
    description: "Draw debug text on screen",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("text", "Text", PinType::String),
        Pin::input("position", "Position", PinType::Vec2).with_default(PinValue::Vec2([10.0, 10.0])),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("size", "Size", PinType::Float).with_default(PinValue::Float(16.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// LOGGING
// =============================================================================

/// Log message
pub static LOG_MESSAGE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/log",
    display_name: "Log",
    category: "Debug",
    description: "Log a message to the console",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("message", "Message", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Log warning
pub static LOG_WARNING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/log_warning",
    display_name: "Log Warning",
    category: "Debug",
    description: "Log a warning message",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("message", "Message", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Log error
pub static LOG_ERROR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/log_error",
    display_name: "Log Error",
    category: "Debug",
    description: "Log an error message",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("message", "Message", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Log value
pub static LOG_VALUE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/log_value",
    display_name: "Log Value",
    category: "Debug",
    description: "Log a named value",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("label", "Label", PinType::String),
        Pin::input("value", "Value", PinType::Any),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// PERFORMANCE
// =============================================================================

/// Get FPS
pub static GET_FPS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/get_fps",
    display_name: "Get FPS",
    category: "Debug",
    description: "Get the current frames per second",
    create_pins: || vec![
        Pin::output("fps", "FPS", PinType::Float),
        Pin::output("frame_time", "Frame Time", PinType::Float),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Start timer
pub static START_TIMER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/start_timer",
    display_name: "Start Timer",
    category: "Debug",
    description: "Start a named performance timer",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("name", "Name", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Stop timer
pub static STOP_TIMER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/stop_timer",
    display_name: "Stop Timer",
    category: "Debug",
    description: "Stop a named performance timer and get elapsed time",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("name", "Name", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("elapsed_ms", "Elapsed (ms)", PinType::Float),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Get entity count
pub static GET_ENTITY_COUNT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/entity_count",
    display_name: "Get Entity Count",
    category: "Debug",
    description: "Get the total number of entities in the world",
    create_pins: || vec![
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// ASSERTIONS
// =============================================================================

/// Assert
pub static ASSERT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/assert",
    display_name: "Assert",
    category: "Debug",
    description: "Assert a condition is true",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("condition", "Condition", PinType::Bool),
        Pin::input("message", "Message", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Assert equal
pub static ASSERT_EQUAL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/assert_equal",
    display_name: "Assert Equal",
    category: "Debug",
    description: "Assert two values are equal",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("a", "A", PinType::Any),
        Pin::input("b", "B", PinType::Any),
        Pin::input("message", "Message", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// DEBUG TOGGLES
// =============================================================================

/// Toggle physics debug
pub static TOGGLE_PHYSICS_DEBUG: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/toggle_physics",
    display_name: "Toggle Physics Debug",
    category: "Debug",
    description: "Toggle physics debug visualization",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("enabled", "Enabled", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Toggle wireframe
pub static TOGGLE_WIREFRAME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/toggle_wireframe",
    display_name: "Toggle Wireframe",
    category: "Debug",
    description: "Toggle wireframe rendering mode",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("enabled", "Enabled", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

/// Toggle bounding boxes
pub static TOGGLE_BOUNDING_BOXES: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/toggle_aabb",
    display_name: "Toggle Bounding Boxes",
    category: "Debug",
    description: "Toggle AABB bounding box visualization",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("enabled", "Enabled", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// BREAKPOINTS
// =============================================================================

/// Breakpoint
pub static BREAKPOINT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "debug/breakpoint",
    display_name: "Breakpoint",
    category: "Debug",
    description: "Pause execution (debug builds only)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("enabled", "Enabled", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [255, 200, 100],
    is_event: false,
    is_comment: false,
};
