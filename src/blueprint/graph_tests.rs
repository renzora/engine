//! Tests for blueprint graph data structures
//!
//! Covers PinType compatibility, PinValue formatting, BlueprintGraph operations,
//! BlueprintNode operations, and variables.

use super::*;
use super::nodes::{NodeRegistry, register_all_nodes};

// =============================================================================
// A. PinType compatibility matrix
// =============================================================================

#[test]
fn same_types_can_connect() {
    let types = [
        PinType::Float, PinType::Int, PinType::Bool, PinType::String,
        PinType::Vec2, PinType::Vec3, PinType::Vec4, PinType::Color,
        PinType::Entity, PinType::EntityArray, PinType::StringArray,
        PinType::Asset, PinType::AudioHandle, PinType::TimerHandle,
        PinType::SceneHandle, PinType::PrefabHandle, PinType::GltfHandle,
        PinType::Texture2D, PinType::Sampler,
    ];

    let mut failures = Vec::new();
    for t in &types {
        if !t.can_connect_to(t) {
            failures.push(format!("{:?} cannot connect to itself", t));
        }
    }
    assert!(failures.is_empty(), "Self-connection failures:\n  {}", failures.join("\n  "));
}

#[test]
fn any_connects_to_everything() {
    let types = [
        PinType::Float, PinType::Int, PinType::Bool, PinType::String,
        PinType::Vec2, PinType::Vec3, PinType::Flow, PinType::Execution,
        PinType::Color, PinType::Entity, PinType::Custom("test".into()),
    ];
    for t in &types {
        assert!(PinType::Any.can_connect_to(t), "Any should connect to {:?}", t);
        assert!(t.can_connect_to(&PinType::Any), "{:?} should connect to Any", t);
    }
}

#[test]
fn flow_and_execution_interchangeable() {
    assert!(PinType::Flow.can_connect_to(&PinType::Execution));
    assert!(PinType::Execution.can_connect_to(&PinType::Flow));
    assert!(PinType::Flow.can_connect_to(&PinType::Flow));
    assert!(PinType::Execution.can_connect_to(&PinType::Execution));
}

#[test]
fn color_and_vec4_compatible() {
    assert!(PinType::Color.can_connect_to(&PinType::Vec4));
    assert!(PinType::Vec4.can_connect_to(&PinType::Color));
}

#[test]
fn incompatible_types_rejected() {
    let pairs = [
        (PinType::Float, PinType::Bool),
        (PinType::String, PinType::Entity),
        (PinType::Int, PinType::Vec3),
        (PinType::Float, PinType::Flow),
        (PinType::Bool, PinType::Color),
        (PinType::Vec2, PinType::Vec3),
        (PinType::Entity, PinType::StringArray),
    ];
    for (a, b) in &pairs {
        assert!(!a.can_connect_to(b), "{:?} should NOT connect to {:?}", a, b);
    }
}

#[test]
fn custom_type_same_name_connects() {
    let a = PinType::Custom("MyType".into());
    let b = PinType::Custom("MyType".into());
    assert!(a.can_connect_to(&b));
}

#[test]
fn custom_type_different_name_rejects() {
    let a = PinType::Custom("TypeA".into());
    let b = PinType::Custom("TypeB".into());
    assert!(!a.can_connect_to(&b));
}

#[test]
fn pin_type_compatibility_symmetry() {
    let types = [
        PinType::Float, PinType::Int, PinType::Bool, PinType::String,
        PinType::Vec2, PinType::Vec3, PinType::Vec4, PinType::Color,
        PinType::Entity, PinType::Flow, PinType::Execution,
    ];
    let mut failures = Vec::new();
    for a in &types {
        for b in &types {
            // Skip Any since it's always symmetric
            if *a == PinType::Any || *b == PinType::Any {
                continue;
            }
            if a.can_connect_to(b) != b.can_connect_to(a) {
                failures.push(format!("{:?}<->{:?} not symmetric", a, b));
            }
        }
    }
    assert!(failures.is_empty(), "Symmetry violations:\n  {}", failures.join("\n  "));
}

