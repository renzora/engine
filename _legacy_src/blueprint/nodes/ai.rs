//! AI and pathfinding nodes
//!
//! Nodes for AI behavior, navigation, and pathfinding.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// PATHFINDING
// =============================================================================

/// Find path between two points
pub static FIND_PATH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/find_path",
    display_name: "Find Path",
    category: "AI",
    description: "Find a path from start to end position",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("start_x", "Start X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("start_y", "Start Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("start_z", "Start Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("end_x", "End X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("end_y", "End Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("end_z", "End Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("path", "Path", PinType::EntityArray),
        Pin::output("found", "Found", PinType::Bool),
        Pin::output("length", "Path Length", PinType::Int),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

/// Get next waypoint in path
pub static GET_NEXT_WAYPOINT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/next_waypoint",
    display_name: "Get Next Waypoint",
    category: "AI",
    description: "Get the next waypoint in a path",
    create_pins: || vec![
        Pin::input("path", "Path", PinType::EntityArray),
        Pin::input("current_index", "Current Index", PinType::Int).with_default(PinValue::Int(0)),
        Pin::output("x", "X", PinType::Float),
        Pin::output("y", "Y", PinType::Float),
        Pin::output("z", "Z", PinType::Float),
        Pin::output("next_index", "Next Index", PinType::Int),
        Pin::output("is_last", "Is Last", PinType::Bool),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

/// Check if point is reachable
pub static IS_REACHABLE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/is_reachable",
    display_name: "Is Reachable",
    category: "AI",
    description: "Check if a destination is reachable from current position",
    create_pins: || vec![
        Pin::input("from_x", "From X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("from_y", "From Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("from_z", "From Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("to_x", "To X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("to_y", "To Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("to_z", "To Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("reachable", "Reachable", PinType::Bool),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// MOVEMENT
// =============================================================================

/// Move entity towards target
pub static MOVE_TO: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/move_to",
    display_name: "Move To",
    category: "AI",
    description: "Move an entity towards a target position",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("target_x", "Target X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("target_y", "Target Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("target_z", "Target Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(5.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("reached", "Reached", PinType::Bool),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

/// Move along path
pub static MOVE_ALONG_PATH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/move_along_path",
    display_name: "Move Along Path",
    category: "AI",
    description: "Move an entity along a computed path",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("path", "Path", PinType::EntityArray),
        Pin::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(5.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("waypoint_reached", "Waypoint Reached", PinType::Execution),
        Pin::output("path_completed", "Path Completed", PinType::Execution),
        Pin::output("current_index", "Current Index", PinType::Int),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

/// Stop movement
pub static STOP_MOVEMENT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/stop_movement",
    display_name: "Stop Movement",
    category: "AI",
    description: "Stop an entity's movement",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// LOOK AT / FACING
// =============================================================================

/// Look at target position
pub static LOOK_AT_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/look_at_position",
    display_name: "Look At Position",
    category: "AI",
    description: "Make an entity face towards a position",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("target_x", "Target X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("target_y", "Target Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("target_z", "Target Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(10.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

/// Look at target entity
pub static LOOK_AT_TARGET: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/look_at_target",
    display_name: "Look At Target",
    category: "AI",
    description: "Make an entity face towards another entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("target", "Target", PinType::Entity),
        Pin::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(10.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

/// Check if entity is facing target
pub static IS_FACING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/is_facing",
    display_name: "Is Facing",
    category: "AI",
    description: "Check if an entity is facing a target within a threshold angle",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("target_x", "Target X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("target_y", "Target Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("target_z", "Target Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("threshold", "Threshold (deg)", PinType::Float).with_default(PinValue::Float(15.0)),
        Pin::output("is_facing", "Is Facing", PinType::Bool),
        Pin::output("angle", "Angle (deg)", PinType::Float),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// DISTANCE & RANGE
// =============================================================================

/// Distance to target
pub static DISTANCE_TO_TARGET: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/distance_to_target",
    display_name: "Distance To Target",
    category: "AI",
    description: "Get the distance from an entity to a target",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("target", "Target", PinType::Entity),
        Pin::output("distance", "Distance", PinType::Float),
        Pin::output("distance_2d", "Distance 2D", PinType::Float),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

/// Distance to position
pub static DISTANCE_TO_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/distance_to_position",
    display_name: "Distance To Position",
    category: "AI",
    description: "Get the distance from an entity to a position",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("distance", "Distance", PinType::Float),
        Pin::output("distance_2d", "Distance 2D", PinType::Float),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

/// Check if in range
pub static IS_IN_RANGE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/is_in_range",
    display_name: "Is In Range",
    category: "AI",
    description: "Check if an entity is within range of another",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("target", "Target", PinType::Entity),
        Pin::input("range", "Range", PinType::Float).with_default(PinValue::Float(10.0)),
        Pin::output("in_range", "In Range", PinType::Bool),
        Pin::output("distance", "Distance", PinType::Float),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// PERCEPTION
// =============================================================================

/// Check line of sight
pub static HAS_LINE_OF_SIGHT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/line_of_sight",
    display_name: "Has Line Of Sight",
    category: "AI",
    description: "Check if there's an unobstructed line of sight between two entities",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("target", "Target", PinType::Entity),
        Pin::output("has_los", "Has LOS", PinType::Bool),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

/// Find nearest entity
pub static FIND_NEAREST: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/find_nearest",
    display_name: "Find Nearest",
    category: "AI",
    description: "Find the nearest entity from a list",
    create_pins: || vec![
        Pin::input("from", "From", PinType::Entity),
        Pin::input("entities", "Entities", PinType::EntityArray),
        Pin::output("nearest", "Nearest", PinType::Entity),
        Pin::output("distance", "Distance", PinType::Float),
        Pin::output("found", "Found", PinType::Bool),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

/// Find entities in range
pub static FIND_IN_RANGE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/find_in_range",
    display_name: "Find In Range",
    category: "AI",
    description: "Find all entities within a range",
    create_pins: || vec![
        Pin::input("from", "From", PinType::Entity),
        Pin::input("entities", "Entities", PinType::EntityArray),
        Pin::input("range", "Range", PinType::Float).with_default(PinValue::Float(10.0)),
        Pin::output("in_range", "In Range", PinType::EntityArray),
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// STEERING / AVOIDANCE
// =============================================================================

/// Flee from position
pub static FLEE_FROM: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/flee_from",
    display_name: "Flee From",
    category: "AI",
    description: "Move an entity away from a position",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("from_x", "From X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("from_y", "From Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("from_z", "From Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(5.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("direction_x", "Direction X", PinType::Float),
        Pin::output("direction_y", "Direction Y", PinType::Float),
        Pin::output("direction_z", "Direction Z", PinType::Float),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

/// Wander randomly
pub static WANDER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/wander",
    display_name: "Wander",
    category: "AI",
    description: "Move an entity in a random direction",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("radius", "Wander Radius", PinType::Float).with_default(PinValue::Float(5.0)),
        Pin::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(2.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("target_x", "Target X", PinType::Float),
        Pin::output("target_y", "Target Y", PinType::Float),
        Pin::output("target_z", "Target Z", PinType::Float),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

/// Patrol between waypoints
pub static PATROL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/patrol",
    display_name: "Patrol",
    category: "AI",
    description: "Move an entity between a set of patrol points",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("waypoints", "Waypoints", PinType::EntityArray),
        Pin::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(3.0)),
        Pin::input("loop", "Loop", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("waypoint_reached", "Waypoint Reached", PinType::Execution),
        Pin::output("current_waypoint", "Current Waypoint", PinType::Int),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// STATE MACHINE HELPERS
// =============================================================================

/// Set AI state
pub static SET_AI_STATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/set_state",
    display_name: "Set AI State",
    category: "AI",
    description: "Set the AI state for an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("state", "State", PinType::String).with_default(PinValue::String("idle".into())),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

/// Get AI state
pub static GET_AI_STATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/get_state",
    display_name: "Get AI State",
    category: "AI",
    description: "Get the AI state for an entity",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("state", "State", PinType::String),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};

/// Is AI state
pub static IS_AI_STATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ai/is_state",
    display_name: "Is AI State",
    category: "AI",
    description: "Check if the AI is in a specific state",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("state", "State", PinType::String).with_default(PinValue::String("idle".into())),
        Pin::output("is_state", "Is State", PinType::Bool),
    ],
    color: [180, 100, 180],
    is_event: false,
    is_comment: false,
};
