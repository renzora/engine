//! UV manipulation nodes (Tiling, Offset, Rotate, etc.)

use crate::blueprint::{BlueprintGraph, BlueprintNode, PinValue};
use super::{get_pin_value, as_float, as_vec2, as_vec3};

/// Evaluate UV manipulation nodes
pub fn evaluate(
    graph: &BlueprintGraph,
    node: &BlueprintNode,
    output_pin: &str,
) -> Option<PinValue> {
    match node.node_type.as_str() {
        // UV Tiling
        "shader/uv_tiling" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let tile_x = get_pin_value(graph, node, "tile_x").and_then(as_float).unwrap_or(1.0);
            let tile_y = get_pin_value(graph, node, "tile_y").and_then(as_float).unwrap_or(1.0);

            Some(PinValue::Vec2([
                (uv[0] * tile_x).fract(),
                (uv[1] * tile_y).fract(),
            ]))
        }

        // UV Offset
        "shader/uv_offset" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let offset_x = get_pin_value(graph, node, "offset_x").and_then(as_float).unwrap_or(0.0);
            let offset_y = get_pin_value(graph, node, "offset_y").and_then(as_float).unwrap_or(0.0);

            Some(PinValue::Vec2([
                uv[0] + offset_x,
                uv[1] + offset_y,
            ]))
        }

        // UV Rotate
        "shader/uv_rotate" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let angle = get_pin_value(graph, node, "angle").and_then(as_float).unwrap_or(0.0);
            let center_x = get_pin_value(graph, node, "center_x").and_then(as_float).unwrap_or(0.5);
            let center_y = get_pin_value(graph, node, "center_y").and_then(as_float).unwrap_or(0.5);

            // Translate to center, rotate, translate back
            let dx = uv[0] - center_x;
            let dy = uv[1] - center_y;

            let cos_a = angle.cos();
            let sin_a = angle.sin();

            Some(PinValue::Vec2([
                dx * cos_a - dy * sin_a + center_x,
                dx * sin_a + dy * cos_a + center_y,
            ]))
        }

        // UV Flipbook (sprite sheet animation)
        "shader/uv_flipbook" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let columns = get_pin_value(graph, node, "columns").and_then(as_float).unwrap_or(4.0).max(1.0);
            let rows = get_pin_value(graph, node, "rows").and_then(as_float).unwrap_or(4.0).max(1.0);
            let frame = get_pin_value(graph, node, "frame").and_then(as_float).unwrap_or(0.0);

            let total_frames = columns * rows;
            let frame_index = (frame % total_frames) as i32;

            let col = (frame_index as f32) % columns;
            let row = (frame_index as f32 / columns).floor();

            let cell_width = 1.0 / columns;
            let cell_height = 1.0 / rows;

            Some(PinValue::Vec2([
                uv[0] * cell_width + col * cell_width,
                uv[1] * cell_height + row * cell_height,
            ]))
        }

        // Triplanar Mapping
        "shader/triplanar" => {
            let position = get_pin_value(graph, node, "position").and_then(as_vec3).unwrap_or([0.0, 0.0, 0.0]);
            let normal = get_pin_value(graph, node, "normal").and_then(as_vec3).unwrap_or([0.0, 1.0, 0.0]);
            let scale = get_pin_value(graph, node, "scale").and_then(as_float).unwrap_or(1.0);
            let blend = get_pin_value(graph, node, "blend").and_then(as_float).unwrap_or(1.0);

            // Calculate blend weights based on normal
            let abs_normal = [normal[0].abs(), normal[1].abs(), normal[2].abs()];
            let blend_weights = if blend > 0.0 {
                let sum = abs_normal[0].powf(blend) + abs_normal[1].powf(blend) + abs_normal[2].powf(blend);
                if sum > 0.0 {
                    [
                        abs_normal[0].powf(blend) / sum,
                        abs_normal[1].powf(blend) / sum,
                        abs_normal[2].powf(blend) / sum,
                    ]
                } else {
                    [0.333, 0.333, 0.333]
                }
            } else {
                [abs_normal[0], abs_normal[1], abs_normal[2]]
            };

            // UV projections
            let uv_x = [position[1] * scale, position[2] * scale];
            let uv_y = [position[0] * scale, position[2] * scale];
            let uv_z = [position[0] * scale, position[1] * scale];

            match output_pin {
                "uv_x" => Some(PinValue::Vec2(uv_x)),
                "uv_y" => Some(PinValue::Vec2(uv_y)),
                "uv_z" => Some(PinValue::Vec2(uv_z)),
                "weights" => Some(PinValue::Vec3(blend_weights)),
                _ => Some(PinValue::Vec2(uv_y)), // Default to Y projection
            }
        }

        _ => None,
    }
}
