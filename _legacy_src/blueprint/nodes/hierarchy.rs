//! Hierarchy nodes
//!
//! Nodes for parent-child relationships and scene hierarchy.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// PARENTING
// =============================================================================

/// Set parent
pub static SET_PARENT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/set_parent",
    display_name: "Set Parent",
    category: "Hierarchy",
    description: "Set the parent of an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("parent", "Parent", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Remove parent (make root)
pub static REMOVE_PARENT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/remove_parent",
    display_name: "Remove Parent",
    category: "Hierarchy",
    description: "Remove the parent of an entity (make it a root entity)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Get parent
pub static GET_PARENT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/get_parent",
    display_name: "Get Parent",
    category: "Hierarchy",
    description: "Get the parent entity of an entity",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("parent", "Parent", PinType::Entity),
        Pin::output("has_parent", "Has Parent", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Has parent
pub static HAS_PARENT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/has_parent",
    display_name: "Has Parent",
    category: "Hierarchy",
    description: "Check if an entity has a parent",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("has_parent", "Has Parent", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// CHILDREN
// =============================================================================

/// Add child
pub static ADD_CHILD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/add_child",
    display_name: "Add Child",
    category: "Hierarchy",
    description: "Add an entity as a child of another",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("parent", "Parent", PinType::Entity),
        Pin::input("child", "Child", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Remove child
pub static REMOVE_CHILD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/remove_child",
    display_name: "Remove Child",
    category: "Hierarchy",
    description: "Remove a child entity from its parent",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("parent", "Parent", PinType::Entity),
        Pin::input("child", "Child", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Get children
pub static GET_CHILDREN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/get_children",
    display_name: "Get Children",
    category: "Hierarchy",
    description: "Get all children of an entity",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("children", "Children", PinType::EntityArray),
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Get child at index
pub static GET_CHILD_AT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/get_child_at",
    display_name: "Get Child At",
    category: "Hierarchy",
    description: "Get a child at a specific index",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("index", "Index", PinType::Int).with_default(PinValue::Int(0)),
        Pin::output("child", "Child", PinType::Entity),
        Pin::output("valid", "Valid", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Get child count
pub static GET_CHILD_COUNT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/get_child_count",
    display_name: "Get Child Count",
    category: "Hierarchy",
    description: "Get the number of children an entity has",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Has children
pub static HAS_CHILDREN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/has_children",
    display_name: "Has Children",
    category: "Hierarchy",
    description: "Check if an entity has children",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("has_children", "Has Children", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// For each child
pub static FOR_EACH_CHILD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/for_each_child",
    display_name: "For Each Child",
    category: "Hierarchy",
    description: "Iterate over all children of an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("loop", "Loop Body", PinType::Execution),
        Pin::output("child", "Child", PinType::Entity),
        Pin::output("index", "Index", PinType::Int),
        Pin::output("completed", "Completed", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// HIERARCHY QUERIES
// =============================================================================

/// Get root
pub static GET_ROOT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/get_root",
    display_name: "Get Root",
    category: "Hierarchy",
    description: "Get the root entity of a hierarchy",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("root", "Root", PinType::Entity),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Is root
pub static IS_ROOT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/is_root",
    display_name: "Is Root",
    category: "Hierarchy",
    description: "Check if an entity is a root (has no parent)",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("is_root", "Is Root", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Is ancestor of
pub static IS_ANCESTOR_OF: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/is_ancestor_of",
    display_name: "Is Ancestor Of",
    category: "Hierarchy",
    description: "Check if an entity is an ancestor of another",
    create_pins: || vec![
        Pin::input("ancestor", "Ancestor", PinType::Entity),
        Pin::input("descendant", "Descendant", PinType::Entity),
        Pin::output("is_ancestor", "Is Ancestor", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Is descendant of
pub static IS_DESCENDANT_OF: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/is_descendant_of",
    display_name: "Is Descendant Of",
    category: "Hierarchy",
    description: "Check if an entity is a descendant of another",
    create_pins: || vec![
        Pin::input("descendant", "Descendant", PinType::Entity),
        Pin::input("ancestor", "Ancestor", PinType::Entity),
        Pin::output("is_descendant", "Is Descendant", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Get all descendants
pub static GET_ALL_DESCENDANTS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/get_all_descendants",
    display_name: "Get All Descendants",
    category: "Hierarchy",
    description: "Get all descendants of an entity (recursive)",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("descendants", "Descendants", PinType::EntityArray),
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Get depth
pub static GET_DEPTH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/get_depth",
    display_name: "Get Depth",
    category: "Hierarchy",
    description: "Get the depth of an entity in the hierarchy (0 = root)",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("depth", "Depth", PinType::Int),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// HIERARCHY TRANSFORMS
// =============================================================================

/// Get local position
pub static GET_LOCAL_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/get_local_position",
    display_name: "Get Local Position",
    category: "Hierarchy",
    description: "Get the local position relative to parent",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("position", "Position", PinType::Vec3),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Set local position
pub static SET_LOCAL_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/set_local_position",
    display_name: "Set Local Position",
    category: "Hierarchy",
    description: "Set the local position relative to parent",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Get local rotation
pub static GET_LOCAL_ROTATION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/get_local_rotation",
    display_name: "Get Local Rotation",
    category: "Hierarchy",
    description: "Get the local rotation relative to parent",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("rotation", "Rotation", PinType::Vec3),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Set local rotation
pub static SET_LOCAL_ROTATION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/set_local_rotation",
    display_name: "Set Local Rotation",
    category: "Hierarchy",
    description: "Set the local rotation relative to parent",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("rotation", "Rotation", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Get local scale
pub static GET_LOCAL_SCALE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/get_local_scale",
    display_name: "Get Local Scale",
    category: "Hierarchy",
    description: "Get the local scale",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("scale", "Scale", PinType::Vec3),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Set local scale
pub static SET_LOCAL_SCALE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/set_local_scale",
    display_name: "Set Local Scale",
    category: "Hierarchy",
    description: "Set the local scale",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("scale", "Scale", PinType::Vec3).with_default(PinValue::Vec3([1.0, 1.0, 1.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Transform point local to world
pub static LOCAL_TO_WORLD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/local_to_world",
    display_name: "Local to World",
    category: "Hierarchy",
    description: "Transform a point from local space to world space",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("local_point", "Local Point", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("world_point", "World Point", PinType::Vec3),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Transform point world to local
pub static WORLD_TO_LOCAL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "hierarchy/world_to_local",
    display_name: "World to Local",
    category: "Hierarchy",
    description: "Transform a point from world space to local space",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("world_point", "World Point", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("local_point", "Local Point", PinType::Vec3),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};
