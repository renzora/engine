//! Runtime Script Engine

use bevy::prelude::*;
use rhai::{Engine, AST, Scope, Dynamic, Map, ImmutableString};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use super::rhai_api;
use super::context::RhaiScriptContext;
use super::commands::RhaiCommand;
use crate::shared::ScriptVariableValue;

/// Cached compiled Rhai script
#[derive(Clone)]
pub struct CompiledScript {
    pub ast: AST,
    pub path: PathBuf,
    pub name: String,
    pub last_modified: std::time::SystemTime,
}

/// Runtime script engine resource
#[derive(Resource)]
pub struct RuntimeScriptEngine {
    engine: Engine,
    scripts_folder: PathBuf,
    cache: Arc<RwLock<HashMap<String, CompiledScript>>>,
}

impl Default for RuntimeScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeScriptEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        rhai_api::register_all(&mut engine);

        Self {
            engine,
            scripts_folder: PathBuf::from("scripts"),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn set_scripts_folder(&mut self, folder: PathBuf) {
        self.scripts_folder = folder;
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    pub fn get_available_scripts(&self) -> Vec<(String, PathBuf)> {
        let mut scripts = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.scripts_folder) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "rhai") {
                    if let Some(name) = path.file_stem() {
                        scripts.push((name.to_string_lossy().to_string(), path));
                    }
                }
            }
        }
        scripts
    }

    pub fn load_script_file(&self, script_path: &str) -> Result<CompiledScript, String> {
        let full_path = self.scripts_folder.join(script_path);

        // Check cache
        if let Ok(cache) = self.cache.read() {
            if let Some(cached) = cache.get(script_path) {
                if let Ok(metadata) = std::fs::metadata(&full_path) {
                    if let Ok(modified) = metadata.modified() {
                        if modified <= cached.last_modified {
                            return Ok(cached.clone());
                        }
                    }
                }
            }
        }

        // Read and compile
        let source = std::fs::read_to_string(&full_path)
            .map_err(|e| format!("Failed to read script '{}': {}", script_path, e))?;

        let ast = self.engine.compile(&source)
            .map_err(|e| format!("Failed to compile script '{}': {}", script_path, e))?;

        let name = Path::new(script_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| script_path.to_string());

        let last_modified = std::fs::metadata(&full_path)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| std::time::SystemTime::now());

        let compiled = CompiledScript {
            ast,
            path: full_path,
            name,
            last_modified,
        };

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(script_path.to_string(), compiled.clone());
        }

        Ok(compiled)
    }

    pub fn run_on_ready(
        &self,
        compiled: &CompiledScript,
        ctx: &mut RhaiScriptContext,
        variables: &HashMap<String, ScriptVariableValue>,
    ) -> Result<(), String> {
        let mut scope = self.create_scope(ctx, variables);
        let result: Result<(), _> = self.engine.call_fn(&mut scope, &compiled.ast, "on_ready", ());
        self.extract_commands(&scope, ctx);
        result.map_err(|e| format!("on_ready error: {}", e))
    }

    pub fn run_on_update(
        &self,
        compiled: &CompiledScript,
        ctx: &mut RhaiScriptContext,
        variables: &HashMap<String, ScriptVariableValue>,
    ) -> Result<(), String> {
        let mut scope = self.create_scope(ctx, variables);
        let result: Result<(), _> = self.engine.call_fn(&mut scope, &compiled.ast, "on_update", ());
        self.extract_commands(&scope, ctx);
        self.extract_context_updates(&scope, ctx);
        result.map_err(|e| format!("on_update error: {}", e))
    }

    fn create_scope(
        &self,
        ctx: &RhaiScriptContext,
        variables: &HashMap<String, ScriptVariableValue>,
    ) -> Scope<'static> {
        let mut scope = Scope::new();

        // Time
        scope.push("delta", ctx.time.delta as f64);
        scope.push("elapsed", ctx.time.elapsed);

        // Transform
        let pos = Map::from([
            ("x".into(), Dynamic::from(ctx.transform.position.x as f64)),
            ("y".into(), Dynamic::from(ctx.transform.position.y as f64)),
            ("z".into(), Dynamic::from(ctx.transform.position.z as f64)),
        ]);
        scope.push("position", pos);

        let rot = Map::from([
            ("x".into(), Dynamic::from(ctx.transform.rotation_euler.x as f64)),
            ("y".into(), Dynamic::from(ctx.transform.rotation_euler.y as f64)),
            ("z".into(), Dynamic::from(ctx.transform.rotation_euler.z as f64)),
        ]);
        scope.push("rotation", rot);

        let scale = Map::from([
            ("x".into(), Dynamic::from(ctx.transform.scale.x as f64)),
            ("y".into(), Dynamic::from(ctx.transform.scale.y as f64)),
            ("z".into(), Dynamic::from(ctx.transform.scale.z as f64)),
        ]);
        scope.push("scale", scale);

        // Input
        let movement = Map::from([
            ("x".into(), Dynamic::from(ctx.input_movement.x as f64)),
            ("y".into(), Dynamic::from(ctx.input_movement.y as f64)),
        ]);
        scope.push("input_movement", movement);

        let mouse_pos = Map::from([
            ("x".into(), Dynamic::from(ctx.mouse_position.x as f64)),
            ("y".into(), Dynamic::from(ctx.mouse_position.y as f64)),
        ]);
        scope.push("mouse_position", mouse_pos);

        let mouse_delta = Map::from([
            ("x".into(), Dynamic::from(ctx.mouse_delta.x as f64)),
            ("y".into(), Dynamic::from(ctx.mouse_delta.y as f64)),
        ]);
        scope.push("mouse_delta", mouse_delta);

        // Gamepad
        let gamepad_left = Map::from([
            ("x".into(), Dynamic::from(ctx.gamepad_left_stick.x as f64)),
            ("y".into(), Dynamic::from(ctx.gamepad_left_stick.y as f64)),
        ]);
        scope.push("gamepad_left_stick", gamepad_left);

        let gamepad_right = Map::from([
            ("x".into(), Dynamic::from(ctx.gamepad_right_stick.x as f64)),
            ("y".into(), Dynamic::from(ctx.gamepad_right_stick.y as f64)),
        ]);
        scope.push("gamepad_right_stick", gamepad_right);

        // Entity info
        scope.push("self_entity", ctx.self_entity_id as i64);
        scope.push("self_name", ctx.self_entity_name.clone());

        // Entity lookups
        let entities_map: Map = ctx.found_entities.iter()
            .map(|(k, v)| (k.clone().into(), Dynamic::from(*v as i64)))
            .collect();
        scope.push("_entities_by_name", entities_map);

        // Collision data
        let collisions_entered: rhai::Array = ctx.collisions_entered.iter()
            .map(|e| Dynamic::from(*e as i64)).collect();
        scope.push("collisions_entered", collisions_entered);

        let active_collisions: rhai::Array = ctx.active_collisions.iter()
            .map(|e| Dynamic::from(*e as i64)).collect();
        scope.push("active_collisions", active_collisions);

        // Timer data
        let timers_finished: rhai::Array = ctx.timers_just_finished.iter()
            .map(|s| Dynamic::from(s.clone())).collect();
        scope.push("timers_just_finished", timers_finished);

        // Commands array
        scope.push("_commands", rhai::Array::new());

        // User variables
        for (name, value) in variables {
            match value {
                ScriptVariableValue::Float(v) => { scope.push(name.clone(), *v); }
                ScriptVariableValue::Int(v) => { scope.push(name.clone(), *v); }
                ScriptVariableValue::Bool(v) => { scope.push(name.clone(), *v); }
                ScriptVariableValue::String(v) => { scope.push(name.clone(), v.clone()); }
                ScriptVariableValue::Vec2(v) => {
                    let map = Map::from([
                        ("x".into(), Dynamic::from(v[0])),
                        ("y".into(), Dynamic::from(v[1])),
                    ]);
                    scope.push(name.clone(), map);
                }
                ScriptVariableValue::Vec3(v) => {
                    let map = Map::from([
                        ("x".into(), Dynamic::from(v[0])),
                        ("y".into(), Dynamic::from(v[1])),
                        ("z".into(), Dynamic::from(v[2])),
                    ]);
                    scope.push(name.clone(), map);
                }
                ScriptVariableValue::Color(v) => {
                    let map = Map::from([
                        ("r".into(), Dynamic::from(v[0])),
                        ("g".into(), Dynamic::from(v[1])),
                        ("b".into(), Dynamic::from(v[2])),
                        ("a".into(), Dynamic::from(v[3])),
                    ]);
                    scope.push(name.clone(), map);
                }
            }
        }

        scope
    }

    fn extract_commands(&self, scope: &Scope, ctx: &mut RhaiScriptContext) {
        if let Some(commands) = scope.get_value::<rhai::Array>("_commands") {
            for cmd_dyn in commands {
                if let Some(cmd) = self.parse_command(&cmd_dyn, ctx.self_entity_id) {
                    ctx.commands.push(cmd);
                }
            }
        }
    }

    fn extract_context_updates(&self, scope: &Scope, ctx: &mut RhaiScriptContext) {
        if let Some(pos) = scope.get_value::<Map>("position") {
            let x = pos.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            let y = pos.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            let z = pos.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            let new_pos = Vec3::new(x, y, z);
            if new_pos != ctx.transform.position {
                ctx.new_position = Some(new_pos);
            }
        }

        if let Some(rot) = scope.get_value::<Map>("rotation") {
            let x = rot.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            let y = rot.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            let z = rot.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            let new_rot = Vec3::new(x, y, z);
            if new_rot != ctx.transform.rotation_euler {
                ctx.new_rotation = Some(Quat::from_euler(EulerRot::XYZ, x.to_radians(), y.to_radians(), z.to_radians()));
            }
        }
    }

    fn parse_command(&self, cmd_dyn: &Dynamic, self_entity_id: u64) -> Option<RhaiCommand> {
        let map = cmd_dyn.clone().try_cast::<Map>()?;
        let cmd_type = map.get("type")?.clone().try_cast::<ImmutableString>()?;

        match cmd_type.as_str() {
            "log" => {
                let msg = map.get("message")?.clone().try_cast::<ImmutableString>()?;
                Some(RhaiCommand::Log { message: msg.to_string() })
            }
            "spawn_entity" => {
                let name = map.get("name")?.clone().try_cast::<ImmutableString>()?;
                Some(RhaiCommand::SpawnEntity { name: name.to_string() })
            }
            "despawn_entity" => {
                let id = map.get("entity")?.clone().try_cast::<i64>()?;
                Some(RhaiCommand::DespawnEntity { entity_id: id as u64 })
            }
            "set_position" => {
                let id = map.get("entity")?.clone().try_cast::<i64>()?;
                let x = map.get("x")?.clone().try_cast::<f64>()? as f32;
                let y = map.get("y")?.clone().try_cast::<f64>()? as f32;
                let z = map.get("z")?.clone().try_cast::<f64>()? as f32;
                Some(RhaiCommand::SetPosition { entity_id: id as u64, position: Vec3::new(x, y, z) })
            }
            "apply_force" => {
                let id = map.get("entity")?.clone().try_cast::<i64>()?;
                let x = map.get("x")?.clone().try_cast::<f64>()? as f32;
                let y = map.get("y")?.clone().try_cast::<f64>()? as f32;
                let z = map.get("z")?.clone().try_cast::<f64>()? as f32;
                Some(RhaiCommand::ApplyForce { entity_id: id as u64, force: Vec3::new(x, y, z) })
            }
            "apply_force_self" => {
                let x = map.get("x")?.clone().try_cast::<f64>()? as f32;
                let y = map.get("y")?.clone().try_cast::<f64>()? as f32;
                let z = map.get("z")?.clone().try_cast::<f64>()? as f32;
                Some(RhaiCommand::ApplyForce { entity_id: self_entity_id, force: Vec3::new(x, y, z) })
            }
            "apply_impulse_self" => {
                let x = map.get("x")?.clone().try_cast::<f64>()? as f32;
                let y = map.get("y")?.clone().try_cast::<f64>()? as f32;
                let z = map.get("z")?.clone().try_cast::<f64>()? as f32;
                Some(RhaiCommand::ApplyImpulse { entity_id: self_entity_id, impulse: Vec3::new(x, y, z) })
            }
            "set_velocity_self" => {
                let x = map.get("x")?.clone().try_cast::<f64>()? as f32;
                let y = map.get("y")?.clone().try_cast::<f64>()? as f32;
                let z = map.get("z")?.clone().try_cast::<f64>()? as f32;
                Some(RhaiCommand::SetVelocity { entity_id: self_entity_id, velocity: Vec3::new(x, y, z) })
            }
            "play_sound" => {
                let path = map.get("path")?.clone().try_cast::<ImmutableString>()?;
                let volume = map.get("volume").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                let looping = map.get("looping").and_then(|v| v.clone().try_cast::<bool>()).unwrap_or(false);
                Some(RhaiCommand::PlaySound { path: path.to_string(), volume, looping })
            }
            "play_music" => {
                let path = map.get("path")?.clone().try_cast::<ImmutableString>()?;
                let volume = map.get("volume").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                let fade_in = map.get("fade_in").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                Some(RhaiCommand::PlayMusic { path: path.to_string(), volume, fade_in })
            }
            "stop_music" => {
                let fade_out = map.get("fade_out").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                Some(RhaiCommand::StopMusic { fade_out })
            }
            "start_timer" => {
                let name = map.get("name")?.clone().try_cast::<ImmutableString>()?;
                let duration = map.get("duration")?.clone().try_cast::<f64>()? as f32;
                let repeat = map.get("repeat").and_then(|v| v.clone().try_cast::<bool>()).unwrap_or(false);
                Some(RhaiCommand::StartTimer { name: name.to_string(), duration, repeat })
            }
            "stop_timer" => {
                let name = map.get("name")?.clone().try_cast::<ImmutableString>()?;
                Some(RhaiCommand::StopTimer { name: name.to_string() })
            }
            "damage" => {
                let id = map.get("entity")?.clone().try_cast::<i64>()?;
                let amount = map.get("amount")?.clone().try_cast::<f64>()? as f32;
                Some(RhaiCommand::Damage { entity_id: id as u64, amount })
            }
            "heal" => {
                let id = map.get("entity")?.clone().try_cast::<i64>()?;
                let amount = map.get("amount")?.clone().try_cast::<f64>()? as f32;
                Some(RhaiCommand::Heal { entity_id: id as u64, amount })
            }
            "kill" => {
                let id = map.get("entity")?.clone().try_cast::<i64>()?;
                Some(RhaiCommand::Kill { entity_id: id as u64 })
            }
            "set_visibility" => {
                let id = map.get("entity")?.clone().try_cast::<i64>()?;
                let visible = map.get("visible")?.clone().try_cast::<bool>()?;
                Some(RhaiCommand::SetVisibility { entity_id: id as u64, visible })
            }
            _ => {
                warn!("Unknown command type: {}", cmd_type);
                None
            }
        }
    }
}
