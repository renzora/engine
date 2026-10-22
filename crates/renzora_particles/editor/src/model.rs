//! Edit operations on a `.vfx` document, with no Bevy in sight.
//!
//! Everything the panels do to an effect (add a node, wire two pins, delete an
//! emitter) goes through one of these functions, so the rules that keep a
//! document valid live in one place and are tested without an editor running:
//! the Particle Output is never deleted and never duplicated, an input takes
//! one wire, a node id is never reused, and an effect always has an emitter.
//!
//! What a document *computes* is still the compiler's business. A type
//! mismatch on a wire is allowed here and reported by `compile_graph` against
//! the node, which is how the panel highlights it; refusing the wire outright
//! would hide the reason from the user.

use renzora::GraphComment;
use renzora_particles::asset::{Connection, Emitter, NodeId, PinValue, VfxAsset, VfxGraph, VfxNode};
use renzora_particles::curve::{CurvePoint, Ease, GradientStop};
use renzora_particles::nodes::{node_def, PropKind, OUTPUT_NODE};

/// Where a fresh graph puts its output node. Off to the right of the origin,
/// so the nodes a user adds at the view centre land to its left, where inputs
/// feed in from.
pub const OUTPUT_POSITION: [f32; 2] = [520.0, 0.0];

/// Why [`connect`] refused a wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectError {
    UnknownNode,
    UnknownPin,
    /// A node wired to itself is a cycle the compiler would reject anyway, and
    /// the widget's drag makes it easy to do by accident.
    SelfLoop,
}

/// The next unused id across nodes *and* comments, which share the widget's
/// id space (a comment and a node with one id would confuse its selection).
pub fn next_id(graph: &VfxGraph) -> NodeId {
    let nodes = graph.nodes.iter().map(|n| n.id);
    let comments = graph.comments.iter().map(|c| c.id);
    nodes.chain(comments).max().map_or(1, |m| m + 1)
}

pub fn node(graph: &VfxGraph, id: NodeId) -> Option<&VfxNode> {
    graph.nodes.iter().find(|n| n.id == id)
}

pub fn node_mut(graph: &mut VfxGraph, id: NodeId) -> Option<&mut VfxNode> {
    graph.nodes.iter_mut().find(|n| n.id == id)
}

pub fn is_output(graph: &VfxGraph, id: NodeId) -> bool {
    node(graph, id).is_some_and(|n| n.node_type == OUTPUT_NODE)
}

/// Add a `node_type` node at `position`. `None` for a type the catalog does
/// not know, or for a second output: an emitter has exactly one, and the
/// compiler would only report the duplicate after the fact.
///
/// Curve and gradient properties are seeded with a visible default rather than
/// left empty: an empty table samples as 1 / white, which reads as "the node
/// does nothing" until the user finds the editor.
pub fn add_node(graph: &mut VfxGraph, node_type: &str, position: [f32; 2]) -> Option<NodeId> {
    let def = node_def(node_type)?;
    if node_type == OUTPUT_NODE && graph.nodes.iter().any(|n| n.node_type == OUTPUT_NODE) {
        return None;
    }
    let id = next_id(graph);
    let mut n = VfxNode::new(id, node_type);
    n.position = position;
    for prop in def.props {
        match prop.kind {
            PropKind::Curve => {
                n.input_values.insert(prop.name.to_string(), PinValue::Curve(default_curve()));
            }
            PropKind::Gradient => {
                n.input_values
                    .insert(prop.name.to_string(), PinValue::Gradient(default_gradient()));
            }
            PropKind::Bool(_) => {}
            // A fresh name per node, so a second Parameter is a second knob a
            // script can turn rather than an alias of the first.
            PropKind::Text(default) => {
                let name = unique_text(graph, prop.name, default);
                n.input_values.insert(prop.name.to_string(), PinValue::Text(name));
            }
        }
    }
    graph.nodes.push(n);
    Some(id)
}

/// `base`, or `base2`, `base3`... whichever no node's `prop` already holds.
fn unique_text(graph: &VfxGraph, prop: &str, base: &str) -> String {
    let taken = |s: &str| {
        graph
            .nodes
            .iter()
            .any(|n| matches!(n.input_values.get(prop), Some(PinValue::Text(t)) if t.trim() == s))
    };
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|k| format!("{base}{k}")).find(|s| !taken(s)).unwrap_or_else(|| base.to_string())
}

