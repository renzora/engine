use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use rhai::{Dynamic, Engine, AST, Scope, Map, ImmutableString};

use crate::backend::ScriptBackend;
use crate::command::{PropertyValue, ScriptCommand};
use crate::component::{ScriptValue, ScriptVariableDefinition, ScriptVariables};
use crate::context::ScriptContext;

struct CachedScript {
    ast: AST,
    #[allow(dead_code)]
    path: PathBuf,
    name: String,
    last_modified: std::time::SystemTime,
    props: Vec<ScriptVariableDefinition>,
}

use super::{push_command, drain_commands};

pub struct RhaiBackend {
    engine: RwLock<Engine>,
    cache: Arc<RwLock<HashMap<PathBuf, CachedScript>>>,
    scripts_folder: Option<PathBuf>,
    extensions_registered: std::sync::atomic::AtomicBool,
}

impl RhaiBackend {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        register_api(&mut engine);
        Self {
            engine: RwLock::new(engine),
            cache: Arc::new(RwLock::new(HashMap::new())),
            scripts_folder: None,
            extensions_registered: std::sync::atomic::AtomicBool::new(false),
        }
    }

    fn load_script(&self, path: &Path) -> Result<(), String> {
        if let Ok(cache) = self.cache.read() {
            if let Some(cached) = cache.get(path) {
                if let Ok(meta) = std::fs::metadata(path) {
                    if let Ok(modified) = meta.modified() {
                        if modified == cached.last_modified {
                            return Ok(());
                        }
                    }
                }
            }
        }

        let source = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read: {}", e))?;

        let ast = self.engine.read().unwrap().compile(&source)
            .map_err(|e| format!("Compile error: {}", e))?;

        let props = parse_script_props(&self.engine.read().unwrap(), &ast);

        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
        let last_modified = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        if let Ok(mut cache) = self.cache.write() {
            cache.insert(path.to_path_buf(), CachedScript { ast, path: path.to_path_buf(), name, last_modified, props });
        }
        Ok(())
    }

    fn execute_hook(
        &self,
        path: &Path,
        hook: &str,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        self.load_script(path)?;

        let ast = {
            let cache = self.cache.read().map_err(|e| e.to_string())?;
            cache.get(path).ok_or("Not cached")?.ast.clone()
        };

        // Register extension functions once (lazily on first execution)
        if !self.extensions_registered.load(std::sync::atomic::Ordering::Relaxed) {
            if let Some(extensions) = ctx.extensions() {
                extensions.register_rhai_functions(&mut self.engine.write().unwrap());
                self.extensions_registered.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }

        let mut scope = Scope::new();
        setup_scope(&mut scope, ctx, vars);

        // Set up extension scope (per-frame data)
        if let Some(extensions) = ctx.extensions() {
            extensions.setup_rhai_scope(&mut scope, &ctx.extension_data);
        }

        drain_commands();

        match self.engine.read().unwrap().call_fn::<Dynamic>(&mut scope, &ast, hook, ()) {
            Ok(_) => {}
            Err(e) => {
                let err = e.to_string();
                if !err.contains("Function not found") {
                    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                    return Err(format!("{} {}: {}", name, hook, e));
                }
            }
        }

        read_back_variables(&scope, vars);
        Ok(drain_commands())
    }
}

impl ScriptBackend for RhaiBackend {
    fn name(&self) -> &str { "Rhai" }
    fn extensions(&self) -> &[&str] { &["rhai"] }

    fn set_scripts_folder(&mut self, path: PathBuf) {
        self.scripts_folder = Some(path);
    }

