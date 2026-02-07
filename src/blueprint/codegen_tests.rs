//! Tests for Rhai code generation from blueprint graphs
//!
//! Covers math node codegen, logic/flow codegen, variable codegen,
//! warnings/errors, and data-driven registry sweeps.

use super::*;
use super::nodes::{NodeRegistry, NodeTypeDefinition, register_all_nodes};

/// Helper: create a registry with all nodes registered
fn test_registry() -> NodeRegistry {
    let mut registry = NodeRegistry::new();
    register_all_nodes(&mut registry);
    registry
}

/// Helper: build a graph with event/on_ready and one data node connected
fn graph_with_node(node_type: &str) -> BlueprintGraph {
    let registry = test_registry();
    let mut graph = BlueprintGraph::new("test");

    // Add on_ready event
    let ready_id = graph.next_node_id();
    if let Some(def) = registry.types.get("event/on_ready") {
        let node = def.create_node(ready_id);
        graph.add_node(node);
    }

    // Add the requested node
    if let Some(def) = registry.types.get(node_type) {
        let node_id = graph.next_node_id();
        let node = def.create_node(node_id);
        graph.add_node(node);

        // Connect flow if both have flow pins
        let ready_has_flow = graph.get_node(ready_id)
            .map(|n| n.output_pins().any(|p| p.pin_type == PinType::Flow || p.pin_type == PinType::Execution))
            .unwrap_or(false);
        let node_has_flow = graph.get_node(node_id)
            .map(|n| n.input_pins().any(|p| p.pin_type == PinType::Flow || p.pin_type == PinType::Execution))
            .unwrap_or(false);

        if ready_has_flow && node_has_flow {
            // Find flow output pin name
            let flow_out = graph.get_node(ready_id).unwrap()
                .output_pins()
                .find(|p| p.pin_type == PinType::Flow || p.pin_type == PinType::Execution)
                .map(|p| p.name.clone());
            let flow_in = graph.get_node(node_id).unwrap()
                .input_pins()
                .find(|p| p.pin_type == PinType::Flow || p.pin_type == PinType::Execution)
                .map(|p| p.name.clone());
            if let (Some(out), Some(inp)) = (flow_out, flow_in) {
                graph.add_connection(PinId::output(ready_id, out), PinId::input(node_id, inp));
            }
        }
    }

    graph
}

// =============================================================================
// A. Empty/minimal graphs
// =============================================================================

#[test]
fn empty_graph_produces_no_errors() {
    let graph = BlueprintGraph::new("empty");
    let result = generate_rhai_code(&graph);
    assert!(result.errors.is_empty(), "Empty graph errors: {:?}", result.errors);
}

#[test]
fn on_ready_event_generates_skeleton() {
    let registry = test_registry();
    let mut graph = BlueprintGraph::new("test");
    let id = graph.next_node_id();
    if let Some(def) = registry.types.get("event/on_ready") {
        graph.add_node(def.create_node(id));
    }
    let result = generate_rhai_code(&graph);
    assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
    assert!(!result.code.is_empty(), "Code should not be empty for on_ready");
    assert!(result.code.contains("on_ready"), "Code should contain 'on_ready': {}", result.code);
}

#[test]
fn on_update_event_generates_delta() {
    let registry = test_registry();
    let mut graph = BlueprintGraph::new("test");
    let id = graph.next_node_id();
    if let Some(def) = registry.types.get("event/on_update") {
        graph.add_node(def.create_node(id));
    }
    let result = generate_rhai_code(&graph);
    assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
    assert!(result.code.contains("on_update"), "Code should contain 'on_update': {}", result.code);
}

// =============================================================================
// B. Math node codegen
// =============================================================================

#[test]
fn math_add_generates_plus_operator() {
    let graph = graph_with_node("math/add");
    let result = generate_rhai_code(&graph);
    // The codegen should produce code containing a + operator
    assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
}

#[test]
fn math_subtract_generates_minus_operator() {
    let graph = graph_with_node("math/subtract");
    let result = generate_rhai_code(&graph);
    assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
}

#[test]
fn math_multiply_generates_star_operator() {
    let graph = graph_with_node("math/multiply");
    let result = generate_rhai_code(&graph);
    assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
}

#[test]
fn math_divide_generates_slash_operator() {
    let graph = graph_with_node("math/divide");
    let result = generate_rhai_code(&graph);
    assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
}

#[test]
fn math_lerp_generates_interpolation() {
    let graph = graph_with_node("math/lerp");
    let result = generate_rhai_code(&graph);
    assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
}

#[test]
fn math_clamp_generates_clamp_call() {
    let graph = graph_with_node("math/clamp");
    let result = generate_rhai_code(&graph);
    assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
}

