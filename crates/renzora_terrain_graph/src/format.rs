//! The `.tgraph` file: a terrain graph as JSON.
//!
//! ```json
//! {
//!   "version": 1,
//!   "name": "Highlands",
//!   "nodes": [
//!     { "id": 1, "type": "mountain_range", "position": [0, 0], "params": { "scale": 800.0 } }
//!   ],
//!   "connections": [
//!     { "from_node": 1, "from_pin": "height", "to_node": 2, "to_pin": "height" }
//!   ],
//!   "params": [
//!     { "name": "peak_height", "label": "Peak height", "node": 2, "param": "peak", "min": 10, "max": 2000 }
//!   ]
//! }
//! ```
//!
//! A node stores only the parameters that differ from its catalog default
//! (see [`crate::catalog`]); a missing one reads as the default. That keeps a
//! hand-written graph short and lets a default improve without rewriting every
//! file that never touched it.
//!
//! **Exposed parameters** are how one recipe makes many terrains. Each names a
//! node parameter and gives it a graph-level name; the terrain's
//! [`crate::TerrainGenerator`] carries per-terrain overrides by that name. The
//! seed is not one of them: every graph has it, and it reaches every node (see
//! [`crate::eval`]).

use std::collections::BTreeMap;

use bevy::asset::{io::Reader, AssetLoader, LoadContext};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The format version this build writes. A file with a higher one still
/// loads; unknown node types evaluate to an error on that node rather than
/// failing the whole file.
pub const FORMAT_VERSION: u32 = 1;

/// One parameter value. Untagged so the JSON reads as plain values: a number,
/// a bool, a string (a choice or a path), or a list of `[x, y]` points.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    Bool(bool),
    Number(f32),
    Text(String),
    Points(Vec<[f32; 2]>),
}

impl ParamValue {
    pub fn as_number(&self) -> Option<f32> {
        match self {
            ParamValue::Number(v) => Some(*v),
            ParamValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ParamValue::Bool(b) => Some(*b),
            ParamValue::Number(v) => Some(*v != 0.0),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            ParamValue::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_points(&self) -> Option<&[[f32; 2]]> {
        match self {
            ParamValue::Points(p) => Some(p),
            _ => None,
        }
    }

    /// Feed the value into a hasher. `f32` has no `Hash`, so its bits stand in:
    /// two values that hash the same are the same value, which is all a cache
    /// key needs.
    pub fn hash_into(&self, h: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        match self {
            ParamValue::Bool(b) => (0u8, *b).hash(h),
            ParamValue::Number(v) => (1u8, v.to_bits()).hash(h),
            ParamValue::Text(s) => (2u8, s).hash(h),
            ParamValue::Points(p) => {
                3u8.hash(h);
                for [x, y] in p {
                    (x.to_bits(), y.to_bits()).hash(h);
                }
            }
        }
    }
}

/// One node in the graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: u64,
    /// A catalog id such as `noise` or `erosion` (see [`crate::catalog`]).
    #[serde(rename = "type")]
    pub node_type: String,
    /// Canvas position in the editor, pixels. Means nothing to the evaluator.
    #[serde(default)]
    pub position: [f32; 2],
    /// Parameters that differ from the catalog default, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, ParamValue>,
}

/// A wire from one node's output pin to another's input pin.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Connection {
    pub from_node: u64,
    pub from_pin: String,
    pub to_node: u64,
    pub to_pin: String,
}

/// A node parameter given a graph-level name, so a terrain using the graph
/// can override it without editing the file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExposedParam {
    /// The key a [`crate::TerrainGenerator`] override uses, e.g. `peak_height`.
    pub name: String,
    /// What the inspector shows, e.g. "Peak height".
    pub label: String,
    pub node: u64,
    pub param: String,
    #[serde(default)]
    pub min: f32,
    #[serde(default = "default_max")]
    pub max: f32,
}

fn default_max() -> f32 {
    1000.0
}

/// A comment box on the canvas. Kept in the file so a graph explains itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphComment {
    pub id: u64,
    pub text: String,
    /// `[x, y, w, h]`, canvas pixels.
    pub rect: [f32; 4],
}

/// A whole `.tgraph` file.
#[derive(Asset, TypePath, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainGraph {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub nodes: Vec<GraphNode>,
    #[serde(default)]
    pub connections: Vec<Connection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<ExposedParam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<GraphComment>,
}

fn default_version() -> u32 {
    FORMAT_VERSION
}

impl Default for TerrainGraph {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            name: String::new(),
            nodes: Vec::new(),
            connections: Vec::new(),
            params: Vec::new(),
            comments: Vec::new(),
        }
    }
}

