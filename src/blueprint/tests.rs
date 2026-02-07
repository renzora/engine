//! Comprehensive data-driven tests for the material blueprint system.
//!
//! Validates that all registered shader node types work correctly across
//! WGSL codegen, preview evaluation, and pin name consistency.

use super::*;
use super::nodes::{NodeRegistry, NodeTypeDefinition, register_all_nodes};
use super::preview_eval;
use super::canvas::is_texture_node;

/// Returns all shader node definitions excluding output nodes.
fn all_shader_node_defs() -> Vec<(String, &'static NodeTypeDefinition)> {
    let mut registry = NodeRegistry::new();
    register_all_nodes(&mut registry);

    registry
        .types
        .iter()
        .filter(|(id, _)| id.starts_with("shader/"))
        .filter(|(id, _)| *id != "shader/pbr_output" && *id != "shader/unlit_output")
        .map(|(id, def)| (id.clone(), *def))
        .collect()
}

/// Build a minimal material graph containing a UV node, the test node, and a PBR output.
/// Connects UV → test node's first Vec2 input (if any), and the test node's first
/// connectable output → an appropriate PBR output input.
/// Returns (graph, test_node_id).
fn build_shader_node_graph(def: &NodeTypeDefinition) -> (BlueprintGraph, NodeId) {
    let mut graph = BlueprintGraph::new_material("test");

    // Create UV node
    let uv_id = graph.next_node_id();
    let uv_node = nodes::shader::UV.create_node(uv_id);
    graph.add_node(uv_node);

    // Create the test node
    let test_id = graph.next_node_id();
    let test_node = def.create_node(test_id);
    graph.add_node(test_node);

    // Create PBR output node
    let pbr_id = graph.next_node_id();
    let pbr_node = nodes::shader::PBR_OUTPUT.create_node(pbr_id);
    graph.add_node(pbr_node);

    // Connect UV "uv" output → test node's first Vec2 input (if any)
    let first_vec2_input = graph
        .get_node(test_id)
        .unwrap()
        .input_pins()
        .find(|p| p.pin_type == PinType::Vec2)
        .map(|p| p.name.clone());

    if let Some(vec2_pin) = first_vec2_input {
        graph.add_connection(
            PinId::output(uv_id, "uv"),
            PinId::input(test_id, vec2_pin),
        );
    }

    // Connect the test node's first output → appropriate PBR input
    let first_output = graph
        .get_node(test_id)
        .unwrap()
        .output_pins()
        .next()
        .map(|p| (p.name.clone(), p.pin_type.clone()));

    if let Some((out_name, out_type)) = first_output {
        // Pick a PBR input that's compatible with this output type
        let pbr_pin = match &out_type {
            PinType::Float => Some("roughness"),
            PinType::Color => Some("base_color"),
            PinType::Vec4 => Some("base_color"), // Vec4 <-> Color compatible
            PinType::Vec3 => Some("normal"),
            _ => None,
        };

        if let Some(pbr_pin_name) = pbr_pin {
            graph.add_connection(
                PinId::output(test_id, out_name),
                PinId::input(pbr_id, pbr_pin_name),
            );
        }
    }

    (graph, test_id)
}

// =============================================================================
// Test 1: All shader nodes compile to WGSL without errors
// =============================================================================