/// A 1 to 0 fade: what a curve on a particle's life is for nine times in ten.
pub fn default_curve() -> Vec<CurvePoint> {
    vec![
        CurvePoint { t: 0.0, value: 1.0, ease: Ease::Linear },
        CurvePoint { t: 1.0, value: 0.0, ease: Ease::Linear },
    ]
}

/// White to transparent white, for the same reason.
pub fn default_gradient() -> Vec<GradientStop> {
    vec![
        GradientStop { t: 0.0, color: [1.0, 1.0, 1.0, 1.0] },
        GradientStop { t: 1.0, color: [1.0, 1.0, 1.0, 0.0] },
    ]
}

/// Remove a node and every wire touching it. `false` (nothing changed) for the
/// output node, which an emitter cannot be without, and for an unknown id.
pub fn remove_node(graph: &mut VfxGraph, id: NodeId) -> bool {
    if is_output(graph, id) || node(graph, id).is_none() {
        return false;
    }
    graph.nodes.retain(|n| n.id != id);
    graph.connections.retain(|c| c.from_node != id && c.to_node != id);
    true
}

/// Wire `from.from_pin` (an output) into `to.to_pin` (an input), replacing
/// whatever fed that input before: an input reads one value.
pub fn connect(
    graph: &mut VfxGraph,
    from: NodeId,
    from_pin: &str,
    to: NodeId,
    to_pin: &str,
) -> Result<(), ConnectError> {
    if from == to {
        return Err(ConnectError::SelfLoop);
    }
    let (Some(a), Some(b)) = (node(graph, from), node(graph, to)) else {
        return Err(ConnectError::UnknownNode);
    };
    let (Some(da), Some(db)) = (node_def(&a.node_type), node_def(&b.node_type)) else {
        return Err(ConnectError::UnknownNode);
    };
    if da.output(from_pin).is_none() || db.input(to_pin).is_none() {
        return Err(ConnectError::UnknownPin);
    }
    graph.connections.retain(|c| !(c.to_node == to && c.to_pin == to_pin));
    graph.connections.push(Connection::new(from, from_pin, to, to_pin));
    Ok(())
}

/// Remove the wire feeding `to.to_pin`. `true` if there was one.
pub fn disconnect(graph: &mut VfxGraph, to: NodeId, to_pin: &str) -> bool {
    let before = graph.connections.len();
    graph.connections.retain(|c| !(c.to_node == to && c.to_pin == to_pin));
    graph.connections.len() != before
}

pub fn is_connected(graph: &VfxGraph, to: NodeId, to_pin: &str) -> bool {
    graph.connections.iter().any(|c| c.to_node == to && c.to_pin == to_pin)
}

/// Write an inline value (a pin's or a property's).
pub fn set_value(graph: &mut VfxGraph, id: NodeId, pin: &str, value: PinValue) -> bool {
    match node_mut(graph, id) {
        Some(n) => {
            if n.input_values.get(pin) == Some(&value) {
                return false;
            }
            n.input_values.insert(pin.to_string(), value);
            true
        }
        None => false,
    }
}

pub fn add_comment(graph: &mut VfxGraph, rect: [f32; 4]) -> NodeId {
    let id = next_id(graph);
    graph.comments.push(GraphComment { id, rect, ..Default::default() });
    id
}

pub fn comment_mut(graph: &mut VfxGraph, id: NodeId) -> Option<&mut GraphComment> {
    graph.comments.iter_mut().find(|c| c.id == id)
}

pub fn remove_comment(graph: &mut VfxGraph, id: NodeId) {
    graph.comments.retain(|c| c.id != id);
}

/// A graph holding just the output node: the starting point of a new emitter.
pub fn output_only_graph() -> VfxGraph {
    let mut graph = VfxGraph::default();
    ensure_output(&mut graph);
    graph
}

