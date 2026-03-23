//! Camera nodes
//!
//! Nodes for camera control, projection, and screen-space operations.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// CAMERA CONTROL
// =============================================================================

/// Get main camera
pub static GET_MAIN_CAMERA: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/get_main",
    display_name: "Get Main Camera",
    category: "Camera",
    description: "Get the main camera entity",
    create_pins: || vec![
        Pin::output("camera", "Camera", PinType::Entity),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Set main camera
pub static SET_MAIN_CAMERA: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/set_main",
    display_name: "Set Main Camera",
    category: "Camera",
    description: "Set the main camera entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Camera look at
pub static CAMERA_LOOK_AT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/look_at",
    display_name: "Camera Look At",
    category: "Camera",
    description: "Point camera at a target position",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::input("target", "Target", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Camera follow
pub static CAMERA_FOLLOW: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/follow",
    display_name: "Camera Follow",
    category: "Camera",
    description: "Make camera follow a target entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::input("target", "Target", PinType::Entity),
        Pin::input("offset", "Offset", PinType::Vec3).with_default(PinValue::Vec3([0.0, 5.0, -10.0])),
        Pin::input("smooth", "Smoothing", PinType::Float).with_default(PinValue::Float(5.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Camera orbit
pub static CAMERA_ORBIT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/orbit",
    display_name: "Camera Orbit",
    category: "Camera",
    description: "Orbit camera around a target",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::input("target", "Target", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("distance", "Distance", PinType::Float).with_default(PinValue::Float(10.0)),
        Pin::input("yaw", "Yaw", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("pitch", "Pitch", PinType::Float).with_default(PinValue::Float(0.3)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// PROJECTION
// =============================================================================

/// Set perspective projection
pub static SET_PERSPECTIVE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/set_perspective",
    display_name: "Set Perspective",
    category: "Camera",
    description: "Set camera to perspective projection",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::input("fov", "FOV", PinType::Float).with_default(PinValue::Float(60.0)),
        Pin::input("near", "Near", PinType::Float).with_default(PinValue::Float(0.1)),
        Pin::input("far", "Far", PinType::Float).with_default(PinValue::Float(1000.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Set orthographic projection
pub static SET_ORTHOGRAPHIC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/set_orthographic",
    display_name: "Set Orthographic",
    category: "Camera",
    description: "Set camera to orthographic projection",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(10.0)),
        Pin::input("near", "Near", PinType::Float).with_default(PinValue::Float(0.1)),
        Pin::input("far", "Far", PinType::Float).with_default(PinValue::Float(1000.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Set FOV
pub static SET_FOV: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/set_fov",
    display_name: "Set FOV",
    category: "Camera",
    description: "Set camera field of view",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::input("fov", "FOV", PinType::Float).with_default(PinValue::Float(60.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Get FOV
pub static GET_FOV: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/get_fov",
    display_name: "Get FOV",
    category: "Camera",
    description: "Get camera field of view",
    create_pins: || vec![
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::output("fov", "FOV", PinType::Float),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// SCREEN SPACE
// =============================================================================

/// World to screen
pub static WORLD_TO_SCREEN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/world_to_screen",
    display_name: "World to Screen",
    category: "Camera",
    description: "Convert world position to screen coordinates",
    create_pins: || vec![
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::input("world_pos", "World Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("screen_pos", "Screen Position", PinType::Vec2),
        Pin::output("visible", "Is Visible", PinType::Bool),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Screen to world
pub static SCREEN_TO_WORLD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/screen_to_world",
    display_name: "Screen to World",
    category: "Camera",
    description: "Convert screen coordinates to world position (ray)",
    create_pins: || vec![
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::input("screen_pos", "Screen Position", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::output("origin", "Ray Origin", PinType::Vec3),
        Pin::output("direction", "Ray Direction", PinType::Vec3),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Screen to world plane
pub static SCREEN_TO_WORLD_PLANE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/screen_to_plane",
    display_name: "Screen to World Plane",
    category: "Camera",
    description: "Convert screen position to world position on a plane",
    create_pins: || vec![
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::input("screen_pos", "Screen Position", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        Pin::input("plane_normal", "Plane Normal", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
        Pin::input("plane_point", "Plane Point", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("world_pos", "World Position", PinType::Vec3),
        Pin::output("hit", "Hit", PinType::Bool),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Get viewport size
pub static GET_VIEWPORT_SIZE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/get_viewport",
    display_name: "Get Viewport Size",
    category: "Camera",
    description: "Get the camera viewport size in pixels",
    create_pins: || vec![
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::output("width", "Width", PinType::Float),
        Pin::output("height", "Height", PinType::Float),
        Pin::output("size", "Size", PinType::Vec2),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// CAMERA EFFECTS
// =============================================================================

/// Camera shake
pub static CAMERA_SHAKE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/shake",
    display_name: "Camera Shake",
    category: "Camera",
    description: "Apply camera shake effect",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::input("intensity", "Intensity", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("frequency", "Frequency", PinType::Float).with_default(PinValue::Float(20.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Camera zoom
pub static CAMERA_ZOOM: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/zoom",
    display_name: "Camera Zoom",
    category: "Camera",
    description: "Animate camera zoom (FOV or ortho scale)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::input("target", "Target Zoom", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// POST PROCESSING (Future)
// =============================================================================

/// Set camera clear color
pub static SET_CLEAR_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/set_clear_color",
    display_name: "Set Clear Color",
    category: "Camera",
    description: "Set the camera background/clear color",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::input("color", "Color", PinType::Color).with_default(PinValue::Color([0.1, 0.1, 0.1, 1.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Set camera active
pub static SET_CAMERA_ACTIVE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/set_active",
    display_name: "Set Camera Active",
    category: "Camera",
    description: "Enable or disable a camera",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::input("active", "Active", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};

/// Set camera order
pub static SET_CAMERA_ORDER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "camera/set_order",
    display_name: "Set Camera Order",
    category: "Camera",
    description: "Set camera rendering order (for multiple cameras)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("camera", "Camera", PinType::Entity),
        Pin::input("order", "Order", PinType::Int).with_default(PinValue::Int(0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 150, 200],
    is_event: false,
    is_comment: false,
};
