//! Building a converted emitter graph, and laying it out.
//!
//! The converter thinks in values ("the velocity is this vector times that
//! random speed"), not in node ids, so it builds through [`GraphBuilder`]: every
//! value is a [`Src`], either an inline constant or a node's output pin, and
//! [`GraphBuilder::feed`] turns either into what the target pin needs (an inline
//! value or a wire). That keeps the mapping code free of the bookkeeping, and
//! it means a constant never costs a node: the graph only holds nodes that do
//! something, which is what the user will be reading.
//!
//! ## Layout
//!
//! A converted graph is opened in the ember graph view straight away, so where
//! its nodes sit is part of the conversion. Hand-placing every node per field
//! would break the first time two features stacked up, so positions are
//! computed once the graph is complete: each node goes in the column of its
//! longest path to the Particle Output (the output on the right, inputs flowing
//! left to right into it), and inside a column nodes are sorted by the height
//! of the pin they feed and stacked without overlapping. Cables then run mostly
//! level and never backwards.

use std::collections::{BTreeMap, HashMap};

use crate::asset::{Connection, NodeId, PinValue, VfxGraph, VfxNode};
use crate::nodes::{node_def, PropKind, OUTPUT_NODE};

/// Horizontal distance between columns, canvas px. Wide enough for a node
/// (about 200 px) plus a cable that reads as a curve rather than a kink.
pub const COLUMN_WIDTH: f32 = 300.0;
/// Rough per-row and header heights of an ember graph node, used only to keep
/// stacked nodes apart. An over-estimate costs a little empty canvas; an
/// under-estimate overlaps nodes, so these err high.
const HEADER: f32 = 36.0;
const ROW: f32 = 24.0;
/// The curve and gradient nodes draw a preview strip under their pins.
const PREVIEW: f32 = 56.0;
const GAP: f32 = 28.0;

/// A value for an input pin: a constant, or an output pin of a node.
#[derive(Clone, Debug, PartialEq)]
pub enum Src {
    Value(PinValue),
    Pin(NodeId, &'static str),
}

impl Src {
    pub fn float(v: f32) -> Self {
        Src::Value(PinValue::Float(v))
    }

    pub fn vec3(v: [f32; 3]) -> Self {
        Src::Value(PinValue::Vec3(v))
    }
}

/// An emitter graph under construction. The Particle Output is node 1.
pub struct GraphBuilder {
    nodes: Vec<VfxNode>,
    connections: Vec<Connection>,
    next: NodeId,
}

pub const OUTPUT_ID: NodeId = 1;

impl Default for GraphBuilder {
    fn default() -> Self {
        Self {
            nodes: vec![VfxNode::new(OUTPUT_ID, OUTPUT_NODE)],
            connections: Vec::new(),
            next: OUTPUT_ID + 1,
        }
    }
}

impl GraphBuilder {
    /// Add a node. The caller only ever names catalog types, which the tests
    /// hold it to by compiling every converted graph.
    pub fn node(&mut self, node_type: &str) -> NodeId {
        let id = self.next;
        self.next += 1;
        self.nodes.push(VfxNode::new(id, node_type));
        id
    }

    /// Set an inline value or property on a node.
    pub fn set(&mut self, node: NodeId, pin: &str, value: PinValue) {
        if let Some(n) = self.nodes.iter_mut().find(|n| n.id == node) {
            n.input_values.insert(pin.to_string(), value);
        }
    }

    pub fn wire(&mut self, from: NodeId, from_pin: &str, to: NodeId, to_pin: &str) {
        self.connections.retain(|c| !(c.to_node == to && c.to_pin == to_pin));
        self.connections.push(Connection::new(from, from_pin, to, to_pin));
    }

    /// Put `src` on `to.pin`: inline when it is a constant, a wire when it is
    /// a node's output.
    pub fn feed(&mut self, src: Src, to: NodeId, pin: &str) {
        match src {
            Src::Value(v) => self.set(to, pin, v),
            Src::Pin(from, from_pin) => self.wire(from, from_pin, to, pin),
        }
    }

    /// Feed one of the Particle Output's pins.
    pub fn output(&mut self, pin: &str, src: Src) {
        self.feed(src, OUTPUT_ID, pin);
    }

    /// A one-output node with its inputs fed, returning its `out` pin.
    pub fn op(&mut self, node_type: &str, inputs: &[(&str, Src)], out: &'static str) -> Src {
        let id = self.node(node_type);
        for (pin, src) in inputs {
            self.feed(src.clone(), id, pin);
        }
        Src::Pin(id, out)
    }

    /// `a * b`.
    pub fn mul(&mut self, a: Src, b: Src) -> Src {
        self.op("math/multiply", &[("a", a), ("b", b)], "value")
    }

    /// `a + b`.
    pub fn add(&mut self, a: Src, b: Src) -> Src {
        self.op("math/add", &[("a", a), ("b", b)], "value")
    }

    pub fn normalize(&mut self, v: Src) -> Src {
        self.op("math/normalize", &[("value", v)], "value")
    }

    /// A per-particle random float between `min` and `max`. A degenerate
    /// range is just the constant, so it costs no node.
    pub fn random_range(&mut self, min: f32, max: f32) -> Src {
        if (min - max).abs() <= f32::EPSILON {
            return Src::float(min);
        }
        self.op(
            "random/range",
            &[("min", Src::float(min)), ("max", Src::float(max))],
            "value",
        )
    }

