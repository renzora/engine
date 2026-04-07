//! Lifecycle graph data model — a project-level node graph (Resource, not Component).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use renzora_core::{BlueprintConnection, BlueprintNode, NodeId, PinType};

/// Project-level lifecycle graph — controls boot sequence, scene flow, networking, timers.
///
/// Same data model as `BlueprintGraph` but stored as a **Resource** (not a Component),
/// because the lifecycle graph is project-wide and persists across scene loads.
#[derive(Resource, Clone, Default, Serialize, Deserialize)]
pub struct LifecycleGraph {
    pub nodes: Vec<BlueprintNode>,
    pub connections: Vec<BlueprintConnection>,
    pub next_id: u64,
}

impl LifecycleGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            connections: Vec::new(),
            next_id: 1,
        }
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
        let is_exec = self
            .get_node(to_node)
            .and_then(|_| crate::nodes::node_def(&self.get_node(to_node)?.node_type))
            .map(|def| {
                (def.pins)()
                    .iter()
                    .any(|p| p.name == to_pin && p.pin_type == PinType::Exec)
            })
            .unwrap_or(false);

        if !is_exec {
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

    pub fn connections_from(&self, from_node: NodeId, from_pin: &str) -> Vec<&BlueprintConnection> {
        self.connections
            .iter()
            .filter(|c| c.from_node == from_node && c.from_pin == from_pin)
            .collect()
    }

    pub fn connection_to(&self, to_node: NodeId, to_pin: &str) -> Option<&BlueprintConnection> {
        self.connections
            .iter()
            .find(|c| c.to_node == to_node && c.to_pin == to_pin)
    }

    /// Find all event nodes (entry points with no exec input).
    pub fn event_nodes(&self) -> Vec<&BlueprintNode> {
        self.nodes
            .iter()
            .filter(|n| n.node_type.starts_with("lifecycle/on_"))
            .collect()
    }

    /// Returns true if this graph has an `on_game_start` event node.
    pub fn has_game_start(&self) -> bool {
        self.nodes
            .iter()
            .any(|n| n.node_type == "lifecycle/on_game_start")
    }
}
