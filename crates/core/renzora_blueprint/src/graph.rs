//! Blueprint graph data model.
//!
//! Stored as a component on entities. Serializes into scene RON.
//! The editor syncs between this and `NodeGraphState` (from renzora_ui).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Identifiers ─────────────────────────────────────────────────────────────

pub type NodeId = u64;

// ── Pin types ───────────────────────────────────────────────────────────────

/// Pin data types for blueprint nodes.
/// Unlike material graphs which are pure data flow, blueprints have
/// execution flow pins (Exec) for controlling when things happen.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Reflect)]
pub enum PinType {
    /// Execution flow (white wires) — controls order of operations.
    Exec,
    /// Numeric types
    Float,
    Int,
    Bool,
    /// Text
    String,
    /// Vectors
    Vec2,
    Vec3,
    /// Color (RGBA)
    Color,
    /// Reference to another entity (by name or id)
    Entity,
    /// Wildcard — accepts any data type
    Any,
}

impl PinType {
    /// Can `from` connect to `to`?
    pub fn compatible(from: PinType, to: PinType) -> bool {
        if from == to {
            return true;
        }
        // Any accepts everything (except Exec)
        if to == PinType::Any && from != PinType::Exec {
            return true;
        }
        if from == PinType::Any && to != PinType::Exec {
            return true;
        }
        // Numeric widening
        matches!(
            (from, to),
            (PinType::Int, PinType::Float)
            | (PinType::Float, PinType::Vec2 | PinType::Vec3 | PinType::Color)
            | (PinType::Vec3, PinType::Color)
            | (PinType::Color, PinType::Vec3)
            | (PinType::Bool, PinType::Int | PinType::Float)
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Reflect)]
pub enum PinDir {
    Input,
    Output,
}

// ── Pin values ──────────────────────────────────────────────────────────────

/// Concrete values stored on pins (inline constants, defaults).
#[derive(Clone, Debug, Serialize, Deserialize, Reflect)]
pub enum PinValue {
    None,
    Float(f32),
    Int(i32),
    Bool(bool),
    String(String),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Color([f32; 4]),
    Entity(String),
}

impl Default for PinValue {
    fn default() -> Self {
        Self::None
    }
}

impl PinValue {
    pub fn as_float(&self) -> f32 {
        match self {
            Self::Float(v) => *v,
            Self::Int(v) => *v as f32,
            Self::Bool(v) => if *v { 1.0 } else { 0.0 },
            _ => 0.0,
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            Self::Int(v) => *v,
            Self::Float(v) => *v as i32,
            Self::Bool(v) => if *v { 1 } else { 0 },
            _ => 0,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(v) => *v,
            Self::Float(v) => *v != 0.0,
            Self::Int(v) => *v != 0,
            _ => false,
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            Self::String(v) => v.clone(),
            Self::Float(v) => format!("{v}"),
            Self::Int(v) => format!("{v}"),
            Self::Bool(v) => format!("{v}"),
            Self::Entity(v) => v.clone(),
            _ => String::new(),
        }
    }

    pub fn as_vec3(&self) -> [f32; 3] {
        match self {
            Self::Vec3(v) => *v,
            Self::Color([r, g, b, _]) => [*r, *g, *b],
            Self::Float(v) => [*v, *v, *v],
            _ => [0.0, 0.0, 0.0],
        }
    }

    pub fn as_vec2(&self) -> [f32; 2] {
        match self {
            Self::Vec2(v) => *v,
            Self::Float(v) => [*v, *v],
            _ => [0.0, 0.0],
        }
    }

    pub fn as_color(&self) -> [f32; 4] {
        match self {
            Self::Color(v) => *v,
            Self::Vec3([r, g, b]) => [*r, *g, *b, 1.0],
            Self::Float(v) => [*v, *v, *v, 1.0],
            _ => [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn pin_type(&self) -> PinType {
        match self {
            Self::None => PinType::Any,
            Self::Float(_) => PinType::Float,
            Self::Int(_) => PinType::Int,
            Self::Bool(_) => PinType::Bool,
            Self::String(_) => PinType::String,
            Self::Vec2(_) => PinType::Vec2,
            Self::Vec3(_) => PinType::Vec3,
            Self::Color(_) => PinType::Color,
            Self::Entity(_) => PinType::Entity,
        }
    }
}

// ── Pin template ────────────────────────────────────────────────────────────

/// Describes a pin on a node type (static definition).
#[derive(Clone, Debug)]
pub struct PinTemplate {
    pub name: String,
    pub label: String,
    pub pin_type: PinType,
    pub direction: PinDir,
    pub default_value: PinValue,
}

impl PinTemplate {
    pub fn exec_in(name: &str, label: &str) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type: PinType::Exec,
            direction: PinDir::Input,
            default_value: PinValue::None,
        }
    }

    pub fn exec_out(name: &str, label: &str) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type: PinType::Exec,
            direction: PinDir::Output,
            default_value: PinValue::None,
        }
    }