    /// Sum a list of forces into one value, a chain of adds.
    pub fn sum(&mut self, mut terms: Vec<Src>) -> Option<Src> {
        if terms.is_empty() {
            return None;
        }
        let first = terms.remove(0);
        Some(terms.into_iter().fold(first, |acc, t| self.add(acc, t)))
    }

    /// The finished graph, pruned and laid out (see the module docs).
    ///
    /// Pruning drops every node with no path to the output. The mapping builds
    /// a shape's outward direction up front, because whether the velocity
    /// needs it is decided later; a directional velocity never wires it in, and
    /// a node that feeds nothing is clutter the user would only delete.
    ///
    /// A collision node is a root like the output (it acts without feeding
    /// anything), so it and whatever feeds it are kept.
    pub fn finish(mut self) -> VfxGraph {
        let roots: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|n| n.id == OUTPUT_ID || node_def(&n.node_type).is_some_and(|d| d.is_collider()))
            .map(|n| n.id)
            .collect();
        let mut live: std::collections::HashSet<NodeId> = roots.iter().copied().collect();
        let mut frontier = roots;
        while let Some(id) = frontier.pop() {
            for c in self.connections.iter().filter(|c| c.to_node == id) {
                if live.insert(c.from_node) {
                    frontier.push(c.from_node);
                }
            }
        }
        self.nodes.retain(|n| live.contains(&n.id));
        self.connections.retain(|c| live.contains(&c.from_node) && live.contains(&c.to_node));
        layout(&mut self.nodes, &self.connections);
        VfxGraph {
            nodes: self.nodes,
            connections: self.connections,
            comments: Vec::new(),
        }
    }
}

/// Estimated on-canvas height of a node, for stacking.
fn node_height(node: &VfxNode) -> f32 {
    let Some(def) = node_def(&node.node_type) else {
        return HEADER + ROW;
    };
    // A text field is a row like a checkbox; only the tables draw a preview.
    let toggles = def.props.iter().filter(|p| matches!(p.kind, PropKind::Bool(_) | PropKind::Text(_))).count();
    let tables = def.props.len() - toggles;
    let rows = def.inputs.len() + def.outputs.len() + toggles;
    HEADER + ROW * rows.max(1) as f32 + PREVIEW * tables as f32
}

/// The row of `pin` among a node's inputs, for lining a source up with it.
fn pin_row(node: &VfxNode, pin: &str) -> f32 {
    let Some(def) = node_def(&node.node_type) else { return 0.0 };
    // Outputs are drawn above the inputs.
    let index = def.inputs.iter().position(|p| p.name == pin).unwrap_or(0) + def.outputs.len();
    index as f32 * ROW
}

/// Place every node: see the module docs. Deterministic, so converting the
/// same file twice gives byte-identical text (which is how a second
/// conversion recognises the first and reuses it).
pub fn layout(nodes: &mut [VfxNode], connections: &[Connection]) {
    let Some(output) = nodes.iter().find(|n| n.node_type == OUTPUT_NODE).map(|n| n.id) else {
        return;
    };
    // Longest path to the output. A converted graph is a small DAG, so plain
    // relaxation (bounded by the node count) is enough.
    let mut depth: HashMap<NodeId, usize> = HashMap::new();
    depth.insert(output, 0);
    for _ in 0..=nodes.len() {
        let mut changed = false;
        for c in connections {
            if let Some(&d) = depth.get(&c.to_node) {
                let entry = depth.entry(c.from_node).or_insert(0);
                if *entry < d + 1 {
                    *entry = d + 1;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    // Anything not feeding the output (`finish` prunes those, but `layout` is
    // public) sits one column out rather than on top of it.
    for n in nodes.iter() {
        depth.entry(n.id).or_insert(1);
    }
    let max_depth = depth.values().copied().max().unwrap_or(0);

    let index: HashMap<NodeId, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();
    let mut y: HashMap<NodeId, f32> = HashMap::new();
    y.insert(output, 0.0);

    let mut columns: BTreeMap<usize, Vec<NodeId>> = BTreeMap::new();
    for n in nodes.iter() {
        columns.entry(depth[&n.id]).or_default().push(n.id);
    }
    for (&d, ids) in columns.iter() {
        if d == 0 {
            continue;
        }
        // Each node wants to sit level with the highest pin it feeds.
        let mut wanted: Vec<(f32, NodeId)> = ids
            .iter()
            .map(|&id| {
                let target = connections
                    .iter()
                    .filter(|c| c.from_node == id)
                    .filter_map(|c| {
                        let consumer = &nodes[*index.get(&c.to_node)?];
                        Some(y.get(&c.to_node)? + pin_row(consumer, &c.to_pin))
                    })
                    .fold(f32::INFINITY, f32::min);
                (if target.is_finite() { target } else { 0.0 }, id)
            })
            .collect();
        wanted.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut cursor = f32::NEG_INFINITY;
        for (want, id) in wanted {
            let top = want.max(cursor);
            y.insert(id, top);
            cursor = top + node_height(&nodes[index[&id]]) + GAP;
        }
    }
    for n in nodes.iter_mut() {
        let col = max_depth - depth[&n.id];
        n.position = [col as f32 * COLUMN_WIDTH, y.get(&n.id).copied().unwrap_or(0.0).round()];
    }
}
