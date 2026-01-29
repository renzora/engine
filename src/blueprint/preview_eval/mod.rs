//! Node evaluation for material preview
//!
//! Handles computing node output values for the material preview system.
//! Organized into submodules by node category.

mod constants;
mod math;
mod vector;
mod color;
mod noise;
mod uv;
mod effects;
mod input;

use crate::blueprint::{BlueprintGraph, BlueprintNode, PinValue};
use crate::core::resources::console::{console_log, LogLevel};

/// Helper to extract a float from a PinValue
pub fn as_float(v: PinValue) -> Option<f32> {
    if let PinValue::Float(f) = v { Some(f) } else { None }
}

/// Helper to extract a color from a PinValue
pub fn as_color(v: PinValue) -> Option<[f32; 4]> {
    if let PinValue::Color(c) = v { Some(c) } else { None }
}

/// Helper to extract a vec2 from a PinValue
pub fn as_vec2(v: PinValue) -> Option<[f32; 2]> {
    if let PinValue::Vec2(v) = v { Some(v) } else { None }
}

/// Helper to extract a vec3 from a PinValue
pub fn as_vec3(v: PinValue) -> Option<[f32; 3]> {
    if let PinValue::Vec3(v) = v { Some(v) } else { None }
}

/// Helper to extract a vec4 from a PinValue
pub fn as_vec4(v: PinValue) -> Option<[f32; 4]> {
    if let PinValue::Vec4(v) = v { Some(v) } else { None }
}

/// Get the value for a pin, following connections if needed
pub fn get_pin_value(
    graph: &BlueprintGraph,
    node: &BlueprintNode,
    pin_name: &str,
) -> Option<PinValue> {
    // Check for connection to this pin - connections take priority
    let connection = graph.connections.iter().find(|c| {
        c.to.node_id == node.id && c.to.pin_name == pin_name
    });

    if let Some(conn) = connection {
        let Some(source_node) = graph.nodes.iter().find(|n| n.id == conn.from.node_id) else {
            console_log(LogLevel::Error, "Preview", format!("Source node {:?} not found!", conn.from.node_id));
            return None;
        };

        return evaluate_node_output(graph, source_node, &conn.from.pin_name);
    }

    // No connection - check for direct value on the node
    node.get_input_value(pin_name)
}

/// Evaluate a node's output value for preview purposes
pub fn evaluate_node_output(
    graph: &BlueprintGraph,
    node: &BlueprintNode,
    output_pin: &str,
) -> Option<PinValue> {
    let node_type = node.node_type.as_str();

    // Texture nodes - return the path
    if crate::blueprint::canvas::is_texture_node(node_type) {
        return node.get_input_value("path");
    }

    // Try each category handler
    if let Some(value) = constants::evaluate(graph, node, output_pin) {
        return Some(value);
    }
    if let Some(value) = input::evaluate(graph, node, output_pin) {
        return Some(value);
    }
    if let Some(value) = math::evaluate(graph, node, output_pin) {
        return Some(value);
    }
    if let Some(value) = vector::evaluate(graph, node, output_pin) {
        return Some(value);
    }
    if let Some(value) = color::evaluate(graph, node, output_pin) {
        return Some(value);
    }
    if let Some(value) = noise::evaluate(graph, node, output_pin) {
        return Some(value);
    }
    if let Some(value) = uv::evaluate(graph, node, output_pin) {
        return Some(value);
    }
    if let Some(value) = effects::evaluate(graph, node, output_pin) {
        return Some(value);
    }

    // Unknown node type
    console_log(LogLevel::Warning, "Preview", format!("Unhandled node type: {}", node_type));
    None
}
