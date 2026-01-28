//! Debug API functions for Rhai scripts

use rhai::{Dynamic, Engine, Map, ImmutableString};

/// Register debug functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Logging
    // ===================

    // log(message) - Log info message
    engine.register_fn("log", |message: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("log"));
        m.insert("level".into(), Dynamic::from("info"));
        m.insert("message".into(), Dynamic::from(message));
        m
    });

    // log_info(message)
    engine.register_fn("log_info", |message: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("log"));
        m.insert("level".into(), Dynamic::from("info"));
        m.insert("message".into(), Dynamic::from(message));
        m
    });

    // log_warn(message)
    engine.register_fn("log_warn", |message: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("log"));
        m.insert("level".into(), Dynamic::from("warn"));
        m.insert("message".into(), Dynamic::from(message));
        m
    });

    // log_error(message)
    engine.register_fn("log_error", |message: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("log"));
        m.insert("level".into(), Dynamic::from("error"));
        m.insert("message".into(), Dynamic::from(message));
        m
    });

    // log_debug(message)
    engine.register_fn("log_debug", |message: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("log"));
        m.insert("level".into(), Dynamic::from("debug"));
        m.insert("message".into(), Dynamic::from(message));
        m
    });

    // ===================
    // Debug Drawing
    // ===================

    // draw_line(start_x, start_y, start_z, end_x, end_y, end_z)
    engine.register_fn("draw_line", |sx: f64, sy: f64, sz: f64, ex: f64, ey: f64, ez: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("draw_line"));
        m.insert("sx".into(), Dynamic::from(sx));
        m.insert("sy".into(), Dynamic::from(sy));
        m.insert("sz".into(), Dynamic::from(sz));
        m.insert("ex".into(), Dynamic::from(ex));
        m.insert("ey".into(), Dynamic::from(ey));
        m.insert("ez".into(), Dynamic::from(ez));
        m.insert("r".into(), Dynamic::from(1.0));
        m.insert("g".into(), Dynamic::from(1.0));
        m.insert("b".into(), Dynamic::from(1.0));
        m.insert("a".into(), Dynamic::from(1.0));
        m.insert("duration".into(), Dynamic::from(0.0));
        m
    });

    // draw_line_color(sx, sy, sz, ex, ey, ez, r, g, b)
    engine.register_fn("draw_line_color", |sx: f64, sy: f64, sz: f64, ex: f64, ey: f64, ez: f64, r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("draw_line"));
        m.insert("sx".into(), Dynamic::from(sx));
        m.insert("sy".into(), Dynamic::from(sy));
        m.insert("sz".into(), Dynamic::from(sz));
        m.insert("ex".into(), Dynamic::from(ex));
        m.insert("ey".into(), Dynamic::from(ey));
        m.insert("ez".into(), Dynamic::from(ez));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m.insert("a".into(), Dynamic::from(1.0));
        m.insert("duration".into(), Dynamic::from(0.0));
        m
    });

    // draw_line_duration(sx, sy, sz, ex, ey, ez, r, g, b, duration)
    engine.register_fn("draw_line_duration", |sx: f64, sy: f64, sz: f64, ex: f64, ey: f64, ez: f64, r: f64, g: f64, b: f64, duration: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("draw_line"));
        m.insert("sx".into(), Dynamic::from(sx));
        m.insert("sy".into(), Dynamic::from(sy));
        m.insert("sz".into(), Dynamic::from(sz));
        m.insert("ex".into(), Dynamic::from(ex));
        m.insert("ey".into(), Dynamic::from(ey));
        m.insert("ez".into(), Dynamic::from(ez));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m.insert("a".into(), Dynamic::from(1.0));
        m.insert("duration".into(), Dynamic::from(duration));
        m
    });

    // draw_ray(origin_x, origin_y, origin_z, dir_x, dir_y, dir_z, length)
    engine.register_fn("draw_ray", |ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, length: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("draw_ray"));
        m.insert("ox".into(), Dynamic::from(ox));
        m.insert("oy".into(), Dynamic::from(oy));
        m.insert("oz".into(), Dynamic::from(oz));
        m.insert("dx".into(), Dynamic::from(dx));
        m.insert("dy".into(), Dynamic::from(dy));
        m.insert("dz".into(), Dynamic::from(dz));
        m.insert("length".into(), Dynamic::from(length));
        m.insert("r".into(), Dynamic::from(1.0));
        m.insert("g".into(), Dynamic::from(0.0));
        m.insert("b".into(), Dynamic::from(0.0));
        m.insert("a".into(), Dynamic::from(1.0));
        m.insert("duration".into(), Dynamic::from(0.0));
        m
    });

    // draw_sphere(center_x, center_y, center_z, radius)
    engine.register_fn("draw_sphere", |x: f64, y: f64, z: f64, radius: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("draw_sphere"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m.insert("radius".into(), Dynamic::from(radius));
        m.insert("r".into(), Dynamic::from(0.0));
        m.insert("g".into(), Dynamic::from(1.0));
        m.insert("b".into(), Dynamic::from(0.0));
        m.insert("a".into(), Dynamic::from(0.5));
        m.insert("duration".into(), Dynamic::from(0.0));
        m
    });

    // draw_sphere_color(x, y, z, radius, r, g, b)
    engine.register_fn("draw_sphere_color", |x: f64, y: f64, z: f64, radius: f64, r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("draw_sphere"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m.insert("radius".into(), Dynamic::from(radius));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m.insert("a".into(), Dynamic::from(0.5));
        m.insert("duration".into(), Dynamic::from(0.0));
        m
    });

    // draw_box(center_x, center_y, center_z, half_x, half_y, half_z)
    engine.register_fn("draw_box", |x: f64, y: f64, z: f64, hx: f64, hy: f64, hz: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("draw_box"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m.insert("hx".into(), Dynamic::from(hx));
        m.insert("hy".into(), Dynamic::from(hy));
        m.insert("hz".into(), Dynamic::from(hz));
        m.insert("r".into(), Dynamic::from(0.0));
        m.insert("g".into(), Dynamic::from(0.5));
        m.insert("b".into(), Dynamic::from(1.0));
        m.insert("a".into(), Dynamic::from(0.5));
        m.insert("duration".into(), Dynamic::from(0.0));
        m
    });

    // draw_box_color(x, y, z, hx, hy, hz, r, g, b)
    engine.register_fn("draw_box_color", |x: f64, y: f64, z: f64, hx: f64, hy: f64, hz: f64, r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("draw_box"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m.insert("hx".into(), Dynamic::from(hx));
        m.insert("hy".into(), Dynamic::from(hy));
        m.insert("hz".into(), Dynamic::from(hz));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m.insert("a".into(), Dynamic::from(0.5));
        m.insert("duration".into(), Dynamic::from(0.0));
        m
    });

    // draw_point(x, y, z, size)
    engine.register_fn("draw_point", |x: f64, y: f64, z: f64, size: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("draw_point"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m.insert("size".into(), Dynamic::from(size));
        m.insert("r".into(), Dynamic::from(1.0));
        m.insert("g".into(), Dynamic::from(1.0));
        m.insert("b".into(), Dynamic::from(0.0));
        m.insert("a".into(), Dynamic::from(1.0));
        m.insert("duration".into(), Dynamic::from(0.0));
        m
    });

    // ===================
    // Assert/Check
    // ===================

    // assert(condition, message) - Log error if condition is false
    engine.register_fn("assert", |condition: bool, message: ImmutableString| -> Map {
        let mut m = Map::new();
        if !condition {
            m.insert("_cmd".into(), Dynamic::from("log"));
            m.insert("level".into(), Dynamic::from("error"));
            m.insert("message".into(), Dynamic::from(format!("ASSERT FAILED: {}", message)));
        }
        m
    });
}
