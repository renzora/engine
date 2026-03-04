//! Shader input nodes (UV, Time, World Position, etc.)

use crate::blueprint::{BlueprintGraph, BlueprintNode, PinValue};
use crate::blueprint::preview::UV_OVERRIDE;

/// Evaluate input nodes - return sensible default values for preview
pub fn evaluate(
    _graph: &BlueprintGraph,
    node: &BlueprintNode,
    output_pin: &str,
) -> Option<PinValue> {
    match node.node_type.as_str() {
        // UV coordinates - check for override (used during procedural texture generation)
        "shader/uv" => {
            let uv = UV_OVERRIDE.with(|cell| {
                cell.borrow().unwrap_or([0.5, 0.5])
            });
            match output_pin {
                "uv" => Some(PinValue::Vec2(uv)),
                "u" => Some(PinValue::Float(uv[0])),
                "v" => Some(PinValue::Float(uv[1])),
                _ => Some(PinValue::Vec2(uv)),
            }
        }

        // Time values
        "shader/time" => {
            match output_pin {
                "time" => Some(PinValue::Float(0.0)),
                "sin_time" => Some(PinValue::Float(0.0)),
                "cos_time" => Some(PinValue::Float(1.0)),
                _ => Some(PinValue::Float(0.0)),
            }
        }

        // World position - use UV as proxy for XZ during texture generation
        // This allows procedural patterns using World Position to generate correctly
        "shader/world_position" => {
            let uv = UV_OVERRIDE.with(|cell| {
                cell.borrow().unwrap_or([0.5, 0.5])
            });
            // Map UV (0-1) to world position range for preview
            // Using a reasonable range that shows the pattern well
            let world_x = uv[0] * 10.0; // 10 world units across texture
            let world_z = uv[1] * 10.0;
            match output_pin {
                "position" => Some(PinValue::Vec3([world_x, 0.0, world_z])),
                "x" => Some(PinValue::Float(world_x)),
                "y" => Some(PinValue::Float(0.0)),
                "z" => Some(PinValue::Float(world_z)),
                _ => Some(PinValue::Vec3([world_x, 0.0, world_z])),
            }
        }

        // World normal
        "shader/world_normal" => {
            match output_pin {
                "normal" => Some(PinValue::Vec3([0.0, 1.0, 0.0])),
                "x" => Some(PinValue::Float(0.0)),
                "y" => Some(PinValue::Float(1.0)),
                "z" => Some(PinValue::Float(0.0)),
                _ => Some(PinValue::Vec3([0.0, 1.0, 0.0])),
            }
        }

        // View direction
        "shader/view_direction" => {
            Some(PinValue::Vec3([0.0, 0.0, 1.0]))
        }

        // Vertex color
        "shader/vertex_color" => {
            match output_pin {
                "color" => Some(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
                "r" => Some(PinValue::Float(1.0)),
                "g" => Some(PinValue::Float(1.0)),
                "b" => Some(PinValue::Float(1.0)),
                "a" => Some(PinValue::Float(1.0)),
                _ => Some(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
            }
        }

        _ => None,
    }
}