/// Give a graph its output node if it has none. A hand-written file can leave
/// it out, and the editor would otherwise have nothing to wire into.
pub fn ensure_output(graph: &mut VfxGraph) -> bool {
    if graph.nodes.iter().any(|n| n.node_type == OUTPUT_NODE) {
        return false;
    }
    let id = next_id(graph);
    let mut n = VfxNode::new(id, OUTPUT_NODE);
    n.position = OUTPUT_POSITION;
    graph.nodes.push(n);
    true
}

/// `base`, or `base 2`, `base 3`... whichever no emitter already uses. Names
/// are what the tabs show, so two emitters called the same are two tabs you
/// cannot tell apart.
pub fn unique_emitter_name(asset: &VfxAsset, base: &str) -> String {
    let taken = |n: &str| asset.emitters.iter().any(|e| e.name == n);
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|i| format!("{base} {i}")).find(|n| !taken(n)).unwrap_or_else(|| base.to_string())
}

/// Append a new emitter with an output-only graph and a seed of its own, and
/// return its index.
pub fn add_emitter(asset: &mut VfxAsset) -> usize {
    let name = unique_emitter_name(asset, "Emitter");
    let seed = asset.emitters.iter().map(|e| e.seed).max().unwrap_or(0) + 1;
    asset.emitters.push(Emitter {
        name,
        seed,
        graph: output_only_graph(),
        ..Default::default()
    });
    asset.emitters.len() - 1
}

/// Copy an emitter (graph, settings and all) into the slot after it, under a
/// fresh name and seed so the copy does not draw the very same particles.
pub fn duplicate_emitter(asset: &mut VfxAsset, index: usize) -> Option<usize> {
    let mut copy = asset.emitters.get(index)?.clone();
    copy.name = unique_emitter_name(asset, &copy.name);
    copy.seed = asset.emitters.iter().map(|e| e.seed).max().unwrap_or(0) + 1;
    asset.emitters.insert(index + 1, copy);
    // Tracks name their emitter by index; the ones after the copy move along.
    renzora_particles::params::on_emitter_inserted(asset, index + 1);
    Some(index + 1)
}

/// Remove an emitter. Refuses the last one: an effect with no emitters has no
/// tab to show and nothing to edit back into existence from.
pub fn remove_emitter(asset: &mut VfxAsset, index: usize) -> bool {
    if asset.emitters.len() <= 1 || index >= asset.emitters.len() {
        return false;
    }
    asset.emitters.remove(index);
    renzora_particles::params::on_emitter_removed(asset, index);
    true
}

/// Rename an emitter. An empty or taken name is refused.
pub fn rename_emitter(asset: &mut VfxAsset, index: usize, name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() || asset.emitters.iter().enumerate().any(|(i, e)| i != index && e.name == name) {
        return false;
    }
    match asset.emitters.get_mut(index) {
        Some(e) if e.name != name => {
            e.name = name.to_string();
            true
        }
        _ => false,
    }
}

