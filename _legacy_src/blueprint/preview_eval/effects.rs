//! Effect nodes (Rim Light, Parallax, Normal Blend, etc.)

use crate::blueprint::{BlueprintGraph, BlueprintNode, PinValue};
use super::{get_pin_value, as_float, as_vec2, as_vec3};

/// Evaluate effect nodes
pub fn evaluate(
    graph: &BlueprintGraph,
    node: &BlueprintNode,
    _output_pin: &str,
) -> Option<PinValue> {
    match node.node_type.as_str() {
        // Rim Light
        "shader/rim_light" => {
            let normal = get_pin_value(graph, node, "normal").and_then(as_vec3).unwrap_or([0.0, 0.0, 1.0]);
            let view_dir = get_pin_value(graph, node, "view_dir").and_then(as_vec3).unwrap_or([0.0, 0.0, 1.0]);
            let power = get_pin_value(graph, node, "power").and_then(as_float).unwrap_or(2.0);
            let intensity = get_pin_value(graph, node, "intensity").and_then(as_float).unwrap_or(1.0);

            // Normalize vectors
            let n_len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
            let v_len = (view_dir[0] * view_dir[0] + view_dir[1] * view_dir[1] + view_dir[2] * view_dir[2]).sqrt();

            if n_len > 0.0 && v_len > 0.0 {
                let n = [normal[0] / n_len, normal[1] / n_len, normal[2] / n_len];
                let v = [view_dir[0] / v_len, view_dir[1] / v_len, view_dir[2] / v_len];

                let dot = n[0] * v[0] + n[1] * v[1] + n[2] * v[2];
                let rim = (1.0 - dot.abs()).powf(power) * intensity;
                Some(PinValue::Float(rim))
            } else {
                Some(PinValue::Float(0.0))
            }
        }

        // Parallax Mapping
        "shader/parallax" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let height = get_pin_value(graph, node, "height").and_then(as_float).unwrap_or(0.5);
            let view_dir = get_pin_value(graph, node, "view_dir").and_then(as_vec3).unwrap_or([0.0, 0.0, 1.0]);
            let scale = get_pin_value(graph, node, "scale").and_then(as_float).unwrap_or(0.05);

            // Simple parallax offset
            let v_len = (view_dir[0] * view_dir[0] + view_dir[1] * view_dir[1] + view_dir[2] * view_dir[2]).sqrt();
            if v_len > 0.0 && view_dir[2].abs() > 0.001 {
                let offset = (height - 0.5) * scale;
                let ratio_x = view_dir[0] / view_dir[2];
                let ratio_y = view_dir[1] / view_dir[2];

                Some(PinValue::Vec2([
                    uv[0] - ratio_x * offset,
                    uv[1] - ratio_y * offset,
                ]))
            } else {
                Some(PinValue::Vec2(uv))
            }
        }

        // Normal Blend (Reoriented Normal Mapping)
        "shader/normal_blend" => {
            let base = get_pin_value(graph, node, "base").and_then(as_vec3).unwrap_or([0.0, 0.0, 1.0]);
            let detail = get_pin_value(graph, node, "detail").and_then(as_vec3).unwrap_or([0.0, 0.0, 1.0]);

            // Reoriented Normal Mapping blend
            // This is a simplified version that works well for most cases
            let t = [base[0], base[1], base[2] + 1.0];
            let u = [-detail[0], -detail[1], detail[2]];

            let result = [
                t[0] * u[2] + u[0] * t[2],
                t[1] * u[2] + u[1] * t[2],
                t[2] * u[2] - t[0] * u[0] - t[1] * u[1],
            ];

            // Normalize
            let len = (result[0] * result[0] + result[1] * result[1] + result[2] * result[2]).sqrt();
            if len > 0.0 {
                Some(PinValue::Vec3([result[0] / len, result[1] / len, result[2] / len]))
            } else {
                Some(PinValue::Vec3([0.0, 0.0, 1.0]))
            }
        }

        // Detail Blend (overlay mode)
        "shader/detail_blend" => {
            let base = get_pin_value(graph, node, "base").and_then(as_vec3).unwrap_or([0.5, 0.5, 0.5]);
            let detail = get_pin_value(graph, node, "detail").and_then(as_vec3).unwrap_or([0.5, 0.5, 0.5]);
            let amount = get_pin_value(graph, node, "amount").and_then(as_float).unwrap_or(1.0);

            // Overlay blend mode
            let overlay = |b: f32, d: f32| -> f32 {
                if b < 0.5 {
                    2.0 * b * d
                } else {
                    1.0 - 2.0 * (1.0 - b) * (1.0 - d)
                }
            };

            let blended = [
                overlay(base[0], detail[0]),
                overlay(base[1], detail[1]),
                overlay(base[2], detail[2]),
            ];

            // Lerp between base and blended based on amount
            Some(PinValue::Vec3([
                base[0] + (blended[0] - base[0]) * amount,
                base[1] + (blended[1] - base[1]) * amount,
                base[2] + (blended[2] - base[2]) * amount,
            ]))
        }

        // Posterize
        "shader/posterize" => {
            let color = get_pin_value(graph, node, "color").and_then(as_vec3).unwrap_or([1.0, 1.0, 1.0]);
            let levels = get_pin_value(graph, node, "levels").and_then(as_float).unwrap_or(4.0).max(1.0);

            Some(PinValue::Vec3([
                (color[0] * levels).floor() / levels,
                (color[1] * levels).floor() / levels,
                (color[2] * levels).floor() / levels,
            ]))
        }

        _ => None,
    }
}
