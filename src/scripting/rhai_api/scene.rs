//! Scene and prefab API functions for Rhai scripts

use rhai::{Engine, Map, ImmutableString};
use super::super::rhai_commands::RhaiCommand;

/// Register scene/prefab functions
pub fn register(engine: &mut Engine) {
    engine.register_fn("load_scene", |path: ImmutableString| {
        super::push_command(RhaiCommand::LoadScene { path: path.to_string() });
    });

    engine.register_fn("unload_scene", |handle_id: i64| {
        super::push_command(RhaiCommand::UnloadScene { handle_id: handle_id as u64 });
    });

    engine.register_fn("spawn_prefab", |path: ImmutableString, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SpawnPrefab {
            path: path.to_string(),
            position: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32),
            rotation: bevy::prelude::Vec3::ZERO,
        });
    });

    engine.register_fn("spawn_prefab_rotated", |path: ImmutableString, x: f64, y: f64, z: f64, rx: f64, ry: f64, rz: f64| {
        super::push_command(RhaiCommand::SpawnPrefab {
            path: path.to_string(),
            position: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32),
            rotation: bevy::prelude::Vec3::new(rx as f32, ry as f32, rz as f32),
        });
    });

    engine.register_fn("spawn_prefab_at", |path: ImmutableString, pos: Map| {
        let x = pos.get("x").and_then(|v| v.as_float().ok()).unwrap_or(0.0) as f32;
        let y = pos.get("y").and_then(|v| v.as_float().ok()).unwrap_or(0.0) as f32;
        let z = pos.get("z").and_then(|v| v.as_float().ok()).unwrap_or(0.0) as f32;
        super::push_command(RhaiCommand::SpawnPrefab {
            path: path.to_string(),
            position: bevy::prelude::Vec3::new(x, y, z),
            rotation: bevy::prelude::Vec3::ZERO,
        });
    });

    engine.register_fn("spawn_prefab_at_transform", |path: ImmutableString, pos: Map, rot: Map| {
        let x = pos.get("x").and_then(|v| v.as_float().ok()).unwrap_or(0.0) as f32;
        let y = pos.get("y").and_then(|v| v.as_float().ok()).unwrap_or(0.0) as f32;
        let z = pos.get("z").and_then(|v| v.as_float().ok()).unwrap_or(0.0) as f32;
        let rx = rot.get("x").and_then(|v| v.as_float().ok()).unwrap_or(0.0) as f32;
        let ry = rot.get("y").and_then(|v| v.as_float().ok()).unwrap_or(0.0) as f32;
        let rz = rot.get("z").and_then(|v| v.as_float().ok()).unwrap_or(0.0) as f32;
        super::push_command(RhaiCommand::SpawnPrefab {
            path: path.to_string(),
            position: bevy::prelude::Vec3::new(x, y, z),
            rotation: bevy::prelude::Vec3::new(rx, ry, rz),
        });
    });

    engine.register_fn("spawn_prefab_here", |path: ImmutableString| {
        super::push_command(RhaiCommand::SpawnPrefab {
            path: path.to_string(),
            position: bevy::prelude::Vec3::ZERO,
            rotation: bevy::prelude::Vec3::ZERO,
        });
    });
}
