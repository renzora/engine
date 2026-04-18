//! Blueprint graph data model.
//!
//! Stored as a component on entities. Serializes into scene RON.
//! The editor syncs between this and `NodeGraphState` (from renzora_ui).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// ── Re-export shared graph types from renzora ─────────────────────────
pub use renzora::{
    NodeId, PinType, PinDir, PinValue, PinTemplate, BlueprintNodeDef,
    BlueprintConnection, BlueprintNode,
};

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
