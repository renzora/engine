//! Health system nodes
//!
//! Nodes for health, damage, and healing operations.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// HEALTH DATA NODES (Pure - no exec pins)
// =============================================================================

/// Get entity health values
pub static GET_HEALTH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/get_health",
    display_name: "Get Health",
    category: "Health",
    description: "Get the current health, max health, and health percentage of an entity",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("health", "Health", PinType::Float),
        Pin::output("max_health", "Max Health", PinType::Float),
        Pin::output("percent", "Percent", PinType::Float),
    ],
    color: [220, 80, 80],
    is_event: false,
    is_comment: false,
};

/// Check if entity is dead
pub static IS_DEAD: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/is_dead",
    display_name: "Is Dead",
    category: "Health",
    description: "Check if an entity's health is zero or below",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("dead", "Dead", PinType::Bool),
    ],
    color: [220, 80, 80],
    is_event: false,
    is_comment: false,
};

/// Check if entity is invincible
pub static IS_INVINCIBLE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/is_invincible",
    display_name: "Is Invincible",
    category: "Health",
    description: "Check if an entity is currently invincible",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("invincible", "Invincible", PinType::Bool),
    ],
    color: [220, 80, 80],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// HEALTH ACTION NODES (Flow - have exec pins)
// =============================================================================

/// Apply damage to entity
pub static DAMAGE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/damage",
    display_name: "Damage",
    category: "Health",
    description: "Apply damage to an entity's health (respects invincibility)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("amount", "Amount", PinType::Float).with_default(PinValue::Float(10.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("new_health", "New Health", PinType::Float),
        Pin::output("killed", "Killed", PinType::Bool),
    ],
    color: [220, 80, 80],
    is_event: false,
    is_comment: false,
};

/// Heal entity
pub static HEAL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/heal",
    display_name: "Heal",
    category: "Health",
    description: "Restore health to an entity (clamped to max health)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("amount", "Amount", PinType::Float).with_default(PinValue::Float(10.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("new_health", "New Health", PinType::Float),
    ],
    color: [80, 220, 80],
    is_event: false,
    is_comment: false,
};

/// Set entity health directly
pub static SET_HEALTH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/set_health",
    display_name: "Set Health",
    category: "Health",
    description: "Set an entity's health to a specific value",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("health", "Health", PinType::Float).with_default(PinValue::Float(100.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 80, 80],
    is_event: false,
    is_comment: false,
};

/// Set entity max health
pub static SET_MAX_HEALTH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/set_max_health",
    display_name: "Set Max Health",
    category: "Health",
    description: "Set an entity's maximum health",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("max_health", "Max Health", PinType::Float).with_default(PinValue::Float(100.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 80, 80],
    is_event: false,
    is_comment: false,
};

/// Set invincibility state
pub static SET_INVINCIBLE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/set_invincible",
    display_name: "Set Invincible",
    category: "Health",
    description: "Set whether an entity is invincible (ignores damage)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("invincible", "Invincible", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [220, 180, 80],
    is_event: false,
    is_comment: false,
};

/// Kill entity (set health to 0)
pub static KILL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/kill",
    display_name: "Kill",
    category: "Health",
    description: "Instantly kill an entity by setting health to 0",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 40, 40],
    is_event: false,
    is_comment: false,
};

/// Revive entity (restore to max health)
pub static REVIVE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/revive",
    display_name: "Revive",
    category: "Health",
    description: "Revive an entity by restoring to max health",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [80, 220, 80],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// HEALTH EVENT NODES
// =============================================================================

/// Event when entity takes damage
pub static ON_DAMAGE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/on_damage",
    display_name: "On Damage",
    category: "Health",
    description: "Triggered when this entity takes damage",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("amount", "Damage Amount", PinType::Float),
        Pin::output("attacker", "Attacker", PinType::Entity),
        Pin::output("new_health", "New Health", PinType::Float),
    ],
    color: [220, 80, 80],
    is_event: true,
    is_comment: false,
};

/// Event when entity dies
pub static ON_DEATH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/on_death",
    display_name: "On Death",
    category: "Health",
    description: "Triggered when this entity's health reaches zero",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("killer", "Killer", PinType::Entity),
    ],
    color: [180, 40, 40],
    is_event: true,
    is_comment: false,
};

/// Event when entity is healed
pub static ON_HEAL: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/on_heal",
    display_name: "On Heal",
    category: "Health",
    description: "Triggered when this entity is healed",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("amount", "Heal Amount", PinType::Float),
        Pin::output("new_health", "New Health", PinType::Float),
    ],
    color: [80, 220, 80],
    is_event: true,
    is_comment: false,
};