    pub fn input(name: &str, label: &str, pin_type: PinType) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type,
            direction: PinDir::Input,
            default_value: PinValue::None,
        }
    }

    pub fn output(name: &str, label: &str, pin_type: PinType) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type,
            direction: PinDir::Output,
            default_value: PinValue::None,
        }
    }

    pub fn with_default(mut self, value: PinValue) -> Self {
        self.default_value = value;
        self
    }
}

// ── Node type definition ────────────────────────────────────────────────────

/// Static definition of a blueprint node type (registered in the node library).
pub struct BlueprintNodeDef {
    pub node_type: &'static str,
    pub display_name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub pins: fn() -> Vec<PinTemplate>,
    /// RGB header color for the node in the graph editor.
    pub color: [u8; 3],
}

// ── Connection ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, Reflect)]
pub struct BlueprintConnection {
    pub from_node: NodeId,
    pub from_pin: String,
    pub to_node: NodeId,
    pub to_pin: String,
}

// ── Blueprint node ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, Reflect)]
pub struct BlueprintNode {
    pub id: NodeId,
    pub node_type: String,
    pub position: [f32; 2],
    /// Override values for input pins (user-set constants).
    pub input_values: HashMap<String, PinValue>,
}

impl BlueprintNode {
    pub fn new(id: NodeId, node_type: &str, position: [f32; 2]) -> Self {
        Self {
            id,
            node_type: node_type.to_string(),
            position,
            input_values: HashMap::new(),
        }
    }

    pub fn get_input_value(&self, pin_name: &str) -> Option<&PinValue> {
        self.input_values.get(pin_name)
    }
}

// ── Blueprint graph (the component) ─────────────────────────────────────────

/// Visual scripting graph stored on an entity.
/// Serializes into scene RON as a regular component.
#[derive(Component, Clone, Debug, Serialize, Deserialize, Reflect)]
#[reflect(Component, Default)]
pub struct BlueprintGraph {
    pub nodes: Vec<BlueprintNode>,
    pub connections: Vec<BlueprintConnection>,
    next_id: u64,
}

impl Default for BlueprintGraph {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            connections: Vec::new(),
            next_id: 1,
        }
    }
}

impl BlueprintGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node_type: &str, position: [f32; 2]) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(BlueprintNode::new(id, node_type, position));
        id
    }

    pub fn remove_node(&mut self, id: NodeId) {
        self.nodes.retain(|n| n.id != id);
        self.connections
            .retain(|c| c.from_node != id && c.to_node != id);
    }

    pub fn connect(
        &mut self,
        from_node: NodeId,
        from_pin: &str,
        to_node: NodeId,
        to_pin: &str,
    ) {
        // For data pins: inputs accept only one connection.
        // For exec pins: outputs can fan out to multiple targets.
        let is_exec = self.get_node(to_node)
            .and_then(|_| crate::node_def(&self.get_node(to_node)?.node_type))
            .map(|def| {
                (def.pins)().iter().any(|p| p.name == to_pin && p.pin_type == PinType::Exec)
            })
            .unwrap_or(false);

        if !is_exec {
            // Remove existing connection to this data input
            self.connections
                .retain(|c| !(c.to_node == to_node && c.to_pin == to_pin));
        }

        self.connections.push(BlueprintConnection {
            from_node,
            from_pin: from_pin.to_string(),
            to_node,
            to_pin: to_pin.to_string(),
        });
    }

    pub fn disconnect(&mut self, node_id: NodeId, pin_name: &str) {
        self.connections.retain(|c| {
            !((c.from_node == node_id && c.from_pin == pin_name)
                || (c.to_node == node_id && c.to_pin == pin_name))
        });
    }

    pub fn get_node(&self, id: NodeId) -> Option<&BlueprintNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut BlueprintNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Find all connections going out of a specific output pin.
    pub fn connections_from(&self, from_node: NodeId, from_pin: &str) -> Vec<&BlueprintConnection> {
        self.connections
            .iter()
            .filter(|c| c.from_node == from_node && c.from_pin == from_pin)
            .collect()
    }

    /// Find the single connection feeding into an input pin.
    pub fn connection_to(&self, to_node: NodeId, to_pin: &str) -> Option<&BlueprintConnection> {
        self.connections
            .iter()
            .find(|c| c.to_node == to_node && c.to_pin == to_pin)
    }

    /// Find all event nodes (nodes with no exec input — entry points).
    pub fn event_nodes(&self) -> Vec<&BlueprintNode> {
        self.nodes
            .iter()
            .filter(|n| n.node_type.starts_with("event/"))
            .collect()
    }
}