/// Make a loaded document editable: at least one emitter, and an output node
/// in every graph. Returns `true` when it had to change anything.
pub fn normalize(asset: &mut VfxAsset) -> bool {
    let mut changed = false;
    if asset.emitters.is_empty() {
        add_emitter(asset);
        changed = true;
    }
    for e in &mut asset.emitters {
        changed |= ensure_output(&mut e.graph);
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_particles::asset::{parse_vfx, write_vfx};
    use renzora_particles::compile::compile_graph;
    use renzora_particles::presets::PRESETS;

    fn graph_with_output() -> (VfxGraph, NodeId) {
        let g = output_only_graph();
        let id = g.nodes[0].id;
        (g, id)
    }

    #[test]
    fn adding_a_node_allocates_a_fresh_id() {
        let (mut g, out) = graph_with_output();
        let a = add_node(&mut g, "math/float", [0.0, 0.0]).unwrap();
        let b = add_node(&mut g, "math/add", [0.0, 0.0]).unwrap();
        assert_ne!(a, out);
        assert_ne!(a, b);
        assert_eq!(g.nodes.len(), 3);
    }

    #[test]
    fn ids_are_never_reused_after_a_delete() {
        let (mut g, _) = graph_with_output();
        let a = add_node(&mut g, "math/float", [0.0, 0.0]).unwrap();
        let b = add_node(&mut g, "math/float", [0.0, 0.0]).unwrap();
        assert!(remove_node(&mut g, a));
        // `b` is still the highest, so the next id is past it, not `a` again.
        let c = add_node(&mut g, "math/float", [0.0, 0.0]).unwrap();
        assert!(c > b);
    }

    #[test]
    fn comments_share_the_id_space() {
        let (mut g, _) = graph_with_output();
        let c = add_comment(&mut g, [0.0, 0.0, 10.0, 10.0]);
        let n = add_node(&mut g, "math/float", [0.0, 0.0]).unwrap();
        assert_ne!(c, n);
    }

    #[test]
    fn unknown_node_types_are_refused() {
        let (mut g, _) = graph_with_output();
        assert_eq!(add_node(&mut g, "math/teleport", [0.0, 0.0]), None);
    }

    #[test]
    fn the_output_cannot_be_deleted_or_duplicated() {
        let (mut g, out) = graph_with_output();
        assert!(!remove_node(&mut g, out));
        assert!(is_output(&g, out));
        assert_eq!(add_node(&mut g, OUTPUT_NODE, [0.0, 0.0]), None);
        assert_eq!(g.nodes.iter().filter(|n| n.node_type == OUTPUT_NODE).count(), 1);
    }

    #[test]
    fn removing_a_node_takes_its_wires_with_it() {
        let (mut g, out) = graph_with_output();
        let f = add_node(&mut g, "math/float", [0.0, 0.0]).unwrap();
        connect(&mut g, f, "value", out, "lifetime").unwrap();
        assert_eq!(g.connections.len(), 1);
        assert!(remove_node(&mut g, f));
        assert!(g.connections.is_empty());
    }

    #[test]
    fn an_input_takes_one_wire() {
        let (mut g, out) = graph_with_output();
        let a = add_node(&mut g, "math/float", [0.0, 0.0]).unwrap();
        let b = add_node(&mut g, "math/float", [0.0, 0.0]).unwrap();
        connect(&mut g, a, "value", out, "lifetime").unwrap();
        connect(&mut g, b, "value", out, "lifetime").unwrap();
        assert_eq!(g.connections.len(), 1);
        assert_eq!(g.connections[0].from_node, b);
    }

    #[test]
    fn bad_wires_are_refused() {
        let (mut g, out) = graph_with_output();
        let a = add_node(&mut g, "math/add", [0.0, 0.0]).unwrap();
        assert_eq!(connect(&mut g, a, "value", a, "a"), Err(ConnectError::SelfLoop));
        assert_eq!(connect(&mut g, a, "nope", out, "lifetime"), Err(ConnectError::UnknownPin));
        assert_eq!(connect(&mut g, a, "value", out, "nope"), Err(ConnectError::UnknownPin));
        assert_eq!(connect(&mut g, 999, "value", out, "lifetime"), Err(ConnectError::UnknownNode));
        assert!(g.connections.is_empty());
    }

    #[test]
    fn disconnect_removes_only_that_input() {
        let (mut g, out) = graph_with_output();
        let a = add_node(&mut g, "math/float", [0.0, 0.0]).unwrap();
        connect(&mut g, a, "value", out, "lifetime").unwrap();
        connect(&mut g, a, "value", out, "drag").unwrap();
        assert!(disconnect(&mut g, out, "lifetime"));
        assert!(!disconnect(&mut g, out, "lifetime"));
        assert!(is_connected(&g, out, "drag"));
    }

    #[test]
    fn curve_and_gradient_nodes_start_with_a_visible_table() {
        let (mut g, _) = graph_with_output();
        let c = add_node(&mut g, "curve/curve", [0.0, 0.0]).unwrap();
        let gr = add_node(&mut g, "curve/gradient", [0.0, 0.0]).unwrap();
        assert!(matches!(node(&g, c).unwrap().input_values.get("points"), Some(PinValue::Curve(p)) if p.len() == 2));
        assert!(matches!(node(&g, gr).unwrap().input_values.get("stops"), Some(PinValue::Gradient(s)) if s.len() == 2));
    }

    /// The edit operations must leave a graph the compiler accepts.
    #[test]
    fn an_edited_graph_compiles() {
        let (mut g, out) = graph_with_output();
        let shape = add_node(&mut g, "shape/sphere", [0.0, 0.0]).unwrap();
        let curve = add_node(&mut g, "curve/curve", [0.0, 0.0]).unwrap();
        connect(&mut g, shape, "position", out, "position").unwrap();
        connect(&mut g, shape, "direction", out, "velocity").unwrap();
        connect(&mut g, curve, "value", out, "drag").unwrap();
        set_value(&mut g, out, "spawn_rate", PinValue::Float(40.0));
        compile_graph(&g).expect("edited graph compiles");
    }

    /// Save and load an edited document: every edit, comments included, must
    /// come back exactly.
    #[test]
    fn an_edited_document_round_trips() {
        let mut asset = parse_vfx(PRESETS[0].source).unwrap();
        let idx = add_emitter(&mut asset);
        let g = &mut asset.emitters[idx].graph;
        let out = g.nodes[0].id;
        let f = add_node(g, "math/float", [12.5, -40.0]).unwrap();
        set_value(g, f, "value", PinValue::Float(2.5));
        connect(g, f, "value", out, "lifetime").unwrap();
        add_comment(g, [1.0, 2.0, 300.0, 200.0]);
        rename_emitter(&mut asset, idx, "Embers");

        let text = write_vfx(&asset).unwrap();
        let back = parse_vfx(&text).unwrap();
        assert_eq!(back, asset);
    }

    /// A graph nobody commented must save exactly as before the field existed.
    #[test]
    fn an_uncommented_graph_writes_no_comments_field() {
        let asset = parse_vfx(PRESETS[1].source).unwrap();
        assert!(!write_vfx(&asset).unwrap().contains("comments"));
    }

    #[test]
    fn emitters_get_unique_names_and_the_last_one_stays() {
        let mut asset = VfxAsset::default();
        let a = add_emitter(&mut asset);
        let b = add_emitter(&mut asset);
        assert_eq!(asset.emitters[a].name, "Emitter");
        assert_eq!(asset.emitters[b].name, "Emitter 2");
        assert_ne!(asset.emitters[a].seed, asset.emitters[b].seed);
        assert!(!rename_emitter(&mut asset, b, "Emitter"), "taken names are refused");
        assert!(!rename_emitter(&mut asset, b, "  "), "blank names are refused");
        assert!(rename_emitter(&mut asset, b, "Embers"));
        assert!(remove_emitter(&mut asset, a));
        assert!(!remove_emitter(&mut asset, 0), "the last emitter stays");
        assert_eq!(asset.emitters[0].name, "Embers");
    }

    #[test]
    fn duplicating_an_emitter_copies_its_graph_under_a_new_name() {
        let mut asset = parse_vfx(PRESETS[0].source).unwrap();
        let n = asset.emitters.len();
        let copy = duplicate_emitter(&mut asset, 0).unwrap();
        assert_eq!(asset.emitters.len(), n + 1);
        assert_eq!(asset.emitters[copy].graph, asset.emitters[0].graph);
        assert_ne!(asset.emitters[copy].name, asset.emitters[0].name);
    }

    #[test]
    fn normalize_gives_every_graph_an_output() {
        let mut asset = VfxAsset::default();
        assert!(normalize(&mut asset));
        assert_eq!(asset.emitters.len(), 1);
        asset.emitters[0].graph.nodes.clear();
        assert!(normalize(&mut asset));
        assert!(asset.emitters[0].graph.nodes.iter().any(|n| n.node_type == OUTPUT_NODE));
        assert!(!normalize(&mut asset), "an already valid document is left alone");
    }

    #[test]
    fn each_new_parameter_gets_a_name_of_its_own() {
        let mut g = VfxGraph::default();
        let a = add_node(&mut g, "input/parameter", [0.0, 0.0]).unwrap();
        let b = add_node(&mut g, "input/parameter", [0.0, 0.0]).unwrap();
        let name = |id| node(&g, id).unwrap().input_values.get("name").cloned();
        assert_eq!(name(a), Some(PinValue::Text("param".into())));
        assert_eq!(name(b), Some(PinValue::Text("param2".into())));
    }
}