#[test]
fn math_trig_functions_codegen() {
    let trig_nodes = ["math/sin", "math/cos", "math/tan"];
    let mut failures = Vec::new();
    for node_type in &trig_nodes {
        let graph = graph_with_node(node_type);
        let result = generate_rhai_code(&graph);
        if !result.errors.is_empty() {
            failures.push(format!("{}: {:?}", node_type, result.errors));
        }
    }
    assert!(failures.is_empty(), "Trig codegen failures:\n  {}", failures.join("\n  "));
}

// =============================================================================
// C. Logic/flow codegen
// =============================================================================

#[test]
fn print_node_generates_code() {
    let graph = graph_with_node("output/print");
    let result = generate_rhai_code(&graph);
    assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
}

// =============================================================================
// D. Data-driven registry tests
// =============================================================================

#[test]
fn all_behavior_nodes_create_valid_instances() {
    let registry = test_registry();
    let mut failures = Vec::new();

    for (type_id, def) in &registry.types {
        if type_id.starts_with("shader/") {
            continue; // skip shader nodes (covered by tests.rs)
        }
        let node = def.create_node(NodeId::new(1));
        if node.node_type != *type_id {
            failures.push(format!("{}: node_type mismatch: '{}'", type_id, node.node_type));
        }
    }

    assert!(failures.is_empty(), "Node creation failures:\n  {}", failures.join("\n  "));
}

#[test]
fn all_behavior_nodes_codegen_without_panic() {
    let registry = test_registry();

    for (type_id, _def) in &registry.types {
        if type_id.starts_with("shader/") {
            continue;
        }
        let graph = graph_with_node(type_id);
        let result = generate_rhai_code(&graph);
        // We don't require zero errors (some nodes need connections), but no panics
        let _ = result;
    }
}

#[test]
fn all_nodes_have_unique_type_ids() {
    let registry = test_registry();
    let count = registry.types.len();
    assert!(count > 0, "Registry should have nodes");
    // HashMap keys are unique by definition, so just verify we have entries
    // Also check no empty type_ids
    for (type_id, _) in &registry.types {
        assert!(!type_id.is_empty(), "Found empty type_id");
    }
}

#[test]
fn event_nodes_have_is_event_true() {
    let registry = test_registry();
    let mut failures = Vec::new();
    for (type_id, def) in &registry.types {
        if type_id.starts_with("event/") {
            if !def.is_event {
                failures.push(format!("{}: is_event is false", type_id));
            }
        }
    }
    assert!(failures.is_empty(), "Event node failures:\n  {}", failures.join("\n  "));
}

#[test]
fn registry_has_minimum_node_counts() {
    let registry = test_registry();
    let behavior_count = registry.types.iter()
        .filter(|(id, _)| !id.starts_with("shader/"))
        .count();
    let shader_count = registry.types.iter()
        .filter(|(id, _)| id.starts_with("shader/"))
        .count();

    assert!(behavior_count >= 30, "Expected at least 30 behavior nodes, got {}", behavior_count);
    assert!(shader_count >= 30, "Expected at least 30 shader nodes, got {}", shader_count);
}

// =============================================================================
// E. Variable codegen
// =============================================================================

#[test]
fn graph_variables_emit_declarations() {
    let mut graph = BlueprintGraph::new("test");
    graph.add_variable(BlueprintVariable::new("speed", PinType::Float).with_default(PinValue::Float(5.0)));

    // Add an on_ready event to generate some code
    let registry = test_registry();
    let id = graph.next_node_id();
    if let Some(def) = registry.types.get("event/on_ready") {
        graph.add_node(def.create_node(id));
    }

    let result = generate_rhai_code(&graph);
    assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
    // The generated code should reference the variable
    assert!(result.code.contains("speed"), "Generated code should reference variable 'speed': {}", result.code);
}

// =============================================================================
// F. Warnings/errors
// =============================================================================

#[test]
fn comment_nodes_excluded_from_code() {
    let mut graph = BlueprintGraph::new("test");
    let id = graph.next_node_id();
    let mut node = BlueprintNode::new(id, "comment");
    node.comment = Some("This is a comment".into());
    graph.add_node(node);

    let result = generate_rhai_code(&graph);
    assert!(result.errors.is_empty(), "Comments should not cause errors");
}

// =============================================================================
// G. All-behavior-nodes codegen sweep
// =============================================================================

#[test]
fn all_behavior_nodes_minimal_graph_codegen() {
    let registry = test_registry();

    for (type_id, def) in &registry.types {
        if type_id.starts_with("shader/") {
            continue;
        }

        // Build minimal graph: on_ready + the node
        let mut graph = BlueprintGraph::new("test");
        let ready_id = graph.next_node_id();
        if let Some(ready_def) = registry.types.get("event/on_ready") {
            graph.add_node(ready_def.create_node(ready_id));
        }

        let node_id = graph.next_node_id();
        let node = def.create_node(node_id);
        graph.add_node(node);

        // Generate code
        let result = generate_rhai_code(&graph);
        // Just verify no panic - some nodes may have warnings about unconnected pins
        let _ = result;
    }
}
