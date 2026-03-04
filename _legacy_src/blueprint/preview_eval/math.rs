//! Shader math nodes (Dot, Cross, Pow, Step, etc.)

use crate::blueprint::{BlueprintGraph, BlueprintNode, PinValue};
use super::{get_pin_value, as_float, as_vec3};

/// Evaluate math nodes
pub fn evaluate(
    graph: &BlueprintGraph,
    node: &BlueprintNode,
    _output_pin: &str,
) -> Option<PinValue> {
    match node.node_type.as_str() {
        // Dot product
        "shader/dot" => {
            let a = get_pin_value(graph, node, "a").and_then(as_vec3).unwrap_or([0.0, 0.0, 0.0]);
            let b = get_pin_value(graph, node, "b").and_then(as_vec3).unwrap_or([0.0, 0.0, 0.0]);
            let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            Some(PinValue::Float(dot))
        }

        // Cross product
        "shader/cross" => {
            let a = get_pin_value(graph, node, "a").and_then(as_vec3).unwrap_or([1.0, 0.0, 0.0]);
            let b = get_pin_value(graph, node, "b").and_then(as_vec3).unwrap_or([0.0, 1.0, 0.0]);
            let cross = [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ];
            Some(PinValue::Vec3(cross))
        }

        // Normalize
        "shader/normalize" => {
            let v = get_pin_value(graph, node, "v").and_then(as_vec3).unwrap_or([1.0, 0.0, 0.0]);
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if len > 0.0 {
                Some(PinValue::Vec3([v[0] / len, v[1] / len, v[2] / len]))
            } else {
                Some(PinValue::Vec3([0.0, 0.0, 0.0]))
            }
        }

        // Length
        "shader/length" => {
            let v = get_pin_value(graph, node, "v").and_then(as_vec3).unwrap_or([1.0, 0.0, 0.0]);
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            Some(PinValue::Float(len))
        }

        // Distance
        "shader/distance" => {
            let a = get_pin_value(graph, node, "a").and_then(as_vec3).unwrap_or([0.0, 0.0, 0.0]);
            let b = get_pin_value(graph, node, "b").and_then(as_vec3).unwrap_or([0.0, 0.0, 0.0]);
            let dx = b[0] - a[0];
            let dy = b[1] - a[1];
            let dz = b[2] - a[2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            Some(PinValue::Float(dist))
        }

        // Reflect
        "shader/reflect" => {
            let i = get_pin_value(graph, node, "incident").and_then(as_vec3).unwrap_or([0.0, -1.0, 0.0]);
            let n = get_pin_value(graph, node, "normal").and_then(as_vec3).unwrap_or([0.0, 1.0, 0.0]);
            let dot = i[0] * n[0] + i[1] * n[1] + i[2] * n[2];
            let reflect = [
                i[0] - 2.0 * dot * n[0],
                i[1] - 2.0 * dot * n[1],
                i[2] - 2.0 * dot * n[2],
            ];
            Some(PinValue::Vec3(reflect))
        }

        // Fresnel
        "shader/fresnel" => {
            let normal = get_pin_value(graph, node, "normal").and_then(as_vec3).unwrap_or([0.0, 0.0, 1.0]);
            let view = get_pin_value(graph, node, "view").and_then(as_vec3).unwrap_or([0.0, 0.0, 1.0]);
            let power = get_pin_value(graph, node, "power").and_then(as_float).unwrap_or(5.0);

            let dot = normal[0] * view[0] + normal[1] * view[1] + normal[2] * view[2];
            let fresnel = (1.0 - dot.abs()).powf(power);
            Some(PinValue::Float(fresnel))
        }

        // Power
        "shader/pow" => {
            let base = get_pin_value(graph, node, "base").and_then(as_float).unwrap_or(2.0);
            let exp = get_pin_value(graph, node, "exp").and_then(as_float).unwrap_or(2.0);
            Some(PinValue::Float(base.powf(exp)))
        }

        // Smoothstep
        "shader/smoothstep" => {
            let edge0 = get_pin_value(graph, node, "edge0").and_then(as_float).unwrap_or(0.0);
            let edge1 = get_pin_value(graph, node, "edge1").and_then(as_float).unwrap_or(1.0);
            let x = get_pin_value(graph, node, "x").and_then(as_float).unwrap_or(0.5);
            let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
            Some(PinValue::Float(t * t * (3.0 - 2.0 * t)))
        }

        // Step
        "shader/step" => {
            let edge = get_pin_value(graph, node, "edge").and_then(as_float).unwrap_or(0.5);
            let x = get_pin_value(graph, node, "x").and_then(as_float).unwrap_or(0.0);
            Some(PinValue::Float(if x < edge { 0.0 } else { 1.0 }))
        }

        // Fract
        "shader/fract" => {
            let x = get_pin_value(graph, node, "x").and_then(as_float).unwrap_or(0.0);
            Some(PinValue::Float(x.fract()))
        }

        // Floor
        "shader/floor" => {
            let x = get_pin_value(graph, node, "x").and_then(as_float).unwrap_or(0.0);
            Some(PinValue::Float(x.floor()))
        }

        // Ceil
        "shader/ceil" => {
            let x = get_pin_value(graph, node, "x").and_then(as_float).unwrap_or(0.0);
            Some(PinValue::Float(x.ceil()))
        }

        // One minus
        "shader/one_minus" => {
            let x = get_pin_value(graph, node, "x").and_then(as_float).unwrap_or(0.0);
            Some(PinValue::Float(1.0 - x))
        }

        // Saturate (clamp 0-1)
        "shader/saturate" => {
            let x = get_pin_value(graph, node, "x").and_then(as_float).unwrap_or(0.0);
            Some(PinValue::Float(x.clamp(0.0, 1.0)))
        }

        // Lerp Vec3
        "shader/lerp_vec3" => {
            let a = get_pin_value(graph, node, "a").and_then(as_vec3).unwrap_or([0.0, 0.0, 0.0]);
            let b = get_pin_value(graph, node, "b").and_then(as_vec3).unwrap_or([1.0, 1.0, 1.0]);
            let t = get_pin_value(graph, node, "t").and_then(as_float).unwrap_or(0.5);
            Some(PinValue::Vec3([
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ]))
        }

        _ => None,
    }
}
