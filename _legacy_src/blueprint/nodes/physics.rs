//! Physics nodes (Rapier/Avian integration)
//!
//! Nodes for rigid bodies, colliders, forces, raycasting, and collision detection.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// RIGID BODY
// =============================================================================

/// Add rigid body to entity
pub static ADD_RIGID_BODY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/add_rigid_body",
    display_name: "Add Rigid Body",
    category: "Physics",
    description: "Add a rigid body component to an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("body_type", "Type", PinType::String).with_default(PinValue::String("dynamic".into())),
        Pin::input("mass", "Mass", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Set rigid body type
pub static SET_BODY_TYPE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/set_body_type",
    display_name: "Set Body Type",
    category: "Physics",
    description: "Set rigid body type (dynamic, static, kinematic)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("body_type", "Type", PinType::String).with_default(PinValue::String("dynamic".into())),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Set mass
pub static SET_MASS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/set_mass",
    display_name: "Set Mass",
    category: "Physics",
    description: "Set the mass of a rigid body",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("mass", "Mass", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Get velocity
pub static GET_VELOCITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/get_velocity",
    display_name: "Get Velocity",
    category: "Physics",
    description: "Get the linear velocity of a rigid body",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("velocity", "Velocity", PinType::Vec3),
        Pin::output("speed", "Speed", PinType::Float),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Set velocity
pub static SET_VELOCITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/set_velocity",
    display_name: "Set Velocity",
    category: "Physics",
    description: "Set the linear velocity of a rigid body",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("velocity", "Velocity", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Get angular velocity
pub static GET_ANGULAR_VELOCITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/get_angular_velocity",
    display_name: "Get Angular Velocity",
    category: "Physics",
    description: "Get the angular velocity of a rigid body",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("angular_velocity", "Angular Velocity", PinType::Vec3),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Set angular velocity
pub static SET_ANGULAR_VELOCITY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/set_angular_velocity",
    display_name: "Set Angular Velocity",
    category: "Physics",
    description: "Set the angular velocity of a rigid body",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("angular_velocity", "Angular Velocity", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// FORCES & IMPULSES
// =============================================================================

/// Apply force
pub static APPLY_FORCE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/apply_force",
    display_name: "Apply Force",
    category: "Physics",
    description: "Apply a continuous force to a rigid body",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("force", "Force", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Apply force at point
pub static APPLY_FORCE_AT_POINT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/apply_force_at_point",
    display_name: "Apply Force At Point",
    category: "Physics",
    description: "Apply a force at a specific point on a rigid body",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("force", "Force", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("point", "Point", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Apply impulse
pub static APPLY_IMPULSE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/apply_impulse",
    display_name: "Apply Impulse",
    category: "Physics",
    description: "Apply an instant impulse to a rigid body",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("impulse", "Impulse", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Apply torque
pub static APPLY_TORQUE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/apply_torque",
    display_name: "Apply Torque",
    category: "Physics",
    description: "Apply rotational force to a rigid body",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("torque", "Torque", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Apply torque impulse
pub static APPLY_TORQUE_IMPULSE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/apply_torque_impulse",
    display_name: "Apply Torque Impulse",
    category: "Physics",
    description: "Apply instant rotational impulse to a rigid body",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("torque", "Torque Impulse", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// COLLIDERS
// =============================================================================

/// Add box collider
pub static ADD_BOX_COLLIDER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/add_box_collider",
    display_name: "Add Box Collider",
    category: "Physics",
    description: "Add a box-shaped collider to an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("half_extents", "Half Extents", PinType::Vec3).with_default(PinValue::Vec3([0.5, 0.5, 0.5])),
        Pin::input("sensor", "Is Sensor", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Add sphere collider
pub static ADD_SPHERE_COLLIDER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/add_sphere_collider",
    display_name: "Add Sphere Collider",
    category: "Physics",
    description: "Add a sphere-shaped collider to an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("radius", "Radius", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("sensor", "Is Sensor", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Add capsule collider
pub static ADD_CAPSULE_COLLIDER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/add_capsule_collider",
    display_name: "Add Capsule Collider",
    category: "Physics",
    description: "Add a capsule-shaped collider to an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("radius", "Radius", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("height", "Height", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("sensor", "Is Sensor", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Add cylinder collider
pub static ADD_CYLINDER_COLLIDER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/add_cylinder_collider",
    display_name: "Add Cylinder Collider",
    category: "Physics",
    description: "Add a cylinder-shaped collider to an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("radius", "Radius", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("height", "Height", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("sensor", "Is Sensor", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Add mesh collider
pub static ADD_MESH_COLLIDER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/add_mesh_collider",
    display_name: "Add Mesh Collider",
    category: "Physics",
    description: "Add a mesh-based collider to an entity (from its mesh)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("convex", "Convex", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::input("sensor", "Is Sensor", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Set collider friction
pub static SET_FRICTION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/set_friction",
    display_name: "Set Friction",
    category: "Physics",
    description: "Set the friction coefficient of a collider",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("friction", "Friction", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Set collider restitution (bounciness)
pub static SET_RESTITUTION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/set_restitution",
    display_name: "Set Restitution",
    category: "Physics",
    description: "Set the bounciness of a collider",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("restitution", "Restitution", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// RAYCASTING
// =============================================================================

/// Raycast
pub static RAYCAST: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/raycast",
    display_name: "Raycast",
    category: "Physics",
    description: "Cast a ray and get the first hit",
    create_pins: || vec![
        Pin::input("origin", "Origin", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("direction", "Direction", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 1.0])),
        Pin::input("max_distance", "Max Distance", PinType::Float).with_default(PinValue::Float(100.0)),
        Pin::output("hit", "Hit", PinType::Bool),
        Pin::output("entity", "Entity", PinType::Entity),
        Pin::output("point", "Hit Point", PinType::Vec3),
        Pin::output("normal", "Hit Normal", PinType::Vec3),
        Pin::output("distance", "Distance", PinType::Float),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Raycast all
pub static RAYCAST_ALL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/raycast_all",
    display_name: "Raycast All",
    category: "Physics",
    description: "Cast a ray and get all hits",
    create_pins: || vec![
        Pin::input("origin", "Origin", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("direction", "Direction", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 1.0])),
        Pin::input("max_distance", "Max Distance", PinType::Float).with_default(PinValue::Float(100.0)),
        Pin::output("hits", "Hits", PinType::EntityArray),
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Spherecast
pub static SPHERECAST: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/spherecast",
    display_name: "Spherecast",
    category: "Physics",
    description: "Cast a sphere and get the first hit",
    create_pins: || vec![
        Pin::input("origin", "Origin", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("direction", "Direction", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 1.0])),
        Pin::input("radius", "Radius", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("max_distance", "Max Distance", PinType::Float).with_default(PinValue::Float(100.0)),
        Pin::output("hit", "Hit", PinType::Bool),
        Pin::output("entity", "Entity", PinType::Entity),
        Pin::output("point", "Hit Point", PinType::Vec3),
        Pin::output("normal", "Hit Normal", PinType::Vec3),
        Pin::output("distance", "Distance", PinType::Float),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// COLLISION EVENTS
// =============================================================================

/// On collision enter
pub static ON_COLLISION_ENTER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/on_collision_enter",
    display_name: "On Collision Enter",
    category: "Physics Events",
    description: "Triggered when collision begins",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("other", "Other Entity", PinType::Entity),
        Pin::output("contact_point", "Contact Point", PinType::Vec3),
        Pin::output("contact_normal", "Contact Normal", PinType::Vec3),
        Pin::output("impulse", "Impulse", PinType::Float),
    ],
    color: [220, 100, 100],
    is_event: true,
    is_comment: false,
};

/// On collision exit
pub static ON_COLLISION_EXIT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/on_collision_exit",
    display_name: "On Collision Exit",
    category: "Physics Events",
    description: "Triggered when collision ends",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("other", "Other Entity", PinType::Entity),
    ],
    color: [220, 100, 100],
    is_event: true,
    is_comment: false,
};

/// On collision stay
pub static ON_COLLISION_STAY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/on_collision_stay",
    display_name: "On Collision Stay",
    category: "Physics Events",
    description: "Triggered every frame while colliding",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("other", "Other Entity", PinType::Entity),
    ],
    color: [220, 100, 100],
    is_event: true,
    is_comment: false,
};

/// On trigger enter
pub static ON_TRIGGER_ENTER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/on_trigger_enter",
    display_name: "On Trigger Enter",
    category: "Physics Events",
    description: "Triggered when entering a sensor/trigger collider",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("other", "Other Entity", PinType::Entity),
    ],
    color: [220, 100, 100],
    is_event: true,
    is_comment: false,
};

/// On trigger exit
pub static ON_TRIGGER_EXIT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/on_trigger_exit",
    display_name: "On Trigger Exit",
    category: "Physics Events",
    description: "Triggered when exiting a sensor/trigger collider",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("other", "Other Entity", PinType::Entity),
    ],
    color: [220, 100, 100],
    is_event: true,
    is_comment: false,
};

// =============================================================================
// PHYSICS SETTINGS
// =============================================================================

/// Set gravity scale
pub static SET_GRAVITY_SCALE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/set_gravity_scale",
    display_name: "Set Gravity Scale",
    category: "Physics",
    description: "Set the gravity scale for a rigid body",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Set linear damping
pub static SET_LINEAR_DAMPING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/set_linear_damping",
    display_name: "Set Linear Damping",
    category: "Physics",
    description: "Set the linear damping of a rigid body",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("damping", "Damping", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Set angular damping
pub static SET_ANGULAR_DAMPING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/set_angular_damping",
    display_name: "Set Angular Damping",
    category: "Physics",
    description: "Set the angular damping of a rigid body",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("damping", "Damping", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Lock rotation axes
pub static LOCK_ROTATION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/lock_rotation",
    display_name: "Lock Rotation",
    category: "Physics",
    description: "Lock rotation on specific axes",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("lock_x", "Lock X", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::input("lock_y", "Lock Y", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::input("lock_z", "Lock Z", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Lock position axes
pub static LOCK_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/lock_position",
    display_name: "Lock Position",
    category: "Physics",
    description: "Lock position on specific axes",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("lock_x", "Lock X", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::input("lock_y", "Lock Y", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::input("lock_z", "Lock Z", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// CHARACTER CONTROLLER
// =============================================================================

/// Add character controller
pub static ADD_CHARACTER_CONTROLLER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/add_character_controller",
    display_name: "Add Character Controller",
    category: "Physics",
    description: "Add a character controller component",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("height", "Height", PinType::Float).with_default(PinValue::Float(2.0)),
        Pin::input("radius", "Radius", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Move character
pub static MOVE_CHARACTER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/move_character",
    display_name: "Move Character",
    category: "Physics",
    description: "Move a character controller",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("direction", "Direction", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("grounded", "Is Grounded", PinType::Bool),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};

/// Is grounded
pub static IS_GROUNDED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "physics/is_grounded",
    display_name: "Is Grounded",
    category: "Physics",
    description: "Check if a character controller is grounded",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("grounded", "Is Grounded", PinType::Bool),
    ],
    color: [220, 140, 80],
    is_event: false,
    is_comment: false,
};