    fn get_available_scripts(&self) -> Vec<(String, PathBuf)> {
        let Some(folder) = &self.scripts_folder else { return Vec::new() };
        let mut scripts = Vec::new();
        if let Ok(entries) = std::fs::read_dir(folder) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "rhai") {
                    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
                    scripts.push((name, path));
                }
            }
        }
        scripts
    }

    fn get_script_props(&self, path: &Path) -> Vec<ScriptVariableDefinition> {
        let _ = self.load_script(path);
        self.cache.read().ok()
            .and_then(|c| c.get(path).map(|s| s.props.clone()))
            .unwrap_or_default()
    }

    fn call_on_ready(&self, path: &Path, ctx: &mut ScriptContext, vars: &mut ScriptVariables) -> Result<Vec<ScriptCommand>, String> {
        self.execute_hook(path, "on_ready", ctx, vars)
    }

    fn call_on_update(&self, path: &Path, ctx: &mut ScriptContext, vars: &mut ScriptVariables) -> Result<Vec<ScriptCommand>, String> {
        self.execute_hook(path, "on_update", ctx, vars)
    }

    fn needs_reload(&self, path: &Path) -> bool {
        let cache = match self.cache.read() { Ok(c) => c, Err(_) => return false };
        let Some(cached) = cache.get(path) else { return true };
        let Ok(meta) = std::fs::metadata(path) else { return false };
        let Ok(modified) = meta.modified() else { return false };
        modified != cached.last_modified
    }

    fn reload(&self, path: &Path) -> Result<(), String> {
        if let Ok(mut cache) = self.cache.write() { cache.remove(path); }
        self.load_script(path)
    }

    fn eval_expression(&self, expr: &str) -> Result<String, String> {
        let mut scope = Scope::new();
        drain_commands();
        match self.engine.read().unwrap().eval_with_scope::<Dynamic>(&mut scope, expr) {
            Ok(result) => {
                let _ = drain_commands();
                let s = format!("{}", result);
                Ok(if s == "()" { String::new() } else { s })
            }
            Err(e) => Err(format!("{}", e)),
        }
    }
}

// =============================================================================
// API registration
// =============================================================================

