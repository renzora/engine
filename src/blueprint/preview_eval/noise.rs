//! Procedural noise and pattern nodes

use crate::blueprint::{BlueprintGraph, BlueprintNode, PinValue};
use super::{get_pin_value, as_float, as_vec2};

/// Evaluate noise/procedural nodes
/// Note: For preview purposes, these return representative average values
/// since we can't compute the actual procedural patterns without UV coordinates
pub fn evaluate(
    graph: &BlueprintGraph,
    node: &BlueprintNode,
    output_pin: &str,
) -> Option<PinValue> {
    match node.node_type.as_str() {
        // Checkerboard pattern
        "shader/checkerboard" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let scale = get_pin_value(graph, node, "scale").and_then(as_float).unwrap_or(2.0);

            // Actually compute checkerboard value at the UV coordinate
            let x = (uv[0] * scale).floor() as i32;
            let y = (uv[1] * scale).floor() as i32;
            let checker = ((x + y) % 2).abs() as f32;
            Some(PinValue::Float(checker))
        }

        // Simple noise
        "shader/noise_simple" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let scale = get_pin_value(graph, node, "scale").and_then(as_float).unwrap_or(10.0);

            // Simple hash-based noise approximation
            let noise = simple_noise(uv[0] * scale, uv[1] * scale);
            Some(PinValue::Float(noise))
        }

        // Gradient noise (Perlin-like)
        "shader/noise_gradient" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let scale = get_pin_value(graph, node, "scale").and_then(as_float).unwrap_or(10.0);

            // Gradient noise approximation
            let noise = gradient_noise(uv[0] * scale, uv[1] * scale);
            Some(PinValue::Float(noise))
        }

        // Voronoi/Cellular noise
        "shader/noise_voronoi" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let scale = get_pin_value(graph, node, "scale").and_then(as_float).unwrap_or(5.0);

            let (distance, cell) = voronoi_noise(uv[0] * scale, uv[1] * scale);

            match output_pin {
                "distance" => Some(PinValue::Float(distance)),
                "cell" => Some(PinValue::Float(cell)),
                _ => Some(PinValue::Float(distance)),
            }
        }

        // Linear gradient
        "shader/gradient" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let direction = get_pin_value(graph, node, "direction").and_then(as_vec2).unwrap_or([0.0, 1.0]);

            // Dot product of UV with direction
            let len_sq = direction[0] * direction[0] + direction[1] * direction[1];
            let value = if len_sq > 0.0 {
                (uv[0] * direction[0] + uv[1] * direction[1]) / len_sq.sqrt()
            } else {
                0.5
            };
            Some(PinValue::Float(value.clamp(0.0, 1.0)))
        }

        // FBM Noise
        "shader/noise_fbm" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let octaves = get_pin_value(graph, node, "octaves").and_then(as_float).unwrap_or(4.0) as i32;
            let frequency = get_pin_value(graph, node, "frequency").and_then(as_float).unwrap_or(1.0);
            let amplitude = get_pin_value(graph, node, "amplitude").and_then(as_float).unwrap_or(0.5);
            let lacunarity = get_pin_value(graph, node, "lacunarity").and_then(as_float).unwrap_or(2.0);
            let persistence = get_pin_value(graph, node, "persistence").and_then(as_float).unwrap_or(0.5);

            let noise = fbm_noise(uv[0], uv[1], octaves, frequency, amplitude, lacunarity, persistence);
            Some(PinValue::Float(noise))
        }

        // Turbulence noise
        "shader/noise_turbulence" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let octaves = get_pin_value(graph, node, "octaves").and_then(as_float).unwrap_or(4.0) as i32;
            let frequency = get_pin_value(graph, node, "frequency").and_then(as_float).unwrap_or(1.0);
            let amplitude = get_pin_value(graph, node, "amplitude").and_then(as_float).unwrap_or(0.5);

            let noise = turbulence_noise(uv[0], uv[1], octaves, frequency, amplitude);
            Some(PinValue::Float(noise))
        }

        // Ridged noise
        "shader/noise_ridged" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let octaves = get_pin_value(graph, node, "octaves").and_then(as_float).unwrap_or(4.0) as i32;
            let frequency = get_pin_value(graph, node, "frequency").and_then(as_float).unwrap_or(1.0);
            let sharpness = get_pin_value(graph, node, "sharpness").and_then(as_float).unwrap_or(2.0);

            let noise = ridged_noise(uv[0], uv[1], octaves, frequency, sharpness);
            Some(PinValue::Float(noise))
        }

        // Brick pattern
        "shader/brick" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let brick_width = get_pin_value(graph, node, "brick_width").and_then(as_float).unwrap_or(0.5);
            let brick_height = get_pin_value(graph, node, "brick_height").and_then(as_float).unwrap_or(0.25);
            let mortar_size = get_pin_value(graph, node, "mortar_size").and_then(as_float).unwrap_or(0.05);

            let row = if brick_height > 0.0 { (uv[1] / brick_height).floor() } else { 0.0 };
            let offset = if (row as i32) % 2 == 1 { 0.5 } else { 0.0 };
            let brick_x = if brick_width > 0.0 { (uv[0] / brick_width + offset).fract() } else { 0.0 };
            let brick_y = if brick_height > 0.0 { (uv[1] / brick_height).fract() } else { 0.0 };

            let is_brick = if brick_x >= mortar_size && brick_x <= 1.0 - mortar_size
                && brick_y >= mortar_size && brick_y <= 1.0 - mortar_size
            {
                1.0
            } else {
                0.0
            };

            match output_pin {
                "brick" => Some(PinValue::Float(is_brick)),
                "mortar" => Some(PinValue::Float(1.0 - is_brick)),
                _ => Some(PinValue::Float(is_brick)),
            }
        }

        // Sine wave pattern
        "shader/wave_sine" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let frequency = get_pin_value(graph, node, "frequency").and_then(as_float).unwrap_or(5.0);
            let amplitude = get_pin_value(graph, node, "amplitude").and_then(as_float).unwrap_or(1.0);
            let phase = get_pin_value(graph, node, "phase").and_then(as_float).unwrap_or(0.0);

            let value = (uv[0] * frequency + phase).sin() * amplitude * 0.5 + 0.5;
            Some(PinValue::Float(value.clamp(0.0, 1.0)))
        }

        // Square wave pattern
        "shader/wave_square" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let frequency = get_pin_value(graph, node, "frequency").and_then(as_float).unwrap_or(5.0);
            let duty = get_pin_value(graph, node, "duty_cycle").and_then(as_float).unwrap_or(0.5);

            let t = (uv[0] * frequency).fract();
            let value = if t < duty { 1.0 } else { 0.0 };
            Some(PinValue::Float(value))
        }

        // Sawtooth wave pattern
        "shader/wave_sawtooth" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let frequency = get_pin_value(graph, node, "frequency").and_then(as_float).unwrap_or(5.0);

            let value = (uv[0] * frequency).fract();
            Some(PinValue::Float(value))
        }

        // Radial gradient
        "shader/radial_gradient" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let center = get_pin_value(graph, node, "center").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let radius = get_pin_value(graph, node, "radius").and_then(as_float).unwrap_or(0.5);

            let dx = uv[0] - center[0];
            let dy = uv[1] - center[1];
            let dist = (dx * dx + dy * dy).sqrt();
            let value = if radius > 0.0 { (dist / radius).clamp(0.0, 1.0) } else { 1.0 };
            Some(PinValue::Float(value))
        }

        // Spiral pattern
        "shader/spiral" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let arms = get_pin_value(graph, node, "arms").and_then(as_float).unwrap_or(1.0);
            let tightness = get_pin_value(graph, node, "tightness").and_then(as_float).unwrap_or(5.0);

            let dx = uv[0] - 0.5;
            let dy = uv[1] - 0.5;
            let angle = dy.atan2(dx);
            let dist = (dx * dx + dy * dy).sqrt();
            let value = ((angle * arms + dist * tightness) / std::f32::consts::TAU).fract();
            Some(PinValue::Float(value))
        }

        // SDF Circle
        "shader/sdf_circle" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let center = get_pin_value(graph, node, "center").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let radius = get_pin_value(graph, node, "radius").and_then(as_float).unwrap_or(0.25);

            let dx = uv[0] - center[0];
            let dy = uv[1] - center[1];
            let dist = (dx * dx + dy * dy).sqrt() - radius;
            Some(PinValue::Float(dist))
        }

        // SDF Box
        "shader/sdf_box" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let center = get_pin_value(graph, node, "center").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let size = get_pin_value(graph, node, "size").and_then(as_vec2).unwrap_or([0.25, 0.25]);

            let dx = (uv[0] - center[0]).abs() - size[0];
            let dy = (uv[1] - center[1]).abs() - size[1];
            let outside = (dx.max(0.0) * dx.max(0.0) + dy.max(0.0) * dy.max(0.0)).sqrt();
            let inside = dx.max(dy).min(0.0);
            Some(PinValue::Float(outside + inside))
        }

        // Domain warp
        "shader/domain_warp" => {
            let uv = get_pin_value(graph, node, "uv").and_then(as_vec2).unwrap_or([0.5, 0.5]);
            let frequency = get_pin_value(graph, node, "frequency").and_then(as_float).unwrap_or(1.0);
            let amplitude = get_pin_value(graph, node, "amplitude").and_then(as_float).unwrap_or(0.1);
            let iterations = get_pin_value(graph, node, "iterations").and_then(as_float).unwrap_or(2.0) as i32;

            let (warped_uv, value) = domain_warp(uv, frequency, amplitude, iterations);

            match output_pin {
                "uv" => Some(PinValue::Vec2(warped_uv)),
                "value" => Some(PinValue::Float(value)),
                _ => Some(PinValue::Vec2(warped_uv)),
            }
        }

        _ => None,
    }
}

