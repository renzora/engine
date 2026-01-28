//! Math API functions for Rhai scripts

use rhai::Engine;

/// Register math functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Basic math
    // ===================

    engine.register_fn("sin", |x: f64| -> f64 { x.sin() });
    engine.register_fn("cos", |x: f64| -> f64 { x.cos() });
    engine.register_fn("tan", |x: f64| -> f64 { x.tan() });
    engine.register_fn("asin", |x: f64| -> f64 { x.asin() });
    engine.register_fn("acos", |x: f64| -> f64 { x.acos() });
    engine.register_fn("atan", |x: f64| -> f64 { x.atan() });
    engine.register_fn("atan2", |y: f64, x: f64| -> f64 { y.atan2(x) });

    engine.register_fn("sqrt", |x: f64| -> f64 { x.sqrt() });
    engine.register_fn("abs", |x: f64| -> f64 { x.abs() });
    engine.register_fn("floor", |x: f64| -> f64 { x.floor() });
    engine.register_fn("ceil", |x: f64| -> f64 { x.ceil() });
    engine.register_fn("round", |x: f64| -> f64 { x.round() });
    engine.register_fn("trunc", |x: f64| -> f64 { x.trunc() });
    engine.register_fn("fract", |x: f64| -> f64 { x.fract() });

    engine.register_fn("pow", |base: f64, exp: f64| -> f64 { base.powf(exp) });
    engine.register_fn("exp", |x: f64| -> f64 { x.exp() });
    engine.register_fn("ln", |x: f64| -> f64 { x.ln() });
    engine.register_fn("log", |x: f64, base: f64| -> f64 { x.log(base) });
    engine.register_fn("log10", |x: f64| -> f64 { x.log10() });
    engine.register_fn("log2", |x: f64| -> f64 { x.log2() });

    // ===================
    // Min/Max/Clamp
    // ===================

    engine.register_fn("min", |a: f64, b: f64| -> f64 { a.min(b) });
    engine.register_fn("max", |a: f64, b: f64| -> f64 { a.max(b) });
    engine.register_fn("clamp", |value: f64, min: f64, max: f64| -> f64 {
        value.max(min).min(max)
    });

    // Integer versions
    engine.register_fn("min_int", |a: i64, b: i64| -> i64 { a.min(b) });
    engine.register_fn("max_int", |a: i64, b: i64| -> i64 { a.max(b) });
    engine.register_fn("clamp_int", |value: i64, min: i64, max: i64| -> i64 {
        value.max(min).min(max)
    });

    // ===================
    // Interpolation
    // ===================

    engine.register_fn("lerp", |a: f64, b: f64, t: f64| -> f64 {
        a + (b - a) * t
    });

    engine.register_fn("inverse_lerp", |a: f64, b: f64, value: f64| -> f64 {
        if (b - a).abs() < 0.0001 { 0.0 } else { (value - a) / (b - a) }
    });

    engine.register_fn("smoothstep", |edge0: f64, edge1: f64, x: f64| -> f64 {
        let t = ((x - edge0) / (edge1 - edge0)).max(0.0).min(1.0);
        t * t * (3.0 - 2.0 * t)
    });

    engine.register_fn("smootherstep", |edge0: f64, edge1: f64, x: f64| -> f64 {
        let t = ((x - edge0) / (edge1 - edge0)).max(0.0).min(1.0);
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    });

    // Move towards target by max_delta
    engine.register_fn("move_towards", |current: f64, target: f64, max_delta: f64| -> f64 {
        let diff = target - current;
        if diff.abs() <= max_delta {
            target
        } else {
            current + diff.signum() * max_delta
        }
    });

    // ===================
    // Angle conversion
    // ===================

    engine.register_fn("deg_to_rad", |deg: f64| -> f64 { deg.to_radians() });
    engine.register_fn("rad_to_deg", |rad: f64| -> f64 { rad.to_degrees() });

    // Normalize angle to 0-360 range
    engine.register_fn("normalize_angle", |angle: f64| -> f64 {
        let mut a = angle % 360.0;
        if a < 0.0 { a += 360.0; }
        a
    });

    // Shortest angle difference (handles wrap-around)
    engine.register_fn("angle_difference", |from: f64, to: f64| -> f64 {
        let diff = (to - from) % 360.0;
        if diff > 180.0 { diff - 360.0 }
        else if diff < -180.0 { diff + 360.0 }
        else { diff }
    });

    // Lerp angle (handles wrap-around)
    engine.register_fn("lerp_angle", |from: f64, to: f64, t: f64| -> f64 {
        let diff = (to - from) % 360.0;
        let short_diff = if diff > 180.0 { diff - 360.0 }
            else if diff < -180.0 { diff + 360.0 }
            else { diff };
        from + short_diff * t
    });

    // ===================
    // Vector math helpers
    // ===================

    // Distance between two 3D points
    engine.register_fn("distance", |x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64| -> f64 {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let dz = z2 - z1;
        (dx*dx + dy*dy + dz*dz).sqrt()
    });

    // Distance 2D
    engine.register_fn("distance_2d", |x1: f64, y1: f64, x2: f64, y2: f64| -> f64 {
        let dx = x2 - x1;
        let dy = y2 - y1;
        (dx*dx + dy*dy).sqrt()
    });

    // Length of a 3D vector
    engine.register_fn("length", |x: f64, y: f64, z: f64| -> f64 {
        (x*x + y*y + z*z).sqrt()
    });

    // Length of a 2D vector
    engine.register_fn("length_2d", |x: f64, y: f64| -> f64 {
        (x*x + y*y).sqrt()
    });

    // Dot product 3D
    engine.register_fn("dot", |x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64| -> f64 {
        x1*x2 + y1*y2 + z1*z2
    });

    // Dot product 2D
    engine.register_fn("dot_2d", |x1: f64, y1: f64, x2: f64, y2: f64| -> f64 {
        x1*x2 + y1*y2
    });

    // ===================
    // Random
    // ===================

    engine.register_fn("random", || -> f64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        // Simple LCG-based random
        let x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (x as f64) / (u128::MAX as f64)
    });

    engine.register_fn("random_range", |min: f64, max: f64| -> f64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let t = (x as f64) / (u128::MAX as f64);
        min + (max - min) * t
    });

    engine.register_fn("random_int", |min: i64, max: i64| -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let range = (max - min + 1) as u128;
        min + ((x % range) as i64)
    });

    // ===================
    // Sign/Step
    // ===================

    engine.register_fn("sign", |x: f64| -> f64 {
        if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
    });

    engine.register_fn("step", |edge: f64, x: f64| -> f64 {
        if x < edge { 0.0 } else { 1.0 }
    });

    // ===================
    // Constants
    // ===================

    engine.register_fn("pi", || -> f64 { std::f64::consts::PI });
    engine.register_fn("tau", || -> f64 { std::f64::consts::TAU });
    engine.register_fn("e", || -> f64 { std::f64::consts::E });
}
