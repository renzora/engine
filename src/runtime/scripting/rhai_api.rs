//! Rhai API registration for runtime
//!
//! Registers all API functions available to scripts.

use rhai::{Engine, Dynamic, Map, ImmutableString};

/// Register all API functions with the Rhai engine
pub fn register_all(engine: &mut Engine) {
    // Math functions
    register_math(engine);
    // Input functions
    register_input(engine);
    // Transform functions
    register_transform(engine);
    // Physics functions
    register_physics(engine);
    // Audio functions
    register_audio(engine);
    // Timer functions
    register_timers(engine);
    // Debug functions
    register_debug(engine);
    // ECS functions
    register_ecs(engine);
    // Health functions
    register_health(engine);
}

fn register_math(engine: &mut Engine) {
    // Vector creation
    engine.register_fn("vec2", |x: f64, y: f64| -> Map {
        Map::from([
            ("x".into(), Dynamic::from(x)),
            ("y".into(), Dynamic::from(y)),
        ])
    });

    engine.register_fn("vec3", |x: f64, y: f64, z: f64| -> Map {
        Map::from([
            ("x".into(), Dynamic::from(x)),
            ("y".into(), Dynamic::from(y)),
            ("z".into(), Dynamic::from(z)),
        ])
    });

    engine.register_fn("color", |r: f64, g: f64, b: f64, a: f64| -> Map {
        Map::from([
            ("r".into(), Dynamic::from(r)),
            ("g".into(), Dynamic::from(g)),
            ("b".into(), Dynamic::from(b)),
            ("a".into(), Dynamic::from(a)),
        ])
    });

    // Math operations
    engine.register_fn("lerp", |a: f64, b: f64, t: f64| -> f64 {
        a + (b - a) * t
    });

    engine.register_fn("clamp", |v: f64, min: f64, max: f64| -> f64 {
        v.clamp(min, max)
    });

    engine.register_fn("abs", |v: f64| -> f64 { v.abs() });
    engine.register_fn("min", |a: f64, b: f64| -> f64 { a.min(b) });
    engine.register_fn("max", |a: f64, b: f64| -> f64 { a.max(b) });
    engine.register_fn("sin", |v: f64| -> f64 { v.sin() });
    engine.register_fn("cos", |v: f64| -> f64 { v.cos() });
    engine.register_fn("sqrt", |v: f64| -> f64 { v.sqrt() });
    engine.register_fn("pow", |v: f64, n: f64| -> f64 { v.powf(n) });
    engine.register_fn("floor", |v: f64| -> f64 { v.floor() });
    engine.register_fn("ceil", |v: f64| -> f64 { v.ceil() });
    engine.register_fn("round", |v: f64| -> f64 { v.round() });

    // Random - using simple LCG-based pseudo-random
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEED: AtomicU64 = AtomicU64::new(12345);

    fn simple_random() -> f64 {
        // Simple LCG: seed = (seed * 1103515245 + 12345) mod 2^32
        let old_seed = SEED.load(Ordering::Relaxed);
        let new_seed = old_seed.wrapping_mul(1103515245).wrapping_add(12345) & 0x7FFFFFFF;
        SEED.store(new_seed, Ordering::Relaxed);
        (new_seed as f64) / (0x7FFFFFFF as f64)
    }

    engine.register_fn("random", || -> f64 { simple_random() });
    engine.register_fn("random_range", |min: f64, max: f64| -> f64 {
        min + simple_random() * (max - min)
    });

    // Vector operations
    engine.register_fn("length", |v: Map| -> f64 {
        let x = v.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0);
        let y = v.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0);
        let z = v.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0);
        (x * x + y * y + z * z).sqrt()
    });

    engine.register_fn("normalize", |v: Map| -> Map {
        let x = v.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0);
        let y = v.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0);
        let z = v.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0);
        let len = (x * x + y * y + z * z).sqrt();
        if len > 0.0 {
            Map::from([
                ("x".into(), Dynamic::from(x / len)),
                ("y".into(), Dynamic::from(y / len)),
                ("z".into(), Dynamic::from(z / len)),
            ])
        } else {
            v
        }
    });

    engine.register_fn("distance", |a: Map, b: Map| -> f64 {
        let ax = a.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0);
        let ay = a.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0);
        let az = a.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0);
        let bx = b.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0);
        let by = b.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0);
        let bz = b.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0);
        let dx = ax - bx;
        let dy = ay - by;
        let dz = az - bz;
        (dx * dx + dy * dy + dz * dz).sqrt()
    });
}

fn register_input(engine: &mut Engine) {
    // Key checking - these read from scope variables set by the executor
    engine.register_fn("is_key_pressed", |key: ImmutableString| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("check_key_pressed")),
            ("key".into(), Dynamic::from(key)),
        ])
    });

    engine.register_fn("is_key_just_pressed", |key: ImmutableString| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("check_key_just_pressed")),
            ("key".into(), Dynamic::from(key)),
        ])
    });
}