// =============================================================================
// B. PinValue to_rhai() formatting
// =============================================================================

#[test]
fn pin_value_to_rhai_float() {
    let v = PinValue::Float(3.14);
    let s = v.to_rhai();
    assert!(s.contains("3.14"), "Expected '3.14' in '{}'", s);
}

#[test]
fn pin_value_to_rhai_string_escaping() {
    let v = PinValue::String(r#"hello "world""#.into());
    let s = v.to_rhai();
    assert!(s.contains(r#"hello \"world\""#), "Expected escaped quotes in '{}'", s);
}

#[test]
fn pin_value_to_rhai_vec3() {
    let v = PinValue::Vec3([1.0, 2.0, 3.0]);
    let s = v.to_rhai();
    assert!(s.starts_with("vec3("), "Expected 'vec3(' prefix in '{}'", s);
}

#[test]
fn pin_value_to_rhai_color() {
    let v = PinValue::Color([1.0, 0.0, 0.0, 1.0]);
    let s = v.to_rhai();
    assert!(s.starts_with("color("), "Expected 'color(' prefix in '{}'", s);
}

#[test]
fn pin_value_to_rhai_entity_array() {
    let v = PinValue::EntityArray(vec![1, 2, 3]);
    let s = v.to_rhai();
    assert!(s.starts_with('['), "Expected array in '{}'", s);
    assert!(s.contains("entity(1)"), "Expected entity(1) in '{}'", s);
}

#[test]
fn pin_value_to_rhai_bool() {
    assert_eq!(PinValue::Bool(true).to_rhai(), "true");
    assert_eq!(PinValue::Bool(false).to_rhai(), "false");
}

#[test]
fn pin_value_to_rhai_int() {
    assert_eq!(PinValue::Int(42).to_rhai(), "42");
}

// =============================================================================
// B2. PinValue to_wgsl() formatting
// =============================================================================

#[test]
fn pin_value_to_wgsl_float() {
    let s = PinValue::Float(1.5).to_wgsl();
    assert!(s.contains("1.5"), "Expected '1.5' in '{}'", s);
}

#[test]
fn pin_value_to_wgsl_int_suffix() {
    let s = PinValue::Int(7).to_wgsl();
    assert!(s.ends_with('i'), "Expected 'i' suffix in '{}'", s);
}

#[test]
fn pin_value_to_wgsl_vec2() {
    let s = PinValue::Vec2([1.0, 2.0]).to_wgsl();
    assert!(s.starts_with("vec2<f32>("), "Expected 'vec2<f32>(' in '{}'", s);
}

#[test]
fn pin_value_to_wgsl_color_as_vec4() {
    let s = PinValue::Color([1.0, 0.0, 0.0, 1.0]).to_wgsl();
    assert!(s.starts_with("vec4<f32>("), "Color should format as vec4<f32> in WGSL, got '{}'", s);
}

#[test]
fn pin_value_to_wgsl_runtime_types() {
    let runtime_values = [
        PinValue::Entity(0),
        PinValue::EntityArray(vec![]),
        PinValue::Asset("test".into()),
        PinValue::AudioHandle(0),
    ];
    for v in &runtime_values {
        assert_eq!(v.to_wgsl(), "/* runtime type */", "Runtime type {:?} should produce comment", v);
    }
}

// =============================================================================
// C. PinValue defaults
// =============================================================================

#[test]
fn default_for_type_roundtrip() {
    let types = [
        PinType::Float, PinType::Int, PinType::Bool, PinType::String,
        PinType::Vec2, PinType::Vec3, PinType::Vec4, PinType::Color,
        PinType::Flow, PinType::Execution, PinType::Entity,
        PinType::EntityArray, PinType::StringArray, PinType::Asset,
        PinType::Texture2D, PinType::Sampler,
        PinType::AudioHandle, PinType::TimerHandle,
    ];
    let mut failures = Vec::new();
    for t in &types {
        let default_val = PinValue::default_for_type(t.clone());
        let round = default_val.pin_type();
        // Flow and Execution both produce PinValue::Flow whose pin_type() is Flow
        if *t == PinType::Execution {
            if round != PinType::Flow {
                failures.push(format!("{:?} → {:?} (expected Flow)", t, round));
            }
        } else if round != *t {
            failures.push(format!("{:?} → {:?}", t, round));
        }
    }
    assert!(failures.is_empty(), "Roundtrip failures:\n  {}", failures.join("\n  "));
}

#[test]
fn default_color_is_white() {
    if let PinValue::Color(c) = PinValue::default_for_type(PinType::Color) {
        assert_eq!(c, [1.0, 1.0, 1.0, 1.0], "Default color should be white");
    } else {
        panic!("default_for_type(Color) should return Color variant");
    }
}

#[test]
fn default_float_is_zero() {
    if let PinValue::Float(v) = PinValue::default_for_type(PinType::Float) {
        assert_eq!(v, 0.0);
    } else {
        panic!("default_for_type(Float) should return Float variant");
    }
}

// =============================================================================
// D. BlueprintGraph operations
// =============================================================================

#[test]
fn new_graph_is_empty() {
    let g = BlueprintGraph::new("test");
    assert_eq!(g.nodes.len(), 0);
    assert_eq!(g.connections.len(), 0);
    assert!(g.variables.is_empty());
    assert!(g.is_behavior());
}

#[test]
fn new_material_graph() {
    let g = BlueprintGraph::new_material("mat_test");
    assert!(g.is_material());
    assert!(!g.is_behavior());
}

#[test]
fn add_and_get_node() {
    let mut g = BlueprintGraph::new("test");
    let id = g.next_node_id();
    let node = BlueprintNode::new(id, "math/add");
    g.add_node(node);
    assert!(g.get_node(id).is_some());
    assert_eq!(g.get_node(id).unwrap().node_type, "math/add");
}

#[test]
fn next_node_id_increments() {
    let mut g = BlueprintGraph::new("test");
    let id1 = g.next_node_id();
    let id2 = g.next_node_id();
    assert_ne!(id1, id2);
    assert!(id2.0 > id1.0);
}

#[test]
fn remove_node_cascades_connections() {
    let mut g = BlueprintGraph::new("test");
    let id1 = g.next_node_id();
    let id2 = g.next_node_id();

    let mut n1 = BlueprintNode::new(id1, "source");
    n1.pins.push(Pin::output("out", "Out", PinType::Float));
    let mut n2 = BlueprintNode::new(id2, "sink");
    n2.pins.push(Pin::input("in", "In", PinType::Float));

    g.add_node(n1);
    g.add_node(n2);
    g.add_connection(PinId::output(id1, "out"), PinId::input(id2, "in"));
    assert_eq!(g.connections.len(), 1);

    g.remove_node(id1);
    assert_eq!(g.connections.len(), 0);
    assert!(g.get_node(id1).is_none());
}

#[test]
fn add_connection_validates_types() {
    let mut g = BlueprintGraph::new("test");
    let id1 = g.next_node_id();
    let id2 = g.next_node_id();

    let mut n1 = BlueprintNode::new(id1, "a");
    n1.pins.push(Pin::output("out", "Out", PinType::Float));
    let mut n2 = BlueprintNode::new(id2, "b");
    n2.pins.push(Pin::input("in", "In", PinType::Bool)); // incompatible

    g.add_node(n1);
    g.add_node(n2);

    let result = g.add_connection(PinId::output(id1, "out"), PinId::input(id2, "in"));
    assert!(!result, "Incompatible types should return false");
    assert_eq!(g.connections.len(), 0);
}

#[test]
fn add_connection_replaces_existing_input() {
    let mut g = BlueprintGraph::new("test");
    let id1 = g.next_node_id();
    let id2 = g.next_node_id();
    let id3 = g.next_node_id();

    let mut n1 = BlueprintNode::new(id1, "a");
    n1.pins.push(Pin::output("out", "Out", PinType::Float));
    let mut n2 = BlueprintNode::new(id2, "b");
    n2.pins.push(Pin::output("out", "Out", PinType::Float));
    let mut n3 = BlueprintNode::new(id3, "c");
    n3.pins.push(Pin::input("in", "In", PinType::Float));

    g.add_node(n1);
    g.add_node(n2);
    g.add_node(n3);

    // Connect id1 -> id3
    assert!(g.add_connection(PinId::output(id1, "out"), PinId::input(id3, "in")));
    assert_eq!(g.connections.len(), 1);

    // Connect id2 -> id3 (should replace the first connection)
    assert!(g.add_connection(PinId::output(id2, "out"), PinId::input(id3, "in")));
    assert_eq!(g.connections.len(), 1);
    assert_eq!(g.connections[0].from.node_id, id2);
}

#[test]
fn connection_to_and_from() {
    let mut g = BlueprintGraph::new("test");
    let id1 = g.next_node_id();
    let id2 = g.next_node_id();

    let mut n1 = BlueprintNode::new(id1, "a");
    n1.pins.push(Pin::output("out", "Out", PinType::Float));
    let mut n2 = BlueprintNode::new(id2, "b");
    n2.pins.push(Pin::input("in", "In", PinType::Float));

    g.add_node(n1);
    g.add_node(n2);
    g.add_connection(PinId::output(id1, "out"), PinId::input(id2, "in"));

    let conn = g.connection_to(&PinId::input(id2, "in"));
    assert!(conn.is_some());
    assert_eq!(conn.unwrap().from.node_id, id1);

    let out_pin = PinId::output(id1, "out");
    let from_conns: Vec<_> = g.connections_from(&out_pin).collect();
    assert_eq!(from_conns.len(), 1);
}

#[test]
fn is_pin_connected_both_directions() {
    let mut g = BlueprintGraph::new("test");
    let id1 = g.next_node_id();
    let id2 = g.next_node_id();

    let mut n1 = BlueprintNode::new(id1, "a");
    n1.pins.push(Pin::output("out", "Out", PinType::Float));
    let mut n2 = BlueprintNode::new(id2, "b");
    n2.pins.push(Pin::input("in", "In", PinType::Float));

    g.add_node(n1);
    g.add_node(n2);
    g.add_connection(PinId::output(id1, "out"), PinId::input(id2, "in"));

    assert!(g.is_pin_connected(&PinId::output(id1, "out")));
    assert!(g.is_pin_connected(&PinId::input(id2, "in")));
    assert!(!g.is_pin_connected(&PinId::input(id1, "nonexistent")));
}

#[test]
fn remove_connections_for_pin() {
    let mut g = BlueprintGraph::new("test");
    let id1 = g.next_node_id();
    let id2 = g.next_node_id();

    let mut n1 = BlueprintNode::new(id1, "a");
    n1.pins.push(Pin::output("out", "Out", PinType::Float));
    let mut n2 = BlueprintNode::new(id2, "b");
    n2.pins.push(Pin::input("in", "In", PinType::Float));

    g.add_node(n1);
    g.add_node(n2);
    g.add_connection(PinId::output(id1, "out"), PinId::input(id2, "in"));

    g.remove_connections_for_pin(&PinId::output(id1, "out"));
    assert_eq!(g.connections.len(), 0);
}

#[test]
fn event_nodes_filters_by_prefix() {
    let mut g = BlueprintGraph::new("test");
    let id1 = g.next_node_id();
    let id2 = g.next_node_id();
    let id3 = g.next_node_id();

    g.add_node(BlueprintNode::new(id1, "event/on_ready"));
    g.add_node(BlueprintNode::new(id2, "math/add"));
    g.add_node(BlueprintNode::new(id3, "event/on_update"));

    let events: Vec<_> = g.event_nodes().collect();
    assert_eq!(events.len(), 2);
}

// =============================================================================
// E. BlueprintNode operations
// =============================================================================

#[test]
fn node_get_input_value_chain() {
    let mut node = BlueprintNode::new(NodeId::new(1), "test");
    node.pins.push(Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(5.0)));

    // Pin default
    assert_eq!(node.get_input_value("x"), Some(PinValue::Float(5.0)));

    // Override
    node.set_input_value("x", PinValue::Float(10.0));
    assert_eq!(node.get_input_value("x"), Some(PinValue::Float(10.0)));

    // Missing pin
    assert_eq!(node.get_input_value("nonexistent"), None);
}

#[test]
fn node_input_output_pins_filter() {
    let node = BlueprintNode::with_pins(
        NodeId::new(1),
        "test",
        vec![
            Pin::input("a", "A", PinType::Float),
            Pin::input("b", "B", PinType::Float),
            Pin::output("result", "Result", PinType::Float),
        ],
    );
    assert_eq!(node.input_pins().count(), 2);
    assert_eq!(node.output_pins().count(), 1);
}

// =============================================================================
// F. Variables & types
// =============================================================================

#[test]
fn graph_add_remove_get_variable() {
    let mut g = BlueprintGraph::new("test");
    let var = BlueprintVariable::new("speed", PinType::Float);
    g.add_variable(var);
    assert!(g.get_variable("speed").is_some());

    g.remove_variable("speed");
    assert!(g.get_variable("speed").is_none());
}

#[test]
fn blueprint_variable_default_matches_type() {
    let var = BlueprintVariable::new("score", PinType::Int);
    assert_eq!(var.default_value.pin_type(), PinType::Int);
}

#[test]
fn blueprint_type_is_category_allowed() {
    // Behavior allows non-shader categories
    assert!(BlueprintType::Behavior.is_category_allowed("Math"));
    assert!(BlueprintType::Behavior.is_category_allowed("Logic"));
    assert!(!BlueprintType::Behavior.is_category_allowed("Shader Nodes"));

    // Material allows shader + Math
    assert!(BlueprintType::Material.is_category_allowed("Shader Nodes"));
    assert!(BlueprintType::Material.is_category_allowed("Math"));
    assert!(!BlueprintType::Material.is_category_allowed("Logic"));
}

// =============================================================================
// G. Node registry meta
// =============================================================================

#[test]
fn all_pin_types_have_colors() {
    let types = [
        PinType::Flow, PinType::Execution, PinType::Float, PinType::Int,
        PinType::Bool, PinType::String, PinType::Vec2, PinType::Vec3,
        PinType::Vec4, PinType::Color, PinType::Texture2D, PinType::Sampler,
        PinType::Any, PinType::Entity, PinType::EntityArray, PinType::StringArray,
        PinType::Asset, PinType::AudioHandle, PinType::TimerHandle,
        PinType::SceneHandle, PinType::PrefabHandle, PinType::GltfHandle,
        PinType::Custom("test".into()),
    ];
    for t in &types {
        let color = t.color();
        // Just verify it doesn't panic and returns valid RGB
        assert!(color[0] <= 255 && color[1] <= 255 && color[2] <= 255);
    }
}

#[test]
fn all_pin_types_have_names() {
    let types = [
        PinType::Flow, PinType::Execution, PinType::Float, PinType::Int,
        PinType::Bool, PinType::String, PinType::Vec2, PinType::Vec3,
        PinType::Custom("test".into()),
    ];
    for t in &types {
        let name = t.name();
        assert!(!name.is_empty(), "PinType {:?} should have a non-empty name", t);
    }
}

#[test]
fn blueprint_type_metadata() {
    assert_eq!(BlueprintType::Behavior.name(), "Behavior");
    assert_eq!(BlueprintType::Material.name(), "Material");
    assert_eq!(BlueprintType::Behavior.extension(), "blueprint");
    assert_eq!(BlueprintType::Material.extension(), "material_bp");
}
