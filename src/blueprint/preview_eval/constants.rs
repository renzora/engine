//! Constant value nodes (Color, Float)

use crate::blueprint::{BlueprintGraph, BlueprintNode, PinValue};

/// Evaluate constant nodes
pub fn evaluate(
    _graph: &BlueprintGraph,
    node: &BlueprintNode,
    output_pin: &str,
) -> Option<PinValue> {
    match node.node_type.as_str() {
        // Color constant
        "shader/color" | "shader/color_constant" => {
            let color = node.get_input_value("color")
                .and_then(super::as_color)
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);

            match output_pin {
                "color" => Some(PinValue::Color(color)),
                "rgb" => Some(PinValue::Vec3([color[0], color[1], color[2]])),
                _ => Some(PinValue::Color(color)),
            }
        }

        // Float constant
        "shader/float" | "shader/float_constant" => {
            node.get_input_value("value")
        }

        _ => None,
    }
}
