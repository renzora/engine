//! What a brand-new `.blueprint` file contains.
//!
//! An empty graph (`{}`) is technically valid but a dead end: the canvas opens
//! blank, and nothing in a blueprint runs unless it hangs off an event, so the
//! first thing anyone does is right-click and add one. Starting every new file
//! with the two lifecycle events already placed turns that into "drag a wire
//! from here", which is the actual first step.
//!
//! They're deliberately left **unwired** — an on_ready with nothing attached
//! compiles to an empty `function on_ready()`, which is harmless, whereas
//! guessing at a body would be something to delete rather than build on.

use crate::graph::BlueprintGraph;

/// Column spacing the auto-layout pass uses between rows — reused here so the
/// two starter nodes sit exactly where "Auto Layout" would put them, and the
/// file doesn't visibly shuffle the first time it's tidied.
const ROW_GAP: f32 = 150.0;

/// A new blueprint's starting graph: **On Ready** above **On Update**, both at
/// the canvas origin so they're in view when the graph opens.
pub fn starter_graph() -> BlueprintGraph {
    let mut graph = BlueprintGraph::new();
    graph.add_node("event/on_ready", [0.0, 0.0]);
    graph.add_node("event/on_update", [0.0, ROW_GAP]);
    graph
}

/// [`starter_graph`] serialized as the JSON a `.blueprint` file holds — what
/// every "new blueprint" action in the editor writes to disk.
pub fn starter_blueprint_json() -> String {
    // Pretty-printed to match what the blueprint editor's own save produces, so
    // a starter file and a saved one don't differ in formatting alone.
    serde_json::to_string_pretty(&starter_graph()).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The starter must round-trip through the same path `load_blueprint_file`
    /// takes — a file the editor can't parse would silently open as an empty
    /// graph, hiding the regression.
    #[test]
    fn starter_json_reloads_to_the_same_graph() {
        let json = starter_blueprint_json();
        let back: BlueprintGraph = serde_json::from_str(&json).expect("starter must parse");
        assert_eq!(back, starter_graph());
    }

    #[test]
    fn starter_has_both_lifecycle_events() {
        let g = starter_graph();
        let types: Vec<&str> = g.nodes.iter().map(|n| n.node_type.as_str()).collect();
        assert!(types.contains(&"event/on_ready"), "missing on_ready: {types:?}");
        assert!(types.contains(&"event/on_update"), "missing on_update: {types:?}");
        assert!(g.connections.is_empty(), "starter nodes ship unwired");
    }

    /// Both node types must exist in the registry — a typo here would produce a
    /// file full of nodes the canvas can't draw.
    #[test]
    fn starter_node_types_are_registered() {
        for node in starter_graph().nodes {
            assert!(
                crate::node_def(&node.node_type).is_some(),
                "unknown node type {}",
                node.node_type
            );
        }
    }
}