fn register_transform(engine: &mut Engine) {
    // Transform manipulation - these generate commands
    engine.register_fn("set_position", |x: f64, y: f64, z: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("set_self_position")),
            ("x".into(), Dynamic::from(x)),
            ("y".into(), Dynamic::from(y)),
            ("z".into(), Dynamic::from(z)),
        ])
    });

    engine.register_fn("set_position_of", |entity: i64, x: f64, y: f64, z: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("set_position")),
            ("entity".into(), Dynamic::from(entity)),
            ("x".into(), Dynamic::from(x)),
            ("y".into(), Dynamic::from(y)),
            ("z".into(), Dynamic::from(z)),
        ])
    });

    engine.register_fn("translate", |x: f64, y: f64, z: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("translate_self")),
            ("x".into(), Dynamic::from(x)),
            ("y".into(), Dynamic::from(y)),
            ("z".into(), Dynamic::from(z)),
        ])
    });

    engine.register_fn("set_rotation", |x: f64, y: f64, z: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("set_self_rotation")),
            ("x".into(), Dynamic::from(x)),
            ("y".into(), Dynamic::from(y)),
            ("z".into(), Dynamic::from(z)),
        ])
    });

    engine.register_fn("rotate", |x: f64, y: f64, z: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("rotate_self")),
            ("x".into(), Dynamic::from(x)),
            ("y".into(), Dynamic::from(y)),
            ("z".into(), Dynamic::from(z)),
        ])
    });

    engine.register_fn("look_at", |x: f64, y: f64, z: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("look_at_self")),
            ("x".into(), Dynamic::from(x)),
            ("y".into(), Dynamic::from(y)),
            ("z".into(), Dynamic::from(z)),
        ])
    });
}

fn register_physics(engine: &mut Engine) {
    engine.register_fn("apply_force", |x: f64, y: f64, z: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("apply_force_self")),
            ("x".into(), Dynamic::from(x)),
            ("y".into(), Dynamic::from(y)),
            ("z".into(), Dynamic::from(z)),
        ])
    });

    engine.register_fn("apply_force_to", |entity: i64, x: f64, y: f64, z: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("apply_force")),
            ("entity".into(), Dynamic::from(entity)),
            ("x".into(), Dynamic::from(x)),
            ("y".into(), Dynamic::from(y)),
            ("z".into(), Dynamic::from(z)),
        ])
    });

    engine.register_fn("apply_impulse", |x: f64, y: f64, z: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("apply_impulse_self")),
            ("x".into(), Dynamic::from(x)),
            ("y".into(), Dynamic::from(y)),
            ("z".into(), Dynamic::from(z)),
        ])
    });

    engine.register_fn("set_velocity", |x: f64, y: f64, z: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("set_velocity_self")),
            ("x".into(), Dynamic::from(x)),
            ("y".into(), Dynamic::from(y)),
            ("z".into(), Dynamic::from(z)),
        ])
    });

    engine.register_fn("raycast", |ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, max_dist: f64, result_var: ImmutableString| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("raycast")),
            ("origin_x".into(), Dynamic::from(ox)),
            ("origin_y".into(), Dynamic::from(oy)),
            ("origin_z".into(), Dynamic::from(oz)),
            ("dir_x".into(), Dynamic::from(dx)),
            ("dir_y".into(), Dynamic::from(dy)),
            ("dir_z".into(), Dynamic::from(dz)),
            ("max_distance".into(), Dynamic::from(max_dist)),
            ("result_var".into(), Dynamic::from(result_var)),
        ])
    });
}

fn register_audio(engine: &mut Engine) {
    engine.register_fn("play_sound", |path: ImmutableString| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("play_sound")),
            ("path".into(), Dynamic::from(path)),
            ("volume".into(), Dynamic::from(1.0_f64)),
            ("looping".into(), Dynamic::from(false)),
        ])
    });

    engine.register_fn("play_sound_at_volume", |path: ImmutableString, volume: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("play_sound")),
            ("path".into(), Dynamic::from(path)),
            ("volume".into(), Dynamic::from(volume)),
            ("looping".into(), Dynamic::from(false)),
        ])
    });

    engine.register_fn("play_music", |path: ImmutableString| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("play_music")),
            ("path".into(), Dynamic::from(path)),
            ("volume".into(), Dynamic::from(1.0_f64)),
            ("fade_in".into(), Dynamic::from(0.0_f64)),
        ])
    });

    engine.register_fn("play_music_with_fade", |path: ImmutableString, fade_in: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("play_music")),
            ("path".into(), Dynamic::from(path)),
            ("volume".into(), Dynamic::from(1.0_f64)),
            ("fade_in".into(), Dynamic::from(fade_in)),
        ])
    });

    engine.register_fn("stop_music", || -> Map {
        Map::from([
            ("type".into(), Dynamic::from("stop_music")),
            ("fade_out".into(), Dynamic::from(0.0_f64)),
        ])
    });

    engine.register_fn("stop_music_with_fade", |fade_out: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("stop_music")),
            ("fade_out".into(), Dynamic::from(fade_out)),
        ])
    });

    engine.register_fn("set_master_volume", |volume: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("set_master_volume")),
            ("volume".into(), Dynamic::from(volume)),
        ])
    });
}

