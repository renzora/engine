//! Vector construction and decomposition nodes

use crate::blueprint::{BlueprintGraph, BlueprintNode, PinValue};
use super::{get_pin_value, as_float, as_vec2, as_vec3, as_color};

/// Evaluate vector nodes
pub fn evaluate(
    graph: &BlueprintGraph,
    node: &BlueprintNode,
    output_pin: &str,
) -> Option<PinValue> {
    match node.node_type.as_str() {
        // Make Vec2
        "shader/make_vec2" => {
            let x = get_pin_value(graph, node, "x").and_then(as_float).unwrap_or(0.0);
            let y = get_pin_value(graph, node, "y").and_then(as_float).unwrap_or(0.0);
            Some(PinValue::Vec2([x, y]))
        }

        // Make Vec3
        "shader/make_vec3" => {
            let x = get_pin_value(graph, node, "x").and_then(as_float).unwrap_or(0.0);
            let y = get_pin_value(graph, node, "y").and_then(as_float).unwrap_or(0.0);
            let z = get_pin_value(graph, node, "z").and_then(as_float).unwrap_or(0.0);
            Some(PinValue::Vec3([x, y, z]))
        }

        // Make Vec4
        "shader/make_vec4" => {
            let x = get_pin_value(graph, node, "x").and_then(as_float).unwrap_or(0.0);
            let y = get_pin_value(graph, node, "y").and_then(as_float).unwrap_or(0.0);
            let z = get_pin_value(graph, node, "z").and_then(as_float).unwrap_or(0.0);
            let w = get_pin_value(graph, node, "w").and_then(as_float).unwrap_or(1.0);
            Some(PinValue::Vec4([x, y, z, w]))
        }

        // Make Color
        "shader/make_color" => {
            let r = get_pin_value(graph, node, "r").and_then(as_float).unwrap_or(1.0);
            let g = get_pin_value(graph, node, "g").and_then(as_float).unwrap_or(1.0);
            let b = get_pin_value(graph, node, "b").and_then(as_float).unwrap_or(1.0);
            let a = get_pin_value(graph, node, "a").and_then(as_float).unwrap_or(1.0);
            Some(PinValue::Color([r, g, b, a]))
        }

        // Split Vec2
        "shader/split_vec2" => {
            let v = get_pin_value(graph, node, "v").and_then(as_vec2).unwrap_or([0.0, 0.0]);
            match output_pin {
                "x" => Some(PinValue::Float(v[0])),
                "y" => Some(PinValue::Float(v[1])),
                _ => Some(PinValue::Float(v[0])),
            }
        }

        // Split Vec3
        "shader/split_vec3" => {
            let v = get_pin_value(graph, node, "v").and_then(as_vec3).unwrap_or([0.0, 0.0, 0.0]);
            match output_pin {
                "x" => Some(PinValue::Float(v[0])),
                "y" => Some(PinValue::Float(v[1])),
                "z" => Some(PinValue::Float(v[2])),
                _ => Some(PinValue::Float(v[0])),
            }
        }

        // Split Color
        "shader/split_color" => {
            let c = get_pin_value(graph, node, "color").and_then(as_color).unwrap_or([1.0, 1.0, 1.0, 1.0]);
            match output_pin {
                "r" => Some(PinValue::Float(c[0])),
                "g" => Some(PinValue::Float(c[1])),
                "b" => Some(PinValue::Float(c[2])),
                "a" => Some(PinValue::Float(c[3])),
                _ => Some(PinValue::Float(c[0])),
            }
        }

        _ => None,
    }
}
