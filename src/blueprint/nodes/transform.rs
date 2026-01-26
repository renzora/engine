//! Transform manipulation nodes

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

/// Get Position - get the entity's world position
pub static GET_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "transform/get_position",
    display_name: "Get Position",
    category: "Transform",
    description: "Get the entity's world position",
    create_pins: || vec![
        Pin::output("position", "Position", PinType::Vec3),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("z", "Z", PinType::Float),
    ],
    color: [200, 150, 100], // Orange for transform
    is_event: false,
    is_comment: false,
};

/// Set Position - set the entity's world position
pub static SET_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "transform/set_position",
    display_name: "Set Position",
    category: "Transform",
    description: "Set the entity's world position",
    create_pins: || vec![
        Pin::input("exec", "", PinType::Flow),
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "", PinType::Flow),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Translate - move the entity by an offset
pub static TRANSLATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "transform/translate",
    display_name: "Translate",
    category: "Transform",
    description: "Move the entity by an offset",
    create_pins: || vec![
        Pin::input("exec", "", PinType::Flow),
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "", PinType::Flow),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Get Rotation - get the entity's rotation (euler angles in degrees)
pub static GET_ROTATION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "transform/get_rotation",
    display_name: "Get Rotation",
    category: "Transform",
    description: "Get the entity's rotation in degrees (euler angles)",
    create_pins: || vec![
        Pin::output("rotation", "Rotation", PinType::Vec3),
        Pin::output("pitch", "Pitch", PinType::Float),
        Pin::output("yaw", "Yaw", PinType::Float),
        Pin::output("roll", "Roll", PinType::Float),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Set Rotation - set the entity's rotation (euler angles in degrees)
pub static SET_ROTATION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "transform/set_rotation",
    display_name: "Set Rotation",
    category: "Transform",
    description: "Set the entity's rotation in degrees (euler angles)",
    create_pins: || vec![
        Pin::input("exec", "", PinType::Flow),
        Pin::input("pitch", "Pitch", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("yaw", "Yaw", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("roll", "Roll", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "", PinType::Flow),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Rotate - rotate the entity by an offset (euler angles in degrees)
pub static ROTATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "transform/rotate",
    display_name: "Rotate",
    category: "Transform",
    description: "Rotate the entity by offset in degrees",
    create_pins: || vec![
        Pin::input("exec", "", PinType::Flow),
        Pin::input("pitch", "Pitch", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("yaw", "Yaw", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("roll", "Roll", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "", PinType::Flow),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};