fn register_timers(engine: &mut Engine) {
    engine.register_fn("start_timer", |name: ImmutableString, duration: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("start_timer")),
            ("name".into(), Dynamic::from(name)),
            ("duration".into(), Dynamic::from(duration)),
            ("repeat".into(), Dynamic::from(false)),
        ])
    });

    engine.register_fn("start_repeating_timer", |name: ImmutableString, duration: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("start_timer")),
            ("name".into(), Dynamic::from(name)),
            ("duration".into(), Dynamic::from(duration)),
            ("repeat".into(), Dynamic::from(true)),
        ])
    });

    engine.register_fn("stop_timer", |name: ImmutableString| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("stop_timer")),
            ("name".into(), Dynamic::from(name)),
        ])
    });
}

fn register_debug(engine: &mut Engine) {
    engine.register_fn("log", |msg: ImmutableString| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("log")),
            ("message".into(), Dynamic::from(msg)),
        ])
    });

    engine.register_fn("draw_line", |sx: f64, sy: f64, sz: f64, ex: f64, ey: f64, ez: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("draw_line")),
            ("start_x".into(), Dynamic::from(sx)),
            ("start_y".into(), Dynamic::from(sy)),
            ("start_z".into(), Dynamic::from(sz)),
            ("end_x".into(), Dynamic::from(ex)),
            ("end_y".into(), Dynamic::from(ey)),
            ("end_z".into(), Dynamic::from(ez)),
            ("color_r".into(), Dynamic::from(1.0_f64)),
            ("color_g".into(), Dynamic::from(1.0_f64)),
            ("color_b".into(), Dynamic::from(1.0_f64)),
            ("color_a".into(), Dynamic::from(1.0_f64)),
            ("duration".into(), Dynamic::from(0.0_f64)),
        ])
    });

    engine.register_fn("draw_sphere", |cx: f64, cy: f64, cz: f64, radius: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("draw_sphere")),
            ("center_x".into(), Dynamic::from(cx)),
            ("center_y".into(), Dynamic::from(cy)),
            ("center_z".into(), Dynamic::from(cz)),
            ("radius".into(), Dynamic::from(radius)),
            ("color_r".into(), Dynamic::from(1.0_f64)),
            ("color_g".into(), Dynamic::from(1.0_f64)),
            ("color_b".into(), Dynamic::from(1.0_f64)),
            ("color_a".into(), Dynamic::from(1.0_f64)),
            ("duration".into(), Dynamic::from(0.0_f64)),
        ])
    });
}

fn register_ecs(engine: &mut Engine) {
    engine.register_fn("spawn_entity", |name: ImmutableString| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("spawn_entity")),
            ("name".into(), Dynamic::from(name)),
        ])
    });

    engine.register_fn("despawn_entity", |entity: i64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("despawn_entity")),
            ("entity".into(), Dynamic::from(entity)),
        ])
    });

    engine.register_fn("find_entity", |name: ImmutableString| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("find_entity")),
            ("name".into(), Dynamic::from(name)),
        ])
    });

    engine.register_fn("set_visibility", |entity: i64, visible: bool| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("set_visibility")),
            ("entity".into(), Dynamic::from(entity)),
            ("visible".into(), Dynamic::from(visible)),
        ])
    });
}

fn register_health(engine: &mut Engine) {
    engine.register_fn("damage", |entity: i64, amount: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("damage")),
            ("entity".into(), Dynamic::from(entity)),
            ("amount".into(), Dynamic::from(amount)),
        ])
    });

    engine.register_fn("heal", |entity: i64, amount: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("heal")),
            ("entity".into(), Dynamic::from(entity)),
            ("amount".into(), Dynamic::from(amount)),
        ])
    });

    engine.register_fn("set_health", |entity: i64, amount: f64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("set_health")),
            ("entity".into(), Dynamic::from(entity)),
            ("amount".into(), Dynamic::from(amount)),
        ])
    });

    engine.register_fn("kill", |entity: i64| -> Map {
        Map::from([
            ("type".into(), Dynamic::from("kill")),
            ("entity".into(), Dynamic::from(entity)),
        ])
    });
}