// ============================================================================
// Noise helper functions
// ============================================================================

/// Simple hash function for noise
fn hash(x: f32, y: f32) -> f32 {
    let h = (x * 12.9898 + y * 78.233).sin() * 43758.5453;
    h.fract()
}

/// Simple value noise
fn simple_noise(x: f32, y: f32) -> f32 {
    let ix = x.floor();
    let iy = y.floor();
    let fx = x.fract();
    let fy = y.fract();

    // Smoothstep interpolation
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);

    // Corner values
    let a = hash(ix, iy);
    let b = hash(ix + 1.0, iy);
    let c = hash(ix, iy + 1.0);
    let d = hash(ix + 1.0, iy + 1.0);

    // Bilinear interpolation
    lerp(lerp(a, b, ux), lerp(c, d, ux), uy)
}

/// Gradient noise (Perlin-like)
fn gradient_noise(x: f32, y: f32) -> f32 {
    let ix = x.floor();
    let iy = y.floor();
    let fx = x - ix;
    let fy = y - iy;

    // Smoothstep
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);

    // Gradient vectors (pseudo-random based on hash)
    let g00 = gradient_dot(ix, iy, fx, fy);
    let g10 = gradient_dot(ix + 1.0, iy, fx - 1.0, fy);
    let g01 = gradient_dot(ix, iy + 1.0, fx, fy - 1.0);
    let g11 = gradient_dot(ix + 1.0, iy + 1.0, fx - 1.0, fy - 1.0);

    let result = lerp(lerp(g00, g10, ux), lerp(g01, g11, ux), uy);
    result * 0.5 + 0.5 // Normalize to 0-1
}