impl TerrainGraph {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..default() }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("not a terrain graph: {e}"))
    }

    pub fn to_json(&self) -> String {
        // Pretty, because a graph is a file people diff and read in review.
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn node(&self, id: u64) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: u64) -> Option<&mut GraphNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// One more than the largest id in use. An id freed by deleting the newest
    /// node can come back; that is harmless, because a cached result is keyed
    /// on the node's type and parameters as well as its id.
    pub fn next_id(&self) -> u64 {
        self.nodes.iter().map(|n| n.id).chain(self.comments.iter().map(|c| c.id)).max().unwrap_or(0) + 1
    }

    /// Add a node of `node_type` at `position`, returning its id.
    pub fn add_node(&mut self, node_type: &str, position: [f32; 2]) -> u64 {
        let id = self.next_id();
        self.nodes.push(GraphNode { id, node_type: node_type.to_string(), position, params: BTreeMap::new() });
        id
    }

    /// Set a parameter on a node. Returns whether anything changed.
    pub fn set_param(&mut self, node: u64, name: &str, value: ParamValue) -> bool {
        let Some(n) = self.node_mut(node) else { return false };
        if n.params.get(name) == Some(&value) {
            return false;
        }
        n.params.insert(name.to_string(), value);
        true
    }

    /// Wire `from_node.from_pin` into `to_node.to_pin`. An input takes one
    /// wire, so whatever fed it before is replaced. Refuses a self loop and a
    /// wire that would close a cycle, which the evaluator could not order.
    pub fn connect(&mut self, from_node: u64, from_pin: &str, to_node: u64, to_pin: &str) -> Result<(), String> {
        if from_node == to_node {
            return Err("a node cannot feed itself".into());
        }
        if self.node(from_node).is_none() || self.node(to_node).is_none() {
            return Err("no such node".into());
        }
        if self.reaches(to_node, from_node) {
            return Err("that wire would make a loop".into());
        }
        self.connections.retain(|c| !(c.to_node == to_node && c.to_pin == to_pin));
        self.connections.push(Connection {
            from_node,
            from_pin: from_pin.to_string(),
            to_node,
            to_pin: to_pin.to_string(),
        });
        Ok(())
    }

    /// Whether following wires downstream from `from` arrives at `to`.
    fn reaches(&self, from: u64, to: u64) -> bool {
        let mut stack = vec![from];
        let mut seen = std::collections::HashSet::new();
        while let Some(n) = stack.pop() {
            if n == to {
                return true;
            }
            if !seen.insert(n) {
                continue;
            }
            stack.extend(self.connections.iter().filter(|c| c.from_node == n).map(|c| c.to_node));
        }
        false
    }

    pub fn disconnect(&mut self, to_node: u64, to_pin: &str) -> bool {
        let before = self.connections.len();
        self.connections.retain(|c| !(c.to_node == to_node && c.to_pin == to_pin));
        self.connections.len() != before
    }

    /// Remove a node, every wire touching it, and any exposed parameter that
    /// pointed into it.
    pub fn remove_node(&mut self, id: u64) -> bool {
        let before = self.nodes.len();
        self.nodes.retain(|n| n.id != id);
        self.connections.retain(|c| c.from_node != id && c.to_node != id);
        self.params.retain(|p| p.node != id);
        self.nodes.len() != before
    }

    /// The wire feeding `node.pin`, if any.
    pub fn input_of(&self, node: u64, pin: &str) -> Option<&Connection> {
        self.connections.iter().find(|c| c.to_node == node && c.to_pin == pin)
    }

    /// Expose `node.param` as `name`, replacing an earlier exposure of the
    /// same name.
    pub fn expose(&mut self, name: &str, label: &str, node: u64, param: &str, min: f32, max: f32) {
        self.params.retain(|p| p.name != name);
        self.params.push(ExposedParam {
            name: name.to_string(),
            label: label.to_string(),
            node,
            param: param.to_string(),
            min,
            max,
        });
    }

    /// The current value of an exposed parameter as the file has it, before
    /// any terrain's override: the node's own value, else its catalog default.
    pub fn exposed_value(&self, name: &str) -> Option<f32> {
        let p = self.params.iter().find(|p| p.name == name)?;
        let node = self.node(p.node)?;
        node.params
            .get(&p.param)
            .and_then(ParamValue::as_number)
            .or_else(|| crate::catalog::default_number(&node.node_type, &p.param))
    }
}

/// Loads a `.tgraph` as a [`TerrainGraph`] asset.
#[derive(Default, TypePath)]
pub struct TerrainGraphLoader;

impl AssetLoader for TerrainGraphLoader {
    type Asset = TerrainGraph;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let text = String::from_utf8_lossy(&bytes);
        TerrainGraph::parse(&text).map_err(std::io::Error::other)
    }

    fn extensions(&self) -> &[&str] {
        &["tgraph"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_read_as_plain_json() {
        let mut g = TerrainGraph::new("t");
        let a = g.add_node("noise", [0.0, 0.0]);
        g.set_param(a, "scale", ParamValue::Number(120.0));
        g.set_param(a, "mode", ParamValue::Text("ridge".into()));
        g.set_param(a, "invert", ParamValue::Bool(true));
        let json = g.to_json();
        assert!(json.contains("\"scale\": 120.0"), "{json}");
        assert!(json.contains("\"mode\": \"ridge\""), "{json}");
        let back = TerrainGraph::parse(&json).unwrap();
        assert_eq!(back, g);
    }

    #[test]
    fn an_input_takes_one_wire_and_loops_are_refused() {
        let mut g = TerrainGraph::new("t");
        let a = g.add_node("noise", [0.0, 0.0]);
        let b = g.add_node("noise", [0.0, 0.0]);
        let c = g.add_node("blur", [0.0, 0.0]);
        g.connect(a, "height", c, "height").unwrap();
        g.connect(b, "height", c, "height").unwrap();
        assert_eq!(g.connections.len(), 1);
        assert_eq!(g.input_of(c, "height").unwrap().from_node, b);
        let d = g.add_node("blur", [0.0, 0.0]);
        g.connect(c, "height", d, "height").unwrap();
        assert!(g.connect(d, "height", c, "height").is_err());
        assert!(g.connect(c, "height", c, "height").is_err());
    }

    #[test]
    fn removing_a_node_drops_its_wires_and_exposures() {
        let mut g = TerrainGraph::new("t");
        let a = g.add_node("noise", [0.0, 0.0]);
        let b = g.add_node("terrain_output", [0.0, 0.0]);
        g.connect(a, "height", b, "height").unwrap();
        g.expose("peak", "Peak", b, "peak", 0.0, 100.0);
        g.remove_node(b);
        assert!(g.connections.is_empty());
        assert!(g.params.is_empty());
        assert_eq!(g.next_id(), 2);
    }
}