#[test]
fn all_shader_nodes_compile_to_wgsl() {
    let defs = all_shader_node_defs();
    assert!(!defs.is_empty(), "No shader node definitions found");

    let mut failures = Vec::new();

    for (type_id, def) in &defs {
        let (graph, _test_id) = build_shader_node_graph(def);
        let result = generate_wgsl_code(&graph);

        if !result.errors.is_empty() {
            failures.push(format!("{}: errors: {:?}", type_id, result.errors));
        } else if result.fragment_shader.len() < 100 {
            failures.push(format!(
                "{}: fragment shader too short ({} chars)",
                type_id,
                result.fragment_shader.len()
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "WGSL codegen failures for {} node(s):\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}

// =============================================================================
// Test 2: All shader nodes have preview evaluator handlers
// =============================================================================

/// Shader nodes that are known to lack preview evaluator handlers.
/// When you add a handler for one of these, remove it from this list.
/// The test will fail if a *new* unhandled node appears that's NOT in this list.
const PREVIEW_EVAL_KNOWN_GAPS: &[&str] = &[
    "shader/blend_add",
    "shader/blend_multiply",
    "shader/blend_overlay",
    "shader/blend_screen",
    "shader/blend_softlight",
    "shader/dither",
    "shader/edge_detect",
    "shader/gamma_to_linear",
    "shader/gradient_map",
    "shader/levels",
    "shader/linear_to_gamma",
    "shader/pixelate",
    "shader/sdf_intersection",
    "shader/sdf_smooth_union",
    "shader/sdf_union",
];

#[test]
fn all_shader_nodes_preview_eval() {
    let defs = all_shader_node_defs();
    assert!(!defs.is_empty(), "No shader node definitions found");

    let mut unhandled = Vec::new();

    for (type_id, def) in &defs {
        // Skip texture nodes - they return texture paths, handled separately
        if is_texture_node(type_id) {
            continue;
        }

        let mut graph = BlueprintGraph::new_material("test");

        // Create UV node
        let uv_id = graph.next_node_id();
        let uv_node = nodes::shader::UV.create_node(uv_id);
        graph.add_node(uv_node);

        // Create the test node
        let test_id = graph.next_node_id();
        let test_node = def.create_node(test_id);
        graph.add_node(test_node);

        // Connect UV → first Vec2 input if available
        let first_vec2_input = graph
            .get_node(test_id)
            .unwrap()
            .input_pins()
            .find(|p| p.pin_type == PinType::Vec2)
            .map(|p| p.name.clone());

        let first_output_name = graph
            .get_node(test_id)
            .unwrap()
            .output_pins()
            .next()
            .map(|p| p.name.clone());

        if let Some(vec2_pin) = first_vec2_input {
            graph.add_connection(
                PinId::output(uv_id, "uv"),
                PinId::input(test_id, vec2_pin),
            );
        }

        let Some(output_pin) = first_output_name else {
            continue;
        };

        let test_node_ref = graph.get_node(test_id).unwrap();
        let result = preview_eval::evaluate_node_output(&graph, test_node_ref, &output_pin);

        if result.is_none() {
            unhandled.push(type_id.clone());
        }
    }

    // Separate into known gaps vs new regressions
    let new_gaps: Vec<_> = unhandled
        .iter()
        .filter(|id| !PREVIEW_EVAL_KNOWN_GAPS.contains(&id.as_str()))
        .collect();

    let fixed_gaps: Vec<_> = PREVIEW_EVAL_KNOWN_GAPS
        .iter()
        .filter(|id| !unhandled.iter().any(|u| u == *id))
        .collect();

    if !fixed_gaps.is_empty() {
        // A known gap has been fixed — remind to remove it from the allowlist
        panic!(
            "These nodes now have preview eval handlers — remove them from PREVIEW_EVAL_KNOWN_GAPS:\n  {}",
            fixed_gaps.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("\n  ")
        );
    }

    if !new_gaps.is_empty() {
        panic!(
            "Preview evaluator returned None for {} NEW node(s) (not in known gaps list):\n  {}\n\n\
             If these are intentionally unhandled, add them to PREVIEW_EVAL_KNOWN_GAPS in tests.rs",
            new_gaps.len(),
            new_gaps.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("\n  ")
        );
    }
}

// =============================================================================
// Test 3: Codegen doesn't produce division by zero from fallback defaults
// =============================================================================

#[test]
fn codegen_output_no_division_by_zero() {
    let defs = all_shader_node_defs();
    let mut failures = Vec::new();

    for (type_id, def) in &defs {
        // Only test nodes that have Float inputs (potential division operands)
        let pins = (def.create_pins)();
        let has_float_input = pins
            .iter()
            .any(|p| p.direction == PinDirection::Input && p.pin_type == PinType::Float);
        if !has_float_input {
            continue;
        }

        let (graph, _test_id) = build_shader_node_graph(def);
        let result = generate_wgsl_code(&graph);

        if result.errors.is_empty() {
            // Check for literal division by zero patterns
            if result.fragment_shader.contains("/ 0.0")
                || result.fragment_shader.contains("/ 0.000000")
            {
                failures.push(format!(
                    "{}: generated WGSL contains division by zero literal",
                    type_id
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Division-by-zero detected in {} node(s):\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}

// =============================================================================
// Test 4: Node default values resolve correctly for all pins
// =============================================================================

#[test]
fn node_default_values_resolve() {
    let defs = all_shader_node_defs();
    let mut failures = Vec::new();

    for (type_id, def) in &defs {
        let node = def.create_node(NodeId::new(1));

        for pin in node.input_pins() {
            if pin.default_value.is_some() {
                let value = node.get_input_value(&pin.name);
                if value.is_none() {
                    failures.push(format!(
                        "{}: pin '{}' has default_value in definition but get_input_value returned None",
                        type_id, pin.name
                    ));
                }
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Default value resolution failed for {} pin(s):\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}

// =============================================================================
// Test 5: Codegen uses correct pin names (non-default float values appear in output)
// =============================================================================

#[test]
fn codegen_uses_correct_pin_names() {
    let defs = all_shader_node_defs();
    let mut failures = Vec::new();

    for (type_id, def) in &defs {
        let pins = (def.create_pins)();

        // Collect non-trivial float default values from pins
        // "Non-trivial" means not 0.0 and not 1.0 (since those are too common in WGSL)
        let distinctive_float_pins: Vec<(String, f32)> = pins
            .iter()
            .filter(|p| p.direction == PinDirection::Input)
            .filter_map(|p| {
                if let Some(PinValue::Float(v)) = &p.default_value {
                    let v = *v;
                    // Skip values that are too generic to search for
                    if v != 0.0 && v != 1.0 && v != -1.0 {
                        return Some((p.name.clone(), v));
                    }
                }
                None
            })
            .collect();

        if distinctive_float_pins.is_empty() {
            continue;
        }

        let (graph, test_id) = build_shader_node_graph(def);

        // Check that the test node's output is actually connected to PBR
        let has_connected_output = graph
            .get_node(test_id)
            .unwrap()
            .output_pins()
            .any(|p| graph.is_pin_connected(&PinId::output(test_id, &p.name)));

        if !has_connected_output {
            // Node output type not connectable to PBR - skip
            continue;
        }

        let result = generate_wgsl_code(&graph);
        if !result.errors.is_empty() {
            continue; // codegen error, already caught by test 1
        }

        for (pin_name, value) in &distinctive_float_pins {
            // Format the value the same way WGSL codegen does
            let formatted = format!("{:.6}", value);
            if !result.fragment_shader.contains(&formatted) {
                failures.push(format!(
                    "{}: pin '{}' default value {} (formatted: {}) not found in generated WGSL \
                     (codegen may be reading a wrong pin name)",
                    type_id, pin_name, value, formatted
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Pin name mismatch detected for {} pin(s):\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}
