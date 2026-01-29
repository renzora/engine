//! Color manipulation nodes (Lerp, Saturation, Brightness, etc.)

use crate::blueprint::{BlueprintGraph, BlueprintNode, PinValue};
use super::{get_pin_value, as_float, as_color, as_vec3};

/// Evaluate color manipulation nodes
pub fn evaluate(
    graph: &BlueprintGraph,
    node: &BlueprintNode,
    output_pin: &str,
) -> Option<PinValue> {
    match node.node_type.as_str() {
        // Lerp Color
        "shader/lerp_color" => {
            let a = get_pin_value(graph, node, "a").and_then(as_color).unwrap_or([0.0, 0.0, 0.0, 1.0]);
            let b = get_pin_value(graph, node, "b").and_then(as_color).unwrap_or([1.0, 1.0, 1.0, 1.0]);
            let t = get_pin_value(graph, node, "t").and_then(as_float).unwrap_or(0.5);
            Some(PinValue::Color([
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
                a[3] + (b[3] - a[3]) * t,
            ]))
        }

        // Hue Shift
        "shader/hue_shift" => {
            let color = get_pin_value(graph, node, "color").and_then(as_color).unwrap_or([1.0, 1.0, 1.0, 1.0]);
            let shift = get_pin_value(graph, node, "shift").and_then(as_float).unwrap_or(0.0);

            // Convert to HSV, shift hue, convert back
            let (h, s, v) = rgb_to_hsv(color[0], color[1], color[2]);
            let new_h = (h + shift).fract();
            let new_h = if new_h < 0.0 { new_h + 1.0 } else { new_h };
            let (r, g, b) = hsv_to_rgb(new_h, s, v);

            Some(PinValue::Color([r, g, b, color[3]]))
        }

        // Saturation
        "shader/saturation" => {
            let color = get_pin_value(graph, node, "color").and_then(as_color).unwrap_or([1.0, 1.0, 1.0, 1.0]);
            let amount = get_pin_value(graph, node, "amount").and_then(as_float).unwrap_or(1.0);
            let lum = luminance(color[0], color[1], color[2]);
            Some(PinValue::Color([
                lum + (color[0] - lum) * amount,
                lum + (color[1] - lum) * amount,
                lum + (color[2] - lum) * amount,
                color[3],
            ]))
        }

        // Brightness
        "shader/brightness" => {
            let color = get_pin_value(graph, node, "color").and_then(as_color).unwrap_or([1.0, 1.0, 1.0, 1.0]);
            let amount = get_pin_value(graph, node, "amount").and_then(as_float).unwrap_or(0.0);
            Some(PinValue::Color([
                (color[0] + amount).clamp(0.0, 1.0),
                (color[1] + amount).clamp(0.0, 1.0),
                (color[2] + amount).clamp(0.0, 1.0),
                color[3],
            ]))
        }

        // Contrast
        "shader/contrast" => {
            let color = get_pin_value(graph, node, "color").and_then(as_color).unwrap_or([1.0, 1.0, 1.0, 1.0]);
            let amount = get_pin_value(graph, node, "amount").and_then(as_float).unwrap_or(1.0);
            Some(PinValue::Color([
                ((color[0] - 0.5) * amount + 0.5).clamp(0.0, 1.0),
                ((color[1] - 0.5) * amount + 0.5).clamp(0.0, 1.0),
                ((color[2] - 0.5) * amount + 0.5).clamp(0.0, 1.0),
                color[3],
            ]))
        }

        // Invert Color
        "shader/invert_color" => {
            let color = get_pin_value(graph, node, "color").and_then(as_color).unwrap_or([1.0, 1.0, 1.0, 1.0]);
            Some(PinValue::Color([
                1.0 - color[0],
                1.0 - color[1],
                1.0 - color[2],
                color[3],
            ]))
        }

        // Desaturate
        "shader/desaturate" => {
            let color = get_pin_value(graph, node, "color").and_then(as_color).unwrap_or([1.0, 1.0, 1.0, 1.0]);
            let amount = get_pin_value(graph, node, "amount").and_then(as_float).unwrap_or(1.0);
            let lum = luminance(color[0], color[1], color[2]);

            match output_pin {
                "luminance" => Some(PinValue::Float(lum)),
                _ => Some(PinValue::Color([
                    color[0] + (lum - color[0]) * amount,
                    color[1] + (lum - color[1]) * amount,
                    color[2] + (lum - color[2]) * amount,
                    color[3],
                ])),
            }
        }

        // RGB to HSV
        "shader/rgb_to_hsv" => {
            let rgb = get_pin_value(graph, node, "rgb").and_then(as_vec3).unwrap_or([1.0, 1.0, 1.0]);
            let (h, s, v) = rgb_to_hsv(rgb[0], rgb[1], rgb[2]);

            match output_pin {
                "hsv" => Some(PinValue::Vec3([h, s, v])),
                "h" => Some(PinValue::Float(h)),
                "s" => Some(PinValue::Float(s)),
                "v" => Some(PinValue::Float(v)),
                _ => Some(PinValue::Vec3([h, s, v])),
            }
        }

        // HSV to RGB
        "shader/hsv_to_rgb" => {
            let h = get_pin_value(graph, node, "h").and_then(as_float).unwrap_or(0.0);
            let s = get_pin_value(graph, node, "s").and_then(as_float).unwrap_or(1.0);
            let v = get_pin_value(graph, node, "v").and_then(as_float).unwrap_or(1.0);
            let (r, g, b) = hsv_to_rgb(h, s, v);
            Some(PinValue::Vec3([r, g, b]))
        }

        _ => None,
    }
}

/// Calculate luminance using standard weights
fn luminance(r: f32, g: f32, b: f32) -> f32 {
    r * 0.2126 + g * 0.7152 + b * 0.0722
}

/// Convert RGB to HSV
fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let v = max;
    let s = if max > 0.0 { delta / max } else { 0.0 };

    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };

    (h, s, v)
}

/// Convert HSV to RGB
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let h6 = h * 6.0;
    let x = c * (1.0 - ((h6 % 2.0) - 1.0).abs());
    let m = v - c;

    let (r, g, b) = match h6 as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    (r + m, g + m, b + m)
}
