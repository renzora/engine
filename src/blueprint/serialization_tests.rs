//! Tests for blueprint serialization/deserialization
//!
//! Covers BlueprintFile, graph serde roundtrips, and pin/type serialization.

use super::*;

// =============================================================================
// A. BlueprintFile
// =============================================================================

#[test]
fn blueprint_file_version_is_current() {
    let graph = BlueprintGraph::new("test");
    let file = BlueprintFile::new(graph);
    assert_eq!(file.version, BlueprintFile::CURRENT_VERSION);
}

#[test]
fn blueprint_file_constructor() {
    let graph = BlueprintGraph::new("my_blueprint");
    let file = BlueprintFile::new(graph);
    assert_eq!(file.graph.name, "my_blueprint");
    assert_eq!(file.version, 1);
}

#[test]
fn blueprint_file_json_roundtrip() {
    let graph = BlueprintGraph::new("roundtrip_test");
    let file = BlueprintFile::new(graph);

    let json = serde_json::to_string(&file).expect("serialize");
    let restored: BlueprintFile = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored.version, file.version);
    assert_eq!(restored.graph.name, "roundtrip_test");
}

// =============================================================================
// B. Graph serde roundtrip
// =============================================================================

#[test]
fn empty_graph_roundtrip() {
    let graph = BlueprintGraph::new("empty");
    let json = serde_json::to_string(&graph).unwrap();
    let restored: BlueprintGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.name, "empty");
    assert_eq!(restored.nodes.len(), 0);
    assert_eq!(restored.connections.len(), 0);
}

#[test]
fn graph_with_nodes_roundtrip() {
    let mut graph = BlueprintGraph::new("with_nodes");
    let id = graph.next_node_id();
    graph.add_node(BlueprintNode::new(id, "math/add"));

    let json = serde_json::to_string(&graph).unwrap();
    let restored: BlueprintGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.nodes.len(), 1);
    assert_eq!(restored.nodes[0].node_type, "math/add");
}

#[test]
fn graph_with_connections_roundtrip() {
    let mut graph = BlueprintGraph::new("with_conns");
    let id1 = graph.next_node_id();
    let id2 = graph.next_node_id();

    let mut n1 = BlueprintNode::new(id1, "a");
    n1.pins.push(Pin::output("out", "Out", PinType::Float));
    let mut n2 = BlueprintNode::new(id2, "b");
    n2.pins.push(Pin::input("in", "In", PinType::Float));

    graph.add_node(n1);
    graph.add_node(n2);
    graph.add_connection(PinId::output(id1, "out"), PinId::input(id2, "in"));

    let json = serde_json::to_string(&graph).unwrap();
    let restored: BlueprintGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.connections.len(), 1);
}

#[test]
fn graph_with_variables_roundtrip() {
    let mut graph = BlueprintGraph::new("with_vars");
    graph.add_variable(BlueprintVariable::new("speed", PinType::Float).with_default(PinValue::Float(5.0)));

    let json = serde_json::to_string(&graph).unwrap();
    let restored: BlueprintGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.variables.len(), 1);
    assert_eq!(restored.variables[0].name, "speed");
}

#[test]
fn graph_with_input_values_roundtrip() {
    let mut graph = BlueprintGraph::new("with_vals");
    let id = graph.next_node_id();
    let mut node = BlueprintNode::new(id, "math/add");
    node.set_input_value("a", PinValue::Float(42.0));
    graph.add_node(node);

    let json = serde_json::to_string(&graph).unwrap();
    let restored: BlueprintGraph = serde_json::from_str(&json).unwrap();
    let restored_node = &restored.nodes[0];
    assert_eq!(restored_node.get_input_value("a"), Some(PinValue::Float(42.0)));
}

// =============================================================================
// C. Pin/type serde
// =============================================================================

#[test]
fn all_pin_types_serde_roundtrip() {
    let types = [
        PinType::Flow, PinType::Execution, PinType::Float, PinType::Int,
        PinType::Bool, PinType::String, PinType::Vec2, PinType::Vec3,
        PinType::Vec4, PinType::Color, PinType::Texture2D, PinType::Sampler,
        PinType::Any, PinType::Entity, PinType::EntityArray, PinType::StringArray,
        PinType::Asset, PinType::AudioHandle, PinType::TimerHandle,
        PinType::SceneHandle, PinType::PrefabHandle, PinType::GltfHandle,
        PinType::Custom("test".into()),
    ];

    let mut failures = Vec::new();
    for t in &types {
        let json = serde_json::to_string(t);
        match json {
            Ok(j) => {
                let restored: Result<PinType, _> = serde_json::from_str(&j);
                match restored {
                    Ok(r) => {
                        if r != *t {
                            failures.push(format!("{:?}: roundtrip produced {:?}", t, r));
                        }
                    }
                    Err(e) => failures.push(format!("{:?}: deserialize error: {}", t, e)),
                }
            }
            Err(e) => failures.push(format!("{:?}: serialize error: {}", t, e)),
        }
    }
    assert!(failures.is_empty(), "PinType serde failures:\n  {}", failures.join("\n  "));
}

#[test]
fn all_pin_values_serde_roundtrip() {
    let values = [
        PinValue::Flow,
        PinValue::Float(3.14),
        PinValue::Int(42),
        PinValue::Bool(true),
        PinValue::String("hello".into()),
        PinValue::Vec2([1.0, 2.0]),
        PinValue::Vec3([1.0, 2.0, 3.0]),
        PinValue::Vec4([1.0, 2.0, 3.0, 4.0]),
        PinValue::Color([1.0, 0.5, 0.0, 1.0]),
        PinValue::Texture2D("tex.png".into()),
        PinValue::Sampler,
        PinValue::Entity(42),
        PinValue::EntityArray(vec![1, 2]),
        PinValue::StringArray(vec!["a".into(), "b".into()]),
        PinValue::Asset("asset.glb".into()),
        PinValue::AudioHandle(1),
        PinValue::TimerHandle(2),
        PinValue::SceneHandle(3),
        PinValue::PrefabHandle(4),
        PinValue::GltfHandle(5),
    ];

    let mut failures = Vec::new();
    for v in &values {
        let json = serde_json::to_string(v);
        match json {
            Ok(j) => {
                let restored: Result<PinValue, _> = serde_json::from_str(&j);
                match restored {
                    Ok(r) => {
                        if r != *v {
                            failures.push(format!("{:?}: roundtrip produced {:?}", v, r));
                        }
                    }
                    Err(e) => failures.push(format!("{:?}: deserialize error: {}", v, e)),
                }
            }
            Err(e) => failures.push(format!("{:?}: serialize error: {}", v, e)),
        }
    }
    assert!(failures.is_empty(), "PinValue serde failures:\n  {}", failures.join("\n  "));
}

#[test]
fn blueprint_type_serde_roundtrip() {
    for t in &[BlueprintType::Behavior, BlueprintType::Material] {
        let json = serde_json::to_string(t).unwrap();
        let restored: BlueprintType = serde_json::from_str(&json).unwrap();
        assert_eq!(*t, restored);
    }
}

#[test]
fn material_graph_type_preserved_through_serde() {
    let graph = BlueprintGraph::new_material("test_mat");
    let json = serde_json::to_string(&graph).unwrap();
    let restored: BlueprintGraph = serde_json::from_str(&json).unwrap();
    assert!(restored.is_material());
}
