//! Scene and prefab API functions for Rhai scripts

use rhai::{Dynamic, Engine, Map, ImmutableString};

/// Register scene/prefab functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Scene Loading
    // ===================

    // load_scene(path) - Load a scene file (during play mode, this is deferred)
    engine.register_fn("load_scene", |path: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("load_scene"));
        m.insert("path".into(), Dynamic::from(path));
        m
    });

    // unload_scene(handle_id) - Unload a previously loaded scene
    engine.register_fn("unload_scene", |handle_id: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("unload_scene"));
        m.insert("handle_id".into(), Dynamic::from(handle_id));
        m
    });

    // ===================
    // Prefab Spawning
    // ===================

    // spawn_prefab(path, x, y, z) - Spawn a prefab at position with no rotation
    engine.register_fn("spawn_prefab", |path: ImmutableString, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_prefab"));
        m.insert("path".into(), Dynamic::from(path));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m.insert("rx".into(), Dynamic::from(0.0));
        m.insert("ry".into(), Dynamic::from(0.0));
        m.insert("rz".into(), Dynamic::from(0.0));
        m
    });

    // spawn_prefab_rotated(path, x, y, z, rx, ry, rz) - Spawn with rotation (degrees)
    engine.register_fn("spawn_prefab_rotated", |path: ImmutableString, x: f64, y: f64, z: f64, rx: f64, ry: f64, rz: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_prefab"));
        m.insert("path".into(), Dynamic::from(path));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m.insert("rx".into(), Dynamic::from(rx));
        m.insert("ry".into(), Dynamic::from(ry));
        m.insert("rz".into(), Dynamic::from(rz));
        m
    });

    // spawn_prefab_at(path, position_map) - Spawn using a position map {x, y, z}
    engine.register_fn("spawn_prefab_at", |path: ImmutableString, pos: Map| -> Map {
        let x = pos.get("x").and_then(|v| v.as_float().ok()).unwrap_or(0.0);
        let y = pos.get("y").and_then(|v| v.as_float().ok()).unwrap_or(0.0);
        let z = pos.get("z").and_then(|v| v.as_float().ok()).unwrap_or(0.0);

        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_prefab"));
        m.insert("path".into(), Dynamic::from(path));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m.insert("rx".into(), Dynamic::from(0.0));
        m.insert("ry".into(), Dynamic::from(0.0));
        m.insert("rz".into(), Dynamic::from(0.0));
        m
    });

    // spawn_prefab_at_transform(path, position_map, rotation_map) - Spawn with both position and rotation maps
    engine.register_fn("spawn_prefab_at_transform", |path: ImmutableString, pos: Map, rot: Map| -> Map {
        let x = pos.get("x").and_then(|v| v.as_float().ok()).unwrap_or(0.0);
        let y = pos.get("y").and_then(|v| v.as_float().ok()).unwrap_or(0.0);
        let z = pos.get("z").and_then(|v| v.as_float().ok()).unwrap_or(0.0);
        let rx = rot.get("x").and_then(|v| v.as_float().ok()).unwrap_or(0.0);
        let ry = rot.get("y").and_then(|v| v.as_float().ok()).unwrap_or(0.0);
        let rz = rot.get("z").and_then(|v| v.as_float().ok()).unwrap_or(0.0);

        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_prefab"));
        m.insert("path".into(), Dynamic::from(path));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m.insert("rx".into(), Dynamic::from(rx));
        m.insert("ry".into(), Dynamic::from(ry));
        m.insert("rz".into(), Dynamic::from(rz));
        m
    });

    // ===================
    // Prefab Helper - spawn at self position
    // ===================

    // spawn_prefab_here(path) - Spawn a prefab at the calling entity's position
    // Note: This requires _position to be set in the scope. The function returns a command
    // that will be processed with the script's context
    engine.register_fn("spawn_prefab_here", |path: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_prefab_here"));
        m.insert("path".into(), Dynamic::from(path));
        m
    });
}