fn gradient_dot(ix: f32, iy: f32, dx: f32, dy: f32) -> f32 {
    let h = hash(ix, iy);
    let angle = h * std::f32::consts::TAU;
    let gx = angle.cos();
    let gy = angle.sin();
    gx * dx + gy * dy
}

/// Voronoi cellular noise
fn voronoi_noise(x: f32, y: f32) -> (f32, f32) {
    let ix = x.floor();
    let iy = y.floor();
    let fx = x.fract();
    let fy = y.fract();

    let mut min_dist = 1.0f32;
    let mut cell_id = 0.0f32;

    // Check 3x3 neighborhood
    for j in -1..=1 {
        for i in -1..=1 {
            let neighbor_x = i as f32;
            let neighbor_y = j as f32;

            // Random point within cell
            let point_x = hash(ix + neighbor_x, iy + neighbor_y);
            let point_y = hash(ix + neighbor_x + 0.5, iy + neighbor_y + 0.5);

            let dx = neighbor_x + point_x - fx;
            let dy = neighbor_y + point_y - fy;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist < min_dist {
                min_dist = dist;
                cell_id = hash(ix + neighbor_x, iy + neighbor_y);
            }
        }
    }

    (min_dist, cell_id)
}

/// Fractal Brownian Motion noise
fn fbm_noise(x: f32, y: f32, octaves: i32, frequency: f32, amplitude: f32, lacunarity: f32, persistence: f32) -> f32 {
    let mut result = 0.0;
    let mut freq = frequency;
    let mut amp = amplitude;
    let mut max_amp = 0.0;

    for _ in 0..octaves.min(8) {
        result += gradient_noise(x * freq, y * freq) * amp;
        max_amp += amp;
        freq *= lacunarity;
        amp *= persistence;
    }

    if max_amp > 0.0 { result / max_amp } else { 0.5 }
}

/// Turbulence noise (FBM with absolute values)
fn turbulence_noise(x: f32, y: f32, octaves: i32, frequency: f32, amplitude: f32) -> f32 {
    let mut result = 0.0;
    let mut freq = frequency;
    let mut amp = amplitude;
    let mut max_amp = 0.0;

    for _ in 0..octaves.min(8) {
        result += (gradient_noise(x * freq, y * freq) * 2.0 - 1.0).abs() * amp;
        max_amp += amp;
        freq *= 2.0;
        amp *= 0.5;
    }

    if max_amp > 0.0 { result / max_amp } else { 0.5 }
}

/// Ridged multifractal noise
fn ridged_noise(x: f32, y: f32, octaves: i32, frequency: f32, sharpness: f32) -> f32 {
    let mut result = 0.0;
    let mut freq = frequency;
    let mut amp = 1.0;

    for _ in 0..octaves.min(8) {
        let n = gradient_noise(x * freq, y * freq);
        let ridge = 1.0 - (n * 2.0 - 1.0).abs();
        result += ridge.powf(sharpness) * amp;
        freq *= 2.0;
        amp *= 0.5;
    }

    result.clamp(0.0, 1.0)
}

/// Domain warping
fn domain_warp(uv: [f32; 2], frequency: f32, amplitude: f32, iterations: i32) -> ([f32; 2], f32) {
    let mut x = uv[0];
    let mut y = uv[1];

    for _ in 0..iterations.min(4) {
        let nx = gradient_noise(x * frequency, y * frequency);
        let ny = gradient_noise((x + 5.2) * frequency, (y + 1.3) * frequency);
        x += (nx * 2.0 - 1.0) * amplitude;
        y += (ny * 2.0 - 1.0) * amplitude;
    }

    let value = gradient_noise(x * frequency, y * frequency);
    ([x, y], value)
}

/// Linear interpolation
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
