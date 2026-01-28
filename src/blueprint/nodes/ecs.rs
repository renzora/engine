//! ECS (Entity Component System) nodes
//!
//! Nodes for entity management, component operations, and queries.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// ENTITY MANAGEMENT
// =============================================================================

/// Spawn a new entity
pub static SPAWN_ENTITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/spawn_entity",
    display_name: "Spawn Entity",
    category: "ECS",
    description: "Create a new entity in the world",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("name", "Name", PinType::String).with_default(PinValue::String("Entity".into())),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Despawn an entity
pub static DESPAWN_ENTITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/despawn_entity",
    display_name: "Despawn Entity",
    category: "ECS",
    description: "Remove an entity from the world",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("recursive", "Recursive", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Get self entity (the entity this script is attached to)
pub static SELF_ENTITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/self",
    display_name: "Self",
    category: "ECS",
    description: "Get the entity this script is attached to",
    create_pins: || vec![
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Check if entity is valid
pub static ENTITY_VALID: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/entity_valid",
    display_name: "Entity Valid",
    category: "ECS",
    description: "Check if an entity reference is still valid",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("valid", "Valid", PinType::Bool),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Get entity by name
pub static FIND_ENTITY_BY_NAME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/find_by_name",
    display_name: "Find Entity By Name",
    category: "ECS",
    description: "Find an entity by its name",
    create_pins: || vec![
        Pin::input("name", "Name", PinType::String),
        Pin::output("entity", "Entity", PinType::Entity),
        Pin::output("found", "Found", PinType::Bool),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Get entity name
pub static GET_ENTITY_NAME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/get_name",
    display_name: "Get Entity Name",
    category: "ECS",
    description: "Get the name of an entity",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("name", "Name", PinType::String),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Set entity name
pub static SET_ENTITY_NAME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/set_name",
    display_name: "Set Entity Name",
    category: "ECS",
    description: "Set the name of an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("name", "Name", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// COMPONENT OPERATIONS
// =============================================================================

/// Add component to entity
pub static ADD_COMPONENT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/add_component",
    display_name: "Add Component",
    category: "ECS",
    description: "Add a component to an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("component", "Component", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Remove component from entity
pub static REMOVE_COMPONENT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/remove_component",
    display_name: "Remove Component",
    category: "ECS",
    description: "Remove a component from an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("component", "Component", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Check if entity has component
pub static HAS_COMPONENT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/has_component",
    display_name: "Has Component",
    category: "ECS",
    description: "Check if an entity has a specific component",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("component", "Component", PinType::String),
        Pin::output("has", "Has", PinType::Bool),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// TAGS
// =============================================================================

/// Add tag to entity
pub static ADD_TAG: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/add_tag",
    display_name: "Add Tag",
    category: "ECS",
    description: "Add a tag to an entity for easy identification",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("tag", "Tag", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Remove tag from entity
pub static REMOVE_TAG: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/remove_tag",
    display_name: "Remove Tag",
    category: "ECS",
    description: "Remove a tag from an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("tag", "Tag", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Check if entity has tag
pub static HAS_TAG: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/has_tag",
    display_name: "Has Tag",
    category: "ECS",
    description: "Check if an entity has a specific tag",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("tag", "Tag", PinType::String),
        Pin::output("has", "Has", PinType::Bool),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Find entities with tag
pub static FIND_BY_TAG: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/find_by_tag",
    display_name: "Find By Tag",
    category: "ECS",
    description: "Find all entities with a specific tag",
    create_pins: || vec![
        Pin::input("tag", "Tag", PinType::String),
        Pin::output("entities", "Entities", PinType::EntityArray),
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// QUERIES
// =============================================================================

/// Get all entities
pub static GET_ALL_ENTITIES: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/get_all_entities",
    display_name: "Get All Entities",
    category: "ECS",
    description: "Get all entities in the world",
    create_pins: || vec![
        Pin::output("entities", "Entities", PinType::EntityArray),
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

/// For each entity in list
pub static FOR_EACH_ENTITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/for_each_entity",
    display_name: "For Each Entity",
    category: "ECS",
    description: "Execute for each entity in a list",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entities", "Entities", PinType::EntityArray),
        Pin::output("loop", "Loop Body", PinType::Execution),
        Pin::output("entity", "Current Entity", PinType::Entity),
        Pin::output("index", "Index", PinType::Int),
        Pin::output("done", "Done", PinType::Execution),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Get closest entity
pub static GET_CLOSEST_ENTITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/get_closest",
    display_name: "Get Closest Entity",
    category: "ECS",
    description: "Find the closest entity from a list to a position",
    create_pins: || vec![
        Pin::input("entities", "Entities", PinType::EntityArray),
        Pin::input("position", "Position", PinType::Vec3),
        Pin::output("entity", "Closest", PinType::Entity),
        Pin::output("distance", "Distance", PinType::Float),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};

/// Get entities in radius
pub static GET_ENTITIES_IN_RADIUS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "ecs/get_in_radius",
    display_name: "Get Entities In Radius",
    category: "ECS",
    description: "Find all entities within a radius of a position",
    create_pins: || vec![
        Pin::input("position", "Position", PinType::Vec3),
        Pin::input("radius", "Radius", PinType::Float).with_default(PinValue::Float(10.0)),
        Pin::output("entities", "Entities", PinType::EntityArray),
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [80, 180, 220],
    is_event: false,
    is_comment: false,
};
