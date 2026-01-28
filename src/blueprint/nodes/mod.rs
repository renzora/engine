//! Node type definitions and registry
//!
//! Each node type defines its pins, default values, and code generation behavior.

mod events;
mod input;
mod logic;
mod math;
mod transform;
mod utility;

use std::collections::HashMap;
use bevy::prelude::*;
use super::{BlueprintNode, NodeId, Pin, PinType, PinValue, PinDirection};

/// Definition of a node type
#[allow(dead_code)]
pub struct NodeTypeDefinition {
    /// Unique type ID (e.g., "math/add")
    pub type_id: &'static str,
    /// Display name in the node palette
    pub display_name: &'static str,
    /// Category for organization (e.g., "Math", "Events")
    pub category: &'static str,
    /// Description shown in tooltips
    pub description: &'static str,
    /// Function to create the node's pins
    pub create_pins: fn() -> Vec<Pin>,
    /// Accent color for the node header [r, g, b]
    pub color: [u8; 3],
    /// Whether this is an event node (entry point)
    pub is_event: bool,
    /// Whether this node can have a comment
    pub is_comment: bool,
}

impl NodeTypeDefinition {
    /// Create a new node instance with this type
    pub fn create_node(&self, id: NodeId) -> BlueprintNode {
        let mut node = BlueprintNode::new(id, self.type_id, (self.create_pins)());

        // Set default values for all input pins that have them
        for pin in &node.pins {
            if pin.direction == PinDirection::Input {
                if let Some(default) = &pin.default_value {
                    node.input_values.insert(pin.name.clone(), default.clone());
                }
            }
        }

        node
    }
}

/// Registry of all available node types
#[derive(Resource)]
pub struct NodeRegistry {
    /// Node types indexed by type_id
    pub types: HashMap<String, &'static NodeTypeDefinition>,
    /// Node types organized by category
    pub by_category: HashMap<String, Vec<&'static NodeTypeDefinition>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
            by_category: HashMap::new(),
        }
    }

    /// Register a node type
    pub fn register(&mut self, def: &'static NodeTypeDefinition) {
        self.types.insert(def.type_id.to_string(), def);
        self.by_category
            .entry(def.category.to_string())
            .or_default()
            .push(def);
    }

    /// Get a node type by ID
    pub fn get(&self, type_id: &str) -> Option<&'static NodeTypeDefinition> {
        self.types.get(type_id).copied()
    }

    /// Get all categories
    pub fn categories(&self) -> impl Iterator<Item = &String> {
        self.by_category.keys()
    }

    /// Get all node types in a category
    pub fn nodes_in_category(&self, category: &str) -> Option<&Vec<&'static NodeTypeDefinition>> {
        self.by_category.get(category)
    }

    /// Create a node instance from a type ID
    pub fn create_node(&self, type_id: &str, id: NodeId) -> Option<BlueprintNode> {
        self.get(type_id).map(|def| def.create_node(id))
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Register all built-in node types
pub fn register_all_nodes(registry: &mut NodeRegistry) {
    // Events
    registry.register(&events::ON_READY);
    registry.register(&events::ON_UPDATE);

    // Math
    registry.register(&math::ADD);
    registry.register(&math::SUBTRACT);
    registry.register(&math::MULTIPLY);
    registry.register(&math::DIVIDE);
    registry.register(&math::LERP);
    registry.register(&math::CLAMP);
    registry.register(&math::ABS);
    registry.register(&math::MIN);
    registry.register(&math::MAX);
    registry.register(&math::SIN);
    registry.register(&math::COS);

    // Logic
    registry.register(&logic::IF_BRANCH);
    registry.register(&logic::COMPARE);
    registry.register(&logic::AND);
    registry.register(&logic::OR);
    registry.register(&logic::NOT);

    // Transform
    registry.register(&transform::GET_POSITION);
    registry.register(&transform::SET_POSITION);
    registry.register(&transform::TRANSLATE);
    registry.register(&transform::GET_ROTATION);
    registry.register(&transform::SET_ROTATION);
    registry.register(&transform::ROTATE);

    // Input
    registry.register(&input::GET_INPUT_AXIS);
    registry.register(&input::IS_KEY_PRESSED);
    registry.register(&input::GET_MOUSE_POSITION);
    registry.register(&input::GET_MOUSE_DELTA);

    // Utility
    registry.register(&utility::PRINT);
    registry.register(&utility::SEQUENCE);
    registry.register(&utility::COMMENT);
    registry.register(&utility::GET_DELTA);
    registry.register(&utility::GET_ELAPSED);

    // Variables
    registry.register(&utility::GET_VARIABLE);
    registry.register(&utility::SET_VARIABLE);
}
