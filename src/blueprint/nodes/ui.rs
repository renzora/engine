//! UI nodes
//!
//! Nodes for creating and manipulating UI elements.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// TEXT
// =============================================================================

/// Spawn text
pub static SPAWN_TEXT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/spawn_text",
    display_name: "Spawn Text",
    category: "UI",
    description: "Create a text UI element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("text", "Text", PinType::String).with_default(PinValue::String("Hello".into())),
        Pin::input("font_size", "Font Size", PinType::Float).with_default(PinValue::Float(32.0)),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("position", "Position", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Set text content
pub static SET_TEXT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_text",
    display_name: "Set Text",
    category: "UI",
    description: "Set the text content of a text element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("text", "Text", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Get text content
pub static GET_TEXT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/get_text",
    display_name: "Get Text",
    category: "UI",
    description: "Get the text content of a text element",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("text", "Text", PinType::String),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Set text color
pub static SET_TEXT_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_text_color",
    display_name: "Set Text Color",
    category: "UI",
    description: "Set the color of a text element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Set font size
pub static SET_FONT_SIZE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_font_size",
    display_name: "Set Font Size",
    category: "UI",
    description: "Set the font size of a text element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("size", "Size", PinType::Float).with_default(PinValue::Float(32.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// BUTTONS
// =============================================================================

/// Spawn button
pub static SPAWN_BUTTON: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/spawn_button",
    display_name: "Spawn Button",
    category: "UI",
    description: "Create a button UI element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("text", "Text", PinType::String).with_default(PinValue::String("Button".into())),
        Pin::input("position", "Position", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("size", "Size", PinType::Vec2).with_default(PinValue::Vec2([150.0, 50.0])),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// On button clicked
pub static ON_BUTTON_CLICKED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/on_button_clicked",
    display_name: "On Button Clicked",
    category: "UI Events",
    description: "Triggered when a button is clicked",
    create_pins: || vec![
        Pin::input("button", "Button", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: true,
    is_comment: false,
};

/// On button hovered
pub static ON_BUTTON_HOVERED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/on_button_hovered",
    display_name: "On Button Hovered",
    category: "UI Events",
    description: "Triggered when a button is hovered",
    create_pins: || vec![
        Pin::input("button", "Button", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("hovered", "Is Hovered", PinType::Bool),
    ],
    color: [100, 200, 200],
    is_event: true,
    is_comment: false,
};

/// Set button enabled
pub static SET_BUTTON_ENABLED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_button_enabled",
    display_name: "Set Button Enabled",
    category: "UI",
    description: "Enable or disable a button",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("button", "Button", PinType::Entity),
        Pin::input("enabled", "Enabled", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// IMAGES
// =============================================================================

/// Spawn UI image
pub static SPAWN_UI_IMAGE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/spawn_image",
    display_name: "Spawn UI Image",
    category: "UI",
    description: "Create an image UI element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("image", "Image", PinType::Asset),
        Pin::input("position", "Position", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("size", "Size", PinType::Vec2).with_default(PinValue::Vec2([100.0, 100.0])),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Set UI image
pub static SET_UI_IMAGE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_image",
    display_name: "Set UI Image",
    category: "UI",
    description: "Set the image of a UI image element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("image", "Image", PinType::Asset),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Set image color tint
pub static SET_IMAGE_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_image_color",
    display_name: "Set Image Color",
    category: "UI",
    description: "Set the color tint of a UI image",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// CONTAINERS/LAYOUT
// =============================================================================

/// Spawn UI node (container)
pub static SPAWN_UI_NODE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/spawn_node",
    display_name: "Spawn UI Node",
    category: "UI",
    description: "Create a UI node container",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("position", "Position", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("size", "Size", PinType::Vec2).with_default(PinValue::Vec2([100.0, 100.0])),
        Pin::input("background", "Background", PinType::Color).with_default(PinValue::Color([0.0, 0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Set UI position
pub static SET_UI_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_position",
    display_name: "Set UI Position",
    category: "UI",
    description: "Set the position of a UI element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("position", "Position", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Set UI size
pub static SET_UI_SIZE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_size",
    display_name: "Set UI Size",
    category: "UI",
    description: "Set the size of a UI element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("width", "Width", PinType::Float).with_default(PinValue::Float(100.0)),
        Pin::input("height", "Height", PinType::Float).with_default(PinValue::Float(100.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Get UI size
pub static GET_UI_SIZE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/get_size",
    display_name: "Get UI Size",
    category: "UI",
    description: "Get the size of a UI element",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("width", "Width", PinType::Float),
        Pin::output("height", "Height", PinType::Float),
        Pin::output("size", "Size", PinType::Vec2),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Set background color
pub static SET_BACKGROUND_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_background",
    display_name: "Set Background Color",
    category: "UI",
    description: "Set the background color of a UI element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([0.2, 0.2, 0.2, 1.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Set border
pub static SET_UI_BORDER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_border",
    display_name: "Set Border",
    category: "UI",
    description: "Set the border of a UI element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("width", "Width", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Set border radius
pub static SET_BORDER_RADIUS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_border_radius",
    display_name: "Set Border Radius",
    category: "UI",
    description: "Set the border radius of a UI element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("radius", "Radius", PinType::Float).with_default(PinValue::Float(5.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// UI VISIBILITY
// =============================================================================

/// Set UI visibility
pub static SET_UI_VISIBILITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_visibility",
    display_name: "Set UI Visibility",
    category: "UI",
    description: "Show or hide a UI element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("visible", "Visible", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Toggle UI visibility
pub static TOGGLE_UI_VISIBILITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/toggle_visibility",
    display_name: "Toggle UI Visibility",
    category: "UI",
    description: "Toggle visibility of a UI element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("visible", "Is Visible", PinType::Bool),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// INPUT FIELDS
// =============================================================================

/// Spawn text input
pub static SPAWN_TEXT_INPUT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/spawn_text_input",
    display_name: "Spawn Text Input",
    category: "UI",
    description: "Create a text input field",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("placeholder", "Placeholder", PinType::String).with_default(PinValue::String("Enter text...".into())),
        Pin::input("position", "Position", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("size", "Size", PinType::Vec2).with_default(PinValue::Vec2([200.0, 40.0])),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Get text input value
pub static GET_TEXT_INPUT_VALUE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/get_input_value",
    display_name: "Get Input Value",
    category: "UI",
    description: "Get the current value of a text input",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("value", "Value", PinType::String),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Set text input value
pub static SET_TEXT_INPUT_VALUE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_input_value",
    display_name: "Set Input Value",
    category: "UI",
    description: "Set the value of a text input",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("value", "Value", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// On text input changed
pub static ON_TEXT_INPUT_CHANGED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/on_input_changed",
    display_name: "On Input Changed",
    category: "UI Events",
    description: "Triggered when text input value changes",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("value", "Value", PinType::String),
    ],
    color: [100, 200, 200],
    is_event: true,
    is_comment: false,
};

/// On text input submitted
pub static ON_TEXT_INPUT_SUBMITTED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/on_input_submitted",
    display_name: "On Input Submitted",
    category: "UI Events",
    description: "Triggered when text input is submitted (Enter pressed)",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("value", "Value", PinType::String),
    ],
    color: [100, 200, 200],
    is_event: true,
    is_comment: false,
};

// =============================================================================
// SLIDERS
// =============================================================================

/// Spawn slider
pub static SPAWN_SLIDER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/spawn_slider",
    display_name: "Spawn Slider",
    category: "UI",
    description: "Create a slider UI element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("min", "Min", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("max", "Max", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("value", "Initial Value", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("position", "Position", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Get slider value
pub static GET_SLIDER_VALUE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/get_slider_value",
    display_name: "Get Slider Value",
    category: "UI",
    description: "Get the current value of a slider",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("value", "Value", PinType::Float),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Set slider value
pub static SET_SLIDER_VALUE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_slider_value",
    display_name: "Set Slider Value",
    category: "UI",
    description: "Set the value of a slider",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("value", "Value", PinType::Float),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// On slider changed
pub static ON_SLIDER_CHANGED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/on_slider_changed",
    display_name: "On Slider Changed",
    category: "UI Events",
    description: "Triggered when slider value changes",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("value", "Value", PinType::Float),
    ],
    color: [100, 200, 200],
    is_event: true,
    is_comment: false,
};

// =============================================================================
// PROGRESS BAR
// =============================================================================

/// Spawn progress bar
pub static SPAWN_PROGRESS_BAR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/spawn_progress_bar",
    display_name: "Spawn Progress Bar",
    category: "UI",
    description: "Create a progress bar UI element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("position", "Position", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("size", "Size", PinType::Vec2).with_default(PinValue::Vec2([200.0, 20.0])),
        Pin::input("color", "Fill Color", PinType::Color).with_default(PinValue::Color([0.2, 0.8, 0.2, 1.0])),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Set progress bar value
pub static SET_PROGRESS_VALUE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_progress_value",
    display_name: "Set Progress Value",
    category: "UI",
    description: "Set the value of a progress bar (0-1)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// UI PARENTING
// =============================================================================

/// Add UI child
pub static ADD_UI_CHILD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/add_child",
    display_name: "Add UI Child",
    category: "UI",
    description: "Add a UI element as child of another",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("parent", "Parent", PinType::Entity),
        Pin::input("child", "Child", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Remove UI child
pub static REMOVE_UI_CHILD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/remove_child",
    display_name: "Remove UI Child",
    category: "UI",
    description: "Remove a UI element from its parent",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("child", "Child", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// UI Z-ORDER
// =============================================================================

/// Set Z-index
pub static SET_Z_INDEX: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/set_z_index",
    display_name: "Set Z-Index",
    category: "UI",
    description: "Set the z-order of a UI element",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("z_index", "Z-Index", PinType::Int).with_default(PinValue::Int(0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Bring to front
pub static BRING_TO_FRONT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/bring_to_front",
    display_name: "Bring To Front",
    category: "UI",
    description: "Bring a UI element to the front",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};

/// Send to back
pub static SEND_TO_BACK: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ui/send_to_back",
    display_name: "Send To Back",
    category: "UI",
    description: "Send a UI element to the back",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 200, 200],
    is_event: false,
    is_comment: false,
};