fn register_api(engine: &mut Engine) {
    // Transform
    engine.register_fn("set_position", |x: f64, y: f64, z: f64| { push_command(ScriptCommand::SetPosition { x: x as f32, y: y as f32, z: z as f32 }); });
    engine.register_fn("set_rotation", |x: f64, y: f64, z: f64| { push_command(ScriptCommand::SetRotation { x: x as f32, y: y as f32, z: z as f32 }); });
    engine.register_fn("set_scale", |x: f64, y: f64, z: f64| { push_command(ScriptCommand::SetScale { x: x as f32, y: y as f32, z: z as f32 }); });
    engine.register_fn("set_scale_uniform", |s: f64| { push_command(ScriptCommand::SetScale { x: s as f32, y: s as f32, z: s as f32 }); });
    engine.register_fn("translate", |x: f64, y: f64, z: f64| { push_command(ScriptCommand::Translate { x: x as f32, y: y as f32, z: z as f32 }); });
    engine.register_fn("rotate", |x: f64, y: f64, z: f64| { push_command(ScriptCommand::Rotate { x: x as f32, y: y as f32, z: z as f32 }); });
    engine.register_fn("look_at", |x: f64, y: f64, z: f64| { push_command(ScriptCommand::LookAt { x: x as f32, y: y as f32, z: z as f32 }); });

    // Parent transform
    engine.register_fn("parent_set_position", |x: f64, y: f64, z: f64| { push_command(ScriptCommand::ParentSetPosition { x: x as f32, y: y as f32, z: z as f32 }); });
    engine.register_fn("parent_set_rotation", |x: f64, y: f64, z: f64| { push_command(ScriptCommand::ParentSetRotation { x: x as f32, y: y as f32, z: z as f32 }); });
    engine.register_fn("parent_translate", |x: f64, y: f64, z: f64| { push_command(ScriptCommand::ParentTranslate { x: x as f32, y: y as f32, z: z as f32 }); });

    // Child transform
    engine.register_fn("set_child_position", |name: ImmutableString, x: f64, y: f64, z: f64| { push_command(ScriptCommand::ChildSetPosition { name: name.to_string(), x: x as f32, y: y as f32, z: z as f32 }); });
    engine.register_fn("set_child_rotation", |name: ImmutableString, x: f64, y: f64, z: f64| { push_command(ScriptCommand::ChildSetRotation { name: name.to_string(), x: x as f32, y: y as f32, z: z as f32 }); });
    engine.register_fn("child_translate", |name: ImmutableString, x: f64, y: f64, z: f64| { push_command(ScriptCommand::ChildTranslate { name: name.to_string(), x: x as f32, y: y as f32, z: z as f32 }); });

    // Input
    engine.register_fn("is_key_pressed", |keys_map: Map, key: ImmutableString| -> bool {
        keys_map.get(key.as_str()).and_then(|v| v.clone().try_cast::<bool>()).unwrap_or(false)
    });
    engine.register_fn("is_key_just_pressed", |keys_map: Map, key: ImmutableString| -> bool {
        keys_map.get(key.as_str()).and_then(|v| v.clone().try_cast::<bool>()).unwrap_or(false)
    });
    engine.register_fn("is_key_just_released", |keys_map: Map, key: ImmutableString| -> bool {
        keys_map.get(key.as_str()).and_then(|v| v.clone().try_cast::<bool>()).unwrap_or(false)
    });

    // Audio
    engine.register_fn("play_sound", |path: ImmutableString| {
        push_command(ScriptCommand::PlaySound { path: path.to_string(), volume: 1.0, looping: false, bus: "Sfx".into() });
    });
    engine.register_fn("play_sound_at_volume", |path: ImmutableString, volume: f64| {
        push_command(ScriptCommand::PlaySound { path: path.to_string(), volume: volume as f32, looping: false, bus: "Sfx".into() });
    });
    engine.register_fn("play_sound_looping", |path: ImmutableString, volume: f64| {
        push_command(ScriptCommand::PlaySound { path: path.to_string(), volume: volume as f32, looping: true, bus: "Sfx".into() });
    });
    engine.register_fn("play_music", |path: ImmutableString| {
        push_command(ScriptCommand::PlayMusic { path: path.to_string(), volume: 1.0, fade_in: 0.0, bus: "Music".into() });
    });
    engine.register_fn("stop_music", || { push_command(ScriptCommand::StopMusic { fade_out: 0.0 }); });
    engine.register_fn("stop_all_sounds", || { push_command(ScriptCommand::StopAllSounds); });

    // Physics
    engine.register_fn("apply_force", |x: f64, y: f64, z: f64| {
        push_command(ScriptCommand::ApplyForce { entity_id: None, force: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });
    engine.register_fn("apply_impulse", |x: f64, y: f64, z: f64| {
        push_command(ScriptCommand::ApplyImpulse { entity_id: None, impulse: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });
    engine.register_fn("set_velocity", |x: f64, y: f64, z: f64| {
        push_command(ScriptCommand::SetVelocity { entity_id: None, velocity: bevy::prelude::Vec3::new(x as f32, y as f32, z as f32) });
    });

    // Timers
    engine.register_fn("start_timer", |name: ImmutableString, duration: f64| {
        push_command(ScriptCommand::StartTimer { name: name.to_string(), duration: duration as f32, repeat: false });
    });
    engine.register_fn("start_timer_repeat", |name: ImmutableString, duration: f64| {
        push_command(ScriptCommand::StartTimer { name: name.to_string(), duration: duration as f32, repeat: true });
    });
    engine.register_fn("stop_timer", |name: ImmutableString| {
        push_command(ScriptCommand::StopTimer { name: name.to_string() });
    });

    // Debug
    engine.register_fn("print_log", |msg: ImmutableString| {
        push_command(ScriptCommand::Log { level: "Info".into(), message: msg.to_string() });
    });

    // Rendering
    engine.register_fn("set_visibility", |visible: bool| {
        push_command(ScriptCommand::SetVisibility { entity_id: None, visible });
    });

    // Animation
    engine.register_fn("play_animation", |name: ImmutableString| {
        push_command(ScriptCommand::PlayAnimation { entity_id: None, name: name.to_string(), looping: true, speed: 1.0 });
    });
    engine.register_fn("stop_animation", || {
        push_command(ScriptCommand::StopAnimation { entity_id: None });
    });
    engine.register_fn("pause_animation", || {
        push_command(ScriptCommand::PauseAnimation { entity_id: None });
    });
    engine.register_fn("resume_animation", || {
        push_command(ScriptCommand::ResumeAnimation { entity_id: None });
    });
    engine.register_fn("set_animation_speed", |speed: f64| {
        push_command(ScriptCommand::SetAnimationSpeed { entity_id: None, speed: speed as f32 });
    });
    engine.register_fn("crossfade_animation", |name: ImmutableString, duration: f64| {
        push_command(ScriptCommand::CrossfadeAnimation { entity_id: None, name: name.to_string(), duration: duration as f32, looping: true });
    });
    engine.register_fn("set_anim_param", |name: ImmutableString, value: f64| {
        push_command(ScriptCommand::SetAnimationParam { entity_id: None, name: name.to_string(), value: value as f32 });
    });
    engine.register_fn("set_anim_bool", |name: ImmutableString, value: bool| {
        push_command(ScriptCommand::SetAnimationBoolParam { entity_id: None, name: name.to_string(), value });
    });
    engine.register_fn("trigger_anim", |name: ImmutableString| {
        push_command(ScriptCommand::TriggerAnimation { entity_id: None, name: name.to_string() });
    });
    engine.register_fn("set_layer_weight", |layer_name: ImmutableString, weight: f64| {
        push_command(ScriptCommand::SetAnimationLayerWeight { entity_id: None, layer_name: layer_name.to_string(), weight: weight as f32 });
    });

    // Camera
    engine.register_fn("screen_shake", |intensity: f64, duration: f64| {
        push_command(ScriptCommand::ScreenShake { intensity: intensity as f32, duration: duration as f32 });
    });

    // ECS
    engine.register_fn("spawn_entity", |name: ImmutableString| {
        push_command(ScriptCommand::SpawnEntity { name: name.to_string() });
    });
    engine.register_fn("despawn_self", || {
        push_command(ScriptCommand::DespawnSelf);
    });

    // Environment
    engine.register_fn("set_sun_angles", |azimuth: f64, elevation: f64| {
        push_command(ScriptCommand::SetSunAngles { azimuth: azimuth as f32, elevation: elevation as f32 });
    });
    engine.register_fn("set_fog", |enabled: bool, start: f64, end: f64| {
        push_command(ScriptCommand::SetFog { enabled, start: start as f32, end: end as f32 });
    });

    // Generic reflection API
    engine.register_fn("set", |path: ImmutableString, value: Dynamic| {
        if let Some((comp, field)) = parse_component_path(&path) {
            push_command(ScriptCommand::SetComponentField {
                entity_id: None,
                entity_name: None,
                component_type: comp,
                field_path: field,
                value: rhai_to_property_value(&value),
            });
        }
    });
    engine.register_fn("set_on", |entity: ImmutableString, path: ImmutableString, value: Dynamic| {
        if let Some((comp, field)) = parse_component_path(&path) {
            push_command(ScriptCommand::SetComponentField {
                entity_id: None,
                entity_name: Some(entity.to_string()),
                component_type: comp,
                field_path: field,
                value: rhai_to_property_value(&value),
            });
        }
    });

    // Generic reflection API (get/get_on)
    engine.register_fn("get", |path: ImmutableString| -> Dynamic {
        if let Some((comp, field)) = parse_component_path(&path) {
            if let Some(v) = crate::get_handler::call_get(None, &comp, &field) {
                return property_value_to_dynamic(v);
            }
        }
        Dynamic::UNIT
    });
    engine.register_fn("get_on", |entity: ImmutableString, path: ImmutableString| -> Dynamic {
        if let Some((comp, field)) = parse_component_path(&path) {
            if let Some(v) = crate::get_handler::call_get(Some(entity.as_str()), &comp, &field) {
                return property_value_to_dynamic(v);
            }
        }
        Dynamic::UNIT
    });

    // Math helpers
    engine.register_fn("vec3", |x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });
    engine.register_fn("vec2", |x: f64, y: f64| -> Map {
        let mut m = Map::new();
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m
    });
    engine.register_fn("lerp", |a: f64, b: f64, t: f64| -> f64 {
        a + (b - a) * t
    });
    engine.register_fn("clamp", |v: f64, min: f64, max: f64| -> f64 {
        v.max(min).min(max)
    });
}

// =============================================================================
// Scope setup / read-back
// =============================================================================

fn setup_scope(scope: &mut Scope, ctx: &ScriptContext, vars: &ScriptVariables) {
    scope.push("delta", ctx.time.delta as f64);
    scope.push("elapsed", ctx.time.elapsed);

    scope.push("position_x", ctx.transform.position.x as f64);
    scope.push("position_y", ctx.transform.position.y as f64);
    scope.push("position_z", ctx.transform.position.z as f64);

    let euler = ctx.transform.euler_degrees();
    scope.push("rotation_x", euler.x as f64);
    scope.push("rotation_y", euler.y as f64);
    scope.push("rotation_z", euler.z as f64);

    scope.push("scale_x", ctx.transform.scale.x as f64);
    scope.push("scale_y", ctx.transform.scale.y as f64);
    scope.push("scale_z", ctx.transform.scale.z as f64);

    scope.push("input_x", ctx.input_movement.x as f64);
    scope.push("input_y", ctx.input_movement.y as f64);
    scope.push("mouse_x", ctx.mouse_position.x as f64);
    scope.push("mouse_y", ctx.mouse_position.y as f64);
    scope.push("mouse_delta_x", ctx.mouse_delta.x as f64);
    scope.push("mouse_delta_y", ctx.mouse_delta.y as f64);
    scope.push("camera_yaw", ctx.camera_yaw as f64);

    // Gamepad
    scope.push("gamepad_left_x", ctx.gamepad_left_stick.x as f64);
    scope.push("gamepad_left_y", ctx.gamepad_left_stick.y as f64);
    scope.push("gamepad_right_x", ctx.gamepad_right_stick.x as f64);
    scope.push("gamepad_right_y", ctx.gamepad_right_stick.y as f64);
    scope.push("gamepad_left_trigger", ctx.gamepad_left_trigger as f64);
    scope.push("gamepad_right_trigger", ctx.gamepad_right_trigger as f64);
    scope.push("gamepad_south", ctx.gamepad_buttons[0]);
    scope.push("gamepad_east", ctx.gamepad_buttons[1]);
    scope.push("gamepad_west", ctx.gamepad_buttons[2]);
    scope.push("gamepad_north", ctx.gamepad_buttons[3]);
    scope.push("gamepad_l1", ctx.gamepad_buttons[4]);
    scope.push("gamepad_r1", ctx.gamepad_buttons[5]);
    scope.push("gamepad_l2", ctx.gamepad_buttons[6]);
    scope.push("gamepad_r2", ctx.gamepad_buttons[7]);
    scope.push("gamepad_select", ctx.gamepad_buttons[8]);
    scope.push("gamepad_start", ctx.gamepad_buttons[9]);
    scope.push("gamepad_l3", ctx.gamepad_buttons[10]);
    scope.push("gamepad_r3", ctx.gamepad_buttons[11]);
    scope.push("gamepad_dpad_up", ctx.gamepad_buttons[12]);
    scope.push("gamepad_dpad_down", ctx.gamepad_buttons[13]);
    scope.push("gamepad_dpad_left", ctx.gamepad_buttons[14]);
    scope.push("gamepad_dpad_right", ctx.gamepad_buttons[15]);

    // Mouse
    scope.push("mouse_left", ctx.mouse_buttons_pressed[0]);
    scope.push("mouse_right", ctx.mouse_buttons_pressed[1]);
    scope.push("mouse_middle", ctx.mouse_buttons_pressed[2]);
    scope.push("mouse_left_just_pressed", ctx.mouse_buttons_just_pressed[0]);
    scope.push("mouse_right_just_pressed", ctx.mouse_buttons_just_pressed[1]);
    scope.push("mouse_scroll", ctx.mouse_scroll as f64);

    // Entity
    scope.push("self_entity_id", ctx.self_entity_id as i64);
    scope.push("self_entity_name", ctx.self_entity_name.clone());

    // Keyboard maps
    let mut keys_pressed_map = Map::new();
    for (key, &pressed) in &ctx.keys_pressed {
        keys_pressed_map.insert(key.clone().into(), Dynamic::from(pressed));
    }
    scope.push("_keys_pressed", keys_pressed_map);

    let mut keys_just_pressed_map = Map::new();
    for (key, &pressed) in &ctx.keys_just_pressed {
        keys_just_pressed_map.insert(key.clone().into(), Dynamic::from(pressed));
    }
    scope.push("_keys_just_pressed", keys_just_pressed_map);

    let mut keys_just_released_map = Map::new();
    for (key, &released) in &ctx.keys_just_released {
        keys_just_released_map.insert(key.clone().into(), Dynamic::from(released));
    }
    scope.push("_keys_just_released", keys_just_released_map);

    // Collisions
    scope.push("is_colliding", !ctx.active_collisions.is_empty());

    // Timers
    let timers: rhai::Array = ctx.timers_just_finished.iter().map(|n| Dynamic::from(n.clone())).collect();
    scope.push("timers_finished", timers);

    // Health
    scope.push("self_health", ctx.self_health as f64);
    scope.push("self_max_health", ctx.self_max_health as f64);

    // Parent
    scope.push("has_parent", ctx.has_parent);
    scope.push("parent_position_x", ctx.parent_position.x as f64);
    scope.push("parent_position_y", ctx.parent_position.y as f64);
    scope.push("parent_position_z", ctx.parent_position.z as f64);

    // Script variables
    for (key, value) in vars.iter_all() {
        let dyn_val = match value {
            ScriptValue::Float(v) => Dynamic::from(*v as f64),
            ScriptValue::Int(v) => Dynamic::from(*v as i64),
            ScriptValue::Bool(v) => Dynamic::from(*v),
            ScriptValue::String(v) => Dynamic::from(v.clone()),
            ScriptValue::Entity(v) => Dynamic::from(v.clone()),
            ScriptValue::Vec2(v) => {
                let mut m = Map::new();
                m.insert("x".into(), Dynamic::from(v.x as f64));
                m.insert("y".into(), Dynamic::from(v.y as f64));
                Dynamic::from(m)
            }
            ScriptValue::Vec3(v) => {
                let mut m = Map::new();
                m.insert("x".into(), Dynamic::from(v.x as f64));
                m.insert("y".into(), Dynamic::from(v.y as f64));
                m.insert("z".into(), Dynamic::from(v.z as f64));
                Dynamic::from(m)
            }
            ScriptValue::Color(v) => {
                let mut m = Map::new();
                m.insert("r".into(), Dynamic::from(v.x as f64));
                m.insert("g".into(), Dynamic::from(v.y as f64));
                m.insert("b".into(), Dynamic::from(v.z as f64));
                m.insert("a".into(), Dynamic::from(v.w as f64));
                Dynamic::from(m)
            }
        };
        scope.push(key.as_str(), dyn_val);
    }
}

fn read_back_variables(scope: &Scope, vars: &mut ScriptVariables) {
    let var_names: Vec<String> = vars.iter_all().map(|(k, _)| k.clone()).collect();
    for name in &var_names {
        if let Some(value) = scope.get_value::<Dynamic>(name) {
            if let Some(sv) = dynamic_to_script_value(&value) {
                vars.set(name.clone(), sv);
            }
        }
    }
}

fn dynamic_to_script_value(value: &Dynamic) -> Option<ScriptValue> {
    if let Some(v) = value.clone().try_cast::<f64>() { return Some(ScriptValue::Float(v as f32)); }
    if let Some(v) = value.clone().try_cast::<i64>() { return Some(ScriptValue::Int(v as i32)); }
    if let Some(v) = value.clone().try_cast::<bool>() { return Some(ScriptValue::Bool(v)); }
    if let Some(v) = value.clone().try_cast::<ImmutableString>() { return Some(ScriptValue::String(v.to_string())); }
    if let Some(map) = value.clone().try_cast::<Map>() {
        if map.contains_key("x") && map.contains_key("y") && !map.contains_key("z") {
            let x = map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            let y = map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            return Some(ScriptValue::Vec2(bevy::prelude::Vec2::new(x, y)));
        }
        if map.contains_key("x") && map.contains_key("y") && map.contains_key("z") {
            let x = map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            let y = map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            let z = map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            return Some(ScriptValue::Vec3(bevy::prelude::Vec3::new(x, y, z)));
        }
        if map.contains_key("r") && map.contains_key("g") && map.contains_key("b") {
            let r = map.get("r").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
            let g = map.get("g").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
            let b = map.get("b").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
            let a = map.get("a").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
            return Some(ScriptValue::Color(bevy::prelude::Vec4::new(r, g, b, a)));
        }
    }
    None
}

fn parse_script_props(engine: &Engine, ast: &AST) -> Vec<ScriptVariableDefinition> {
    let mut scope = Scope::new();
    let result: Result<Dynamic, _> = engine.call_fn(&mut scope, ast, "props", ());
    let Ok(result) = result else { return Vec::new() };
    let Some(map) = result.try_cast::<Map>() else { return Vec::new() };

    let mut props = Vec::new();
    for (key, value) in map.iter() {
        let name = key.to_string();
        let display_name = to_display_name(&name);

        if let Some(prop_map) = value.clone().try_cast::<Map>() {
            let default_val = prop_map.get("value").or_else(|| prop_map.get("default"));
            if let Some(default_val) = default_val {
                if let Some(sv) = dynamic_to_script_value(default_val) {
                    let hint = prop_map.get("hint")
                        .and_then(|v| v.clone().try_cast::<ImmutableString>())
                        .map(|s| s.to_string());
                    let mut def = ScriptVariableDefinition::new(name.clone(), sv).with_display_name(display_name.clone());
                    if let Some(h) = hint { def = def.with_hint(h); }
                    props.push(def);
                    continue;
                }
            }
        }

        if let Some(sv) = dynamic_to_script_value(value) {
            props.push(ScriptVariableDefinition::new(name, sv).with_display_name(display_name));
        }
    }

    props.sort_by(|a, b| a.name.cmp(&b.name));
    props
}

fn parse_component_path(path: &str) -> Option<(String, String)> {
    let dot = path.find('.')?;
    let component = path[..dot].to_string();
    let field = path[dot + 1..].to_string();
    if component.is_empty() || field.is_empty() { return None; }
    Some((component, field))
}

fn rhai_to_property_value(value: &Dynamic) -> PropertyValue {
    if let Some(v) = value.clone().try_cast::<f64>() {
        return PropertyValue::Float(v as f32);
    }
    if let Some(v) = value.clone().try_cast::<i64>() {
        return PropertyValue::Int(v);
    }
    if let Some(v) = value.clone().try_cast::<bool>() {
        return PropertyValue::Bool(v);
    }
    if let Some(v) = value.clone().try_cast::<ImmutableString>() {
        return PropertyValue::String(v.to_string());
    }
    PropertyValue::Float(0.0)
}

fn property_value_to_dynamic(value: PropertyValue) -> Dynamic {
    match value {
        PropertyValue::Float(v) => Dynamic::from(v as f64),
        PropertyValue::Int(v) => Dynamic::from(v),
        PropertyValue::Bool(v) => Dynamic::from(v),
        PropertyValue::String(v) => Dynamic::from(v),
        PropertyValue::Vec3(v) => {
            let mut m = Map::new();
            m.insert("x".into(), Dynamic::from(v[0] as f64));
            m.insert("y".into(), Dynamic::from(v[1] as f64));
            m.insert("z".into(), Dynamic::from(v[2] as f64));
            Dynamic::from(m)
        }
        PropertyValue::Color(v) => {
            let mut m = Map::new();
            m.insert("r".into(), Dynamic::from(v[0] as f64));
            m.insert("g".into(), Dynamic::from(v[1] as f64));
            m.insert("b".into(), Dynamic::from(v[2] as f64));
            m.insert("a".into(), Dynamic::from(v[3] as f64));
            Dynamic::from(m)
        }
    }
}


fn to_display_name(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
