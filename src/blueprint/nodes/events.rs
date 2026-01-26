//! Event nodes - entry points for blueprint execution

use super::{NodeTypeDefinition, Pin, PinType};

/// On Ready - called once when the entity spawns
pub static ON_READY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "event/on_ready",
    display_name: "On Ready",
    category: "Events",
    description: "Called once when the entity spawns or the scene loads",
    create_pins: || vec![
        Pin::output("exec", "", PinType::Flow),
    ],
    color: [200, 50, 50], // Red accent for events
    is_event: true,
    is_comment: false,
};

/// On Update - called every frame
pub static ON_UPDATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "event/on_update",
    display_name: "On Update",
    category: "Events",
    description: "Called every frame during gameplay",
    create_pins: || vec![
        Pin::output("exec", "", PinType::Flow),
        Pin::output("delta", "Delta", PinType::Float),
    ],
    color: [200, 50, 50], // Red accent for events
    is_event: true,
    is_comment: false,
};
