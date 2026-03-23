//! Window nodes
//!
//! Nodes for window management, cursor control, and display settings.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// WINDOW PROPERTIES
// =============================================================================

/// Get window size
pub static GET_WINDOW_SIZE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/get_size",
    display_name: "Get Window Size",
    category: "Window",
    description: "Get the window size in pixels",
    create_pins: || vec![
        Pin::output("width", "Width", PinType::Float),
        Pin::output("height", "Height", PinType::Float),
        Pin::output("size", "Size", PinType::Vec2),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Set window size
pub static SET_WINDOW_SIZE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/set_size",
    display_name: "Set Window Size",
    category: "Window",
    description: "Set the window size",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("width", "Width", PinType::Float).with_default(PinValue::Float(1280.0)),
        Pin::input("height", "Height", PinType::Float).with_default(PinValue::Float(720.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Get window position
pub static GET_WINDOW_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/get_position",
    display_name: "Get Window Position",
    category: "Window",
    description: "Get the window position on screen",
    create_pins: || vec![
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("position", "Position", PinType::Vec2),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Set window position
pub static SET_WINDOW_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/set_position",
    display_name: "Set Window Position",
    category: "Window",
    description: "Set the window position on screen",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(100.0)),
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(100.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Center window
pub static CENTER_WINDOW: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/center",
    display_name: "Center Window",
    category: "Window",
    description: "Center the window on screen",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Get window title
pub static GET_WINDOW_TITLE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/get_title",
    display_name: "Get Window Title",
    category: "Window",
    description: "Get the window title",
    create_pins: || vec![
        Pin::output("title", "Title", PinType::String),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Set window title
pub static SET_WINDOW_TITLE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/set_title",
    display_name: "Set Window Title",
    category: "Window",
    description: "Set the window title",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("title", "Title", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// WINDOW MODES
// =============================================================================

/// Set fullscreen
pub static SET_FULLSCREEN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/set_fullscreen",
    display_name: "Set Fullscreen",
    category: "Window",
    description: "Set fullscreen mode",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("fullscreen", "Fullscreen", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Toggle fullscreen
pub static TOGGLE_FULLSCREEN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/toggle_fullscreen",
    display_name: "Toggle Fullscreen",
    category: "Window",
    description: "Toggle fullscreen mode",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("fullscreen", "Is Fullscreen", PinType::Bool),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Is fullscreen
pub static IS_FULLSCREEN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/is_fullscreen",
    display_name: "Is Fullscreen",
    category: "Window",
    description: "Check if window is fullscreen",
    create_pins: || vec![
        Pin::output("fullscreen", "Is Fullscreen", PinType::Bool),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Set borderless
pub static SET_BORDERLESS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/set_borderless",
    display_name: "Set Borderless",
    category: "Window",
    description: "Set borderless fullscreen mode",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("borderless", "Borderless", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Minimize window
pub static MINIMIZE_WINDOW: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/minimize",
    display_name: "Minimize Window",
    category: "Window",
    description: "Minimize the window",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Maximize window
pub static MAXIMIZE_WINDOW: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/maximize",
    display_name: "Maximize Window",
    category: "Window",
    description: "Maximize the window",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Restore window
pub static RESTORE_WINDOW: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/restore",
    display_name: "Restore Window",
    category: "Window",
    description: "Restore window from minimized/maximized",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Is minimized
pub static IS_MINIMIZED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/is_minimized",
    display_name: "Is Minimized",
    category: "Window",
    description: "Check if window is minimized",
    create_pins: || vec![
        Pin::output("minimized", "Is Minimized", PinType::Bool),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Is maximized
pub static IS_MAXIMIZED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/is_maximized",
    display_name: "Is Maximized",
    category: "Window",
    description: "Check if window is maximized",
    create_pins: || vec![
        Pin::output("maximized", "Is Maximized", PinType::Bool),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// WINDOW DECORATIONS
// =============================================================================

/// Set window resizable
pub static SET_RESIZABLE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/set_resizable",
    display_name: "Set Resizable",
    category: "Window",
    description: "Set whether window can be resized",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("resizable", "Resizable", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Set decorations
pub static SET_DECORATIONS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/set_decorations",
    display_name: "Set Decorations",
    category: "Window",
    description: "Show or hide window decorations (title bar, borders)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("decorations", "Decorations", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Set always on top
pub static SET_ALWAYS_ON_TOP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/set_always_on_top",
    display_name: "Set Always On Top",
    category: "Window",
    description: "Set window to stay on top of other windows",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("on_top", "On Top", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// CURSOR
// =============================================================================

/// Get cursor position
pub static GET_CURSOR_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/get_cursor",
    display_name: "Get Cursor Position",
    category: "Window",
    description: "Get cursor position in window coordinates",
    create_pins: || vec![
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("position", "Position", PinType::Vec2),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Set cursor position
pub static SET_CURSOR_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/set_cursor",
    display_name: "Set Cursor Position",
    category: "Window",
    description: "Set cursor position in window coordinates",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("x", "X", PinType::Float),
        Pin::input("y", "Y", PinType::Float),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Show cursor
pub static SHOW_CURSOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/show_cursor",
    display_name: "Show Cursor",
    category: "Window",
    description: "Show the cursor",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Hide cursor
pub static HIDE_CURSOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/hide_cursor",
    display_name: "Hide Cursor",
    category: "Window",
    description: "Hide the cursor",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Lock cursor
pub static LOCK_CURSOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/lock_cursor",
    display_name: "Lock Cursor",
    category: "Window",
    description: "Lock cursor to window center",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("locked", "Locked", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Confine cursor
pub static CONFINE_CURSOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/confine_cursor",
    display_name: "Confine Cursor",
    category: "Window",
    description: "Confine cursor to window bounds",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("confined", "Confined", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Set cursor icon
pub static SET_CURSOR_ICON: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/set_cursor_icon",
    display_name: "Set Cursor Icon",
    category: "Window",
    description: "Set the cursor icon",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("icon", "Icon", PinType::String).with_default(PinValue::String("default".into())),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// DISPLAY INFO
// =============================================================================

/// Get primary monitor size
pub static GET_MONITOR_SIZE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/get_monitor_size",
    display_name: "Get Monitor Size",
    category: "Window",
    description: "Get the primary monitor resolution",
    create_pins: || vec![
        Pin::output("width", "Width", PinType::Float),
        Pin::output("height", "Height", PinType::Float),
        Pin::output("size", "Size", PinType::Vec2),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Get monitor count
pub static GET_MONITOR_COUNT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/get_monitor_count",
    display_name: "Get Monitor Count",
    category: "Window",
    description: "Get the number of connected monitors",
    create_pins: || vec![
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Get scale factor
pub static GET_SCALE_FACTOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/get_scale_factor",
    display_name: "Get Scale Factor",
    category: "Window",
    description: "Get the window DPI scale factor",
    create_pins: || vec![
        Pin::output("scale", "Scale", PinType::Float),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// WINDOW EVENTS
// =============================================================================

/// On window resized
pub static ON_WINDOW_RESIZED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/on_resized",
    display_name: "On Window Resized",
    category: "Window Events",
    description: "Triggered when window is resized",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("width", "Width", PinType::Float),
        Pin::output("height", "Height", PinType::Float),
    ],
    color: [180, 180, 200],
    is_event: true,
    is_comment: false,
};

/// On window moved
pub static ON_WINDOW_MOVED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/on_moved",
    display_name: "On Window Moved",
    category: "Window Events",
    description: "Triggered when window is moved",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
    ],
    color: [180, 180, 200],
    is_event: true,
    is_comment: false,
};

/// On window focused
pub static ON_WINDOW_FOCUSED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/on_focused",
    display_name: "On Window Focused",
    category: "Window Events",
    description: "Triggered when window gains or loses focus",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("focused", "Is Focused", PinType::Bool),
    ],
    color: [180, 180, 200],
    is_event: true,
    is_comment: false,
};

/// Is window focused
pub static IS_WINDOW_FOCUSED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/is_focused",
    display_name: "Is Window Focused",
    category: "Window",
    description: "Check if window has focus",
    create_pins: || vec![
        Pin::output("focused", "Is Focused", PinType::Bool),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// On close requested
pub static ON_CLOSE_REQUESTED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/on_close_requested",
    display_name: "On Close Requested",
    category: "Window Events",
    description: "Triggered when window close is requested",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: true,
    is_comment: false,
};

// =============================================================================
// VSYNC
// =============================================================================

/// Set VSync
pub static SET_VSYNC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/set_vsync",
    display_name: "Set VSync",
    category: "Window",
    description: "Enable or disable VSync",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("enabled", "Enabled", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};

/// Is VSync enabled
pub static IS_VSYNC_ENABLED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "window/is_vsync",
    display_name: "Is VSync Enabled",
    category: "Window",
    description: "Check if VSync is enabled",
    create_pins: || vec![
        Pin::output("enabled", "Enabled", PinType::Bool),
    ],
    color: [180, 180, 200],
    is_event: false,
    is_comment: false,
};
