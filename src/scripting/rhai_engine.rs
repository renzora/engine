//! Rhai scripting engine integration
//!
//! Core engine for loading, compiling, and executing Rhai scripts.
//! Supports both .rhai files and .blueprint files (compiled to Rhai).

#![allow(dead_code)]

use bevy::prelude::*;
use rhai::{Dynamic, Engine, AST, Scope, Map, ImmutableString};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use super::{ScriptValue, ScriptVariables, ScriptVariableDefinition};
use super::rhai_api;
use super::rhai_context::{RhaiScriptContext, ChildChange};
use super::rhai_commands::{RhaiCommand, ComponentValue};
use crate::blueprint::{BlueprintFile, generate_rhai_code};
use crate::core::resources::console::{console_log, LogLevel};

/// Cached compiled Rhai script
#[derive(Clone)]
pub struct CompiledScript {
    pub ast: AST,
    pub path: PathBuf,
    pub name: String,
    pub last_modified: std::time::SystemTime,
    /// Script-defined props (from props() function)
    pub props: Vec<ScriptVariableDefinition>,
}

/// Create nice display name from snake_case
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

/// Convert a Rhai Dynamic value to ScriptValue
fn dynamic_to_script_value(value: &Dynamic) -> Option<ScriptValue> {
    if let Some(v) = value.clone().try_cast::<f64>() {
        return Some(ScriptValue::Float(v as f32));
    }
    if let Some(v) = value.clone().try_cast::<i64>() {
        return Some(ScriptValue::Int(v as i32));
    }
    if let Some(v) = value.clone().try_cast::<bool>() {
        return Some(ScriptValue::Bool(v));
    }
    if let Some(v) = value.clone().try_cast::<ImmutableString>() {
        return Some(ScriptValue::String(v.to_string()));
    }
    if let Some(map) = value.clone().try_cast::<Map>() {
        // Check if it's a vec2
        if map.contains_key("x") && map.contains_key("y") && !map.contains_key("z") {
            let x = map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            let y = map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            return Some(ScriptValue::Vec2(Vec2::new(x, y)));
        }
        // Check if it's a vec3
        if map.contains_key("x") && map.contains_key("y") && map.contains_key("z") && !map.contains_key("w") {
            let x = map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            let y = map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            let z = map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
            return Some(ScriptValue::Vec3(Vec3::new(x, y, z)));
        }
        // Check if it's a color (r,g,b,a)
        if map.contains_key("r") && map.contains_key("g") && map.contains_key("b") {
            let r = map.get("r").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
            let g = map.get("g").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
            let b = map.get("b").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
            let a = map.get("a").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
            return Some(ScriptValue::Color(Vec4::new(r, g, b, a)));
        }
    }
    None
}

/// Parse props by executing the props() function in the script
fn parse_script_props(engine: &Engine, ast: &AST) -> Vec<ScriptVariableDefinition> {
    let mut props = Vec::new();
    let mut scope = Scope::new();

    let result: Result<Dynamic, _> = engine.call_fn(&mut scope, ast, "props", ());
    let result = match result {
        Ok(r) => r,
        Err(_e) => {
            // No props() function is fine - most scripts won't have one
            return props;
        }
    };
    let Some(map) = result.try_cast::<Map>() else {
        console_log(LogLevel::Warning, "Script", "props() did not return a map");
        return props;
    };

    for (key, value) in map.iter() {
        let name = key.to_string();
        let display_name = to_display_name(&name);

        // Check if value is a map with 'value' (or 'default' for backwards compat) and optional 'hint'
        if let Some(prop_map) = value.clone().try_cast::<Map>() {
            // Try "value" first, then "default" for backwards compatibility
            let default_val = prop_map.get("value").or_else(|| prop_map.get("default"));

            if let Some(default_val) = default_val {
                let hint = prop_map.get("hint")
                    .and_then(|v| v.clone().try_cast::<ImmutableString>())
                    .map(|s| s.to_string());

                if let Some(script_value) = dynamic_to_script_value(default_val) {
                    let mut def = ScriptVariableDefinition::new(name.clone(), script_value)
                        .with_display_name(display_name);
                    if let Some(h) = hint {
                        def = def.with_hint(h);
                    }
                    props.push(def);
                } else {
                    console_log(LogLevel::Warning, "Script", format!("Could not convert default value for prop '{}'", name));
                }
                continue;
            }
        }

        if let Some(script_value) = dynamic_to_script_value(value) {
            let def = ScriptVariableDefinition::new(name, script_value)
                .with_display_name(display_name);
            props.push(def);
        }
    }

    props.sort_by(|a, b| a.name.cmp(&b.name));
    props
}

/// Rhai script engine resource
#[derive(Resource)]
pub struct RhaiScriptEngine {
    engine: Engine,
    compiled_scripts: Arc<RwLock<HashMap<PathBuf, CompiledScript>>>,
    scripts_folder: Option<PathBuf>,
}

impl Default for RhaiScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RhaiScriptEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();

        // Register all API functions from modular modules
        rhai_api::register_all(&mut engine);

        Self {
            engine,
            compiled_scripts: Arc::new(RwLock::new(HashMap::new())),
            scripts_folder: None,
        }
    }

    /// Set the scripts folder path
    pub fn set_scripts_folder(&mut self, path: PathBuf) {
        self.scripts_folder = Some(path);
    }

    /// Get available script files from the scripts folder (both .rhai and .blueprint)
    pub fn get_available_scripts(&self) -> Vec<(String, PathBuf)> {
        let Some(folder) = &self.scripts_folder else {
            return Vec::new();
        };

        let mut scripts = Vec::new();

        // Check scripts folder for .rhai and .blueprint files
        if let Ok(entries) = std::fs::read_dir(folder) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_valid = path.extension().map_or(false, |e| e == "rhai" || e == "blueprint");
                if is_valid {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                    // Show extension in name to distinguish types
                    let name = if ext == "blueprint" {
                        format!("{} (Blueprint)", stem)
                    } else {
                        stem.to_string()
                    };
                    scripts.push((name, path));
                }
            }
        }

        // Also check blueprints folder (sibling to scripts folder)
        if let Some(parent) = folder.parent() {
            let blueprints_folder = parent.join("blueprints");
            if let Ok(entries) = std::fs::read_dir(&blueprints_folder) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |e| e == "blueprint") {
                        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                        let name = format!("{} (Blueprint)", stem);
                        scripts.push((name, path));
                    }
                }
            }
        }

        scripts.sort_by(|a, b| a.0.cmp(&b.0));
        scripts
    }

    /// Get the props defined in a script file (supports .rhai and .blueprint)
    pub fn get_script_props(&self, path: &Path) -> Vec<ScriptVariableDefinition> {
        if let Ok(compiled) = self.load_script_file(path) {
            compiled.props
        } else {
            Vec::new()
        }
    }

    /// Parse props from in-memory script source (for live preview)
    pub fn get_props_from_source(&self, source: &str) -> Vec<ScriptVariableDefinition> {
        match self.engine.compile(source) {
            Ok(ast) => parse_script_props(&self.engine, &ast),
            Err(_) => Vec::new(),
        }
    }

    /// Load and compile a script from file
    pub fn load_script(&self, path: &Path) -> Result<CompiledScript, String> {
        // Check if already compiled and up to date
        if let Ok(scripts) = self.compiled_scripts.read() {
            if let Some(cached) = scripts.get(path) {
                if let Ok(metadata) = std::fs::metadata(path) {
                    if let Ok(modified) = metadata.modified() {
                        if modified == cached.last_modified {
                            return Ok(cached.clone());
                        }
                    }
                }
            }
        }

        let source = std::fs::read_to_string(path)
            .map_err(|e| {
                let msg = format!("Failed to read script '{}': {}", path.display(), e);
                console_log(LogLevel::Error, "Script", &msg);
                msg
            })?;

        let ast = self.engine.compile(&source)
            .map_err(|e| {
                let script_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
                let msg = format!("{}: {}", script_name, e);
                console_log(LogLevel::Error, "Script", &msg);
                format!("Failed to compile script: {}", e)
            })?;

        let props = parse_script_props(&self.engine, &ast);

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let last_modified = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        let compiled = CompiledScript { ast, path: path.to_path_buf(), name, last_modified, props };

        if let Ok(mut scripts) = self.compiled_scripts.write() {
            scripts.insert(path.to_path_buf(), compiled.clone());
        }

        Ok(compiled)
    }

    /// Reload a script if it has changed
    pub fn reload_if_changed(&self, path: &Path) -> Option<CompiledScript> {
        let needs_reload = if let Ok(scripts) = self.compiled_scripts.read() {
            if let Some(cached) = scripts.get(path) {
                if let Ok(metadata) = std::fs::metadata(path) {
                    if let Ok(modified) = metadata.modified() {
                        modified != cached.last_modified
                    } else { false }
                } else { false }
            } else { true }
        } else { false };

        if needs_reload { self.load_script_file(path).ok() } else { None }
    }

    /// Load and compile a blueprint file (.blueprint) to Rhai
    pub fn load_blueprint(&self, path: &Path) -> Result<CompiledScript, String> {
        // Check if already compiled and up to date
        if let Ok(scripts) = self.compiled_scripts.read() {
            if let Some(cached) = scripts.get(path) {
                if let Ok(metadata) = std::fs::metadata(path) {
                    if let Ok(modified) = metadata.modified() {
                        if modified == cached.last_modified {
                            return Ok(cached.clone());
                        }
                    }
                }
            }
        }

        // Load the blueprint file
        let blueprint_file = BlueprintFile::load(path)
            .map_err(|e| format!("Failed to load blueprint: {}", e))?;

        // Generate Rhai code from the blueprint graph
        let codegen_result = generate_rhai_code(&blueprint_file.graph);

        if !codegen_result.errors.is_empty() {
            return Err(format!("Blueprint compilation errors: {:?}", codegen_result.errors));
        }

        // Log warnings
        for warning in &codegen_result.warnings {
            bevy::log::warn!("[Blueprint] {}: {}", path.display(), warning);
        }

        // Compile the generated Rhai code
        let ast = self.engine.compile(&codegen_result.code)
            .map_err(|e| format!("Failed to compile generated Rhai code: {}", e))?;

        let props = parse_script_props(&self.engine, &ast);

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let last_modified = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        let compiled = CompiledScript { ast, path: path.to_path_buf(), name, last_modified, props };

        if let Ok(mut scripts) = self.compiled_scripts.write() {
            scripts.insert(path.to_path_buf(), compiled.clone());
        }

        Ok(compiled)
    }

    /// Load and compile a script file (auto-detects .rhai or .blueprint)
    pub fn load_script_file(&self, path: &Path) -> Result<CompiledScript, String> {
        let extension = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match extension {
            "blueprint" => self.load_blueprint(path),
            "rhai" | _ => self.load_script(path),
        }
    }

    /// Execute on_ready function
    pub fn call_on_ready(
        &self,
        script: &CompiledScript,
        ctx: &mut RhaiScriptContext,
        vars: &ScriptVariables,
    ) {
        let mut scope = Scope::new();
        self.setup_scope(&mut scope, ctx, vars);

        match self.engine.call_fn::<()>(&mut scope, &script.ast, "on_ready", ()) {
            Ok(_) => {}
            Err(e) => {
                // Check if it's just a missing function (which is OK)
                let err_str = e.to_string();
                if !err_str.contains("Function not found") {
                    console_log(LogLevel::Error, "Script", format!("{} on_ready: {}", script.name, e));
                }
            }
        }

        self.read_scope_changes(&scope, ctx);
    }

    /// Execute on_update function
    pub fn call_on_update(
        &self,
        script: &CompiledScript,
        ctx: &mut RhaiScriptContext,
        vars: &ScriptVariables,
    ) {
        let mut scope = Scope::new();
        self.setup_scope(&mut scope, ctx, vars);

        match self.engine.call_fn::<()>(&mut scope, &script.ast, "on_update", ()) {
            Ok(_) => {}
            Err(e) => {
                let err_str = e.to_string();
                if !err_str.contains("Function not found") {
                    console_log(LogLevel::Error, "Script", format!("{} on_update: {}", script.name, e));
                }
            }
        }

        self.read_scope_changes(&scope, ctx);
    }

    fn setup_scope(&self, scope: &mut Scope, ctx: &RhaiScriptContext, vars: &ScriptVariables) {
        // Time
        scope.push("delta", ctx.time.delta as f64);
        scope.push("elapsed", ctx.time.elapsed);

        // Transform
        scope.push("position_x", ctx.transform.position.x as f64);
        scope.push("position_y", ctx.transform.position.y as f64);
        scope.push("position_z", ctx.transform.position.z as f64);

        let euler = ctx.transform.euler_angles_degrees();
        scope.push("rotation_x", euler.x as f64);
        scope.push("rotation_y", euler.y as f64);
        scope.push("rotation_z", euler.z as f64);

        scope.push("scale_x", ctx.transform.scale.x as f64);
        scope.push("scale_y", ctx.transform.scale.y as f64);
        scope.push("scale_z", ctx.transform.scale.z as f64);

        // Input movement
        scope.push("input_x", ctx.input_movement.x as f64);
        scope.push("input_y", ctx.input_movement.y as f64);
        scope.push("mouse_x", ctx.mouse_position.x as f64);
        scope.push("mouse_y", ctx.mouse_position.y as f64);
        scope.push("mouse_delta_x", ctx.mouse_delta.x as f64);
        scope.push("mouse_delta_y", ctx.mouse_delta.y as f64);

        // Gamepad
        scope.push("gamepad_left_x", ctx.gamepad_left_stick.x as f64);
        scope.push("gamepad_left_y", ctx.gamepad_left_stick.y as f64);
        scope.push("gamepad_right_x", ctx.gamepad_right_stick.x as f64);
        scope.push("gamepad_right_y", ctx.gamepad_right_stick.y as f64);
        scope.push("gamepad_left_trigger", ctx.gamepad_left_trigger as f64);
        scope.push("gamepad_right_trigger", ctx.gamepad_right_trigger as f64);
        scope.push("gamepad_a", ctx.gamepad_buttons[0]);
        scope.push("gamepad_b", ctx.gamepad_buttons[1]);
        scope.push("gamepad_x", ctx.gamepad_buttons[2]);
        scope.push("gamepad_y", ctx.gamepad_buttons[3]);
        scope.push("gamepad_lb", ctx.gamepad_buttons[4]);
        scope.push("gamepad_rb", ctx.gamepad_buttons[5]);
        scope.push("gamepad_select", ctx.gamepad_buttons[6]);
        scope.push("gamepad_start", ctx.gamepad_buttons[7]);
        scope.push("gamepad_l3", ctx.gamepad_buttons[8]);
        scope.push("gamepad_r3", ctx.gamepad_buttons[9]);
        scope.push("gamepad_dpad_up", ctx.gamepad_buttons[10]);
        scope.push("gamepad_dpad_down", ctx.gamepad_buttons[11]);
        scope.push("gamepad_dpad_left", ctx.gamepad_buttons[12]);
        scope.push("gamepad_dpad_right", ctx.gamepad_buttons[13]);

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

        // Mouse buttons
        scope.push("mouse_left", ctx.mouse_buttons_pressed[0]);
        scope.push("mouse_right", ctx.mouse_buttons_pressed[1]);
        scope.push("mouse_middle", ctx.mouse_buttons_pressed[2]);
        scope.push("mouse_left_just_pressed", ctx.mouse_buttons_just_pressed[0]);
        scope.push("mouse_right_just_pressed", ctx.mouse_buttons_just_pressed[1]);
        scope.push("mouse_middle_just_pressed", ctx.mouse_buttons_just_pressed[2]);
        scope.push("mouse_scroll", ctx.mouse_scroll as f64);

        // Self entity
        scope.push("self_entity_id", ctx.self_entity_id as i64);
        scope.push("self_entity_name", ctx.self_entity_name.clone());

        // Found entities (name → entity_id)
        let mut found_entities_map = Map::new();
        for (name, &id) in &ctx.found_entities {
            found_entities_map.insert(name.clone().into(), Dynamic::from(id as i64));
        }
        scope.push("_found_entities", found_entities_map);

        // Entities by tag (tag → array of entity_ids)
        let mut entities_by_tag_map = Map::new();
        for (tag, ids) in &ctx.entities_by_tag {
            let ids_array: rhai::Array = ids.iter().map(|&id| Dynamic::from(id as i64)).collect();
            entities_by_tag_map.insert(tag.clone().into(), Dynamic::from(ids_array));
        }
        scope.push("_entities_by_tag", entities_by_tag_map);

        // Collision events
        let collisions_entered: rhai::Array = ctx.collisions_entered.iter()
            .map(|&id| Dynamic::from(id as i64))
            .collect();
        scope.push("collisions_entered", collisions_entered);

        let collisions_exited: rhai::Array = ctx.collisions_exited.iter()
            .map(|&id| Dynamic::from(id as i64))
            .collect();
        scope.push("collisions_exited", collisions_exited);

        let active_collisions: rhai::Array = ctx.active_collisions.iter()
            .map(|&id| Dynamic::from(id as i64))
            .collect();
        scope.push("active_collisions", active_collisions);

        // Check if entity is colliding with anything (convenient bool)
        scope.push("is_colliding", !ctx.active_collisions.is_empty());

        // Timer data - list of timer names that just finished
        let timers_finished: rhai::Array = ctx.timers_just_finished.iter()
            .map(|name| Dynamic::from(name.clone()))
            .collect();
        scope.push("timers_finished", timers_finished);

        // Helper: Check if any timer just finished
        scope.push("any_timer_finished", !ctx.timers_just_finished.is_empty());

        // Raycast results - push each result as a map with the variable name
        for (var_name, hit) in &ctx.raycast_results {
            let mut hit_map = Map::new();
            hit_map.insert("hit".into(), Dynamic::from(hit.hit));
            hit_map.insert("entity_id".into(), Dynamic::from(hit.entity.map(|e| e.to_bits() as i64).unwrap_or(-1)));
            hit_map.insert("point_x".into(), Dynamic::from(hit.point.x as f64));
            hit_map.insert("point_y".into(), Dynamic::from(hit.point.y as f64));
            hit_map.insert("point_z".into(), Dynamic::from(hit.point.z as f64));
            hit_map.insert("normal_x".into(), Dynamic::from(hit.normal.x as f64));
            hit_map.insert("normal_y".into(), Dynamic::from(hit.normal.y as f64));
            hit_map.insert("normal_z".into(), Dynamic::from(hit.normal.z as f64));
            hit_map.insert("distance".into(), Dynamic::from(hit.distance as f64));
            scope.push(var_name.as_str(), hit_map);
        }

        // Component data - Health
        scope.push("self_health", ctx.self_health as f64);
        scope.push("self_max_health", ctx.self_max_health as f64);
        scope.push("self_health_percent", ctx.self_health_percent as f64);
        scope.push("self_is_invincible", ctx.self_is_invincible);

        // Component data - Light
        scope.push("self_light_intensity", ctx.self_light_intensity as f64);
        scope.push("self_light_color_r", ctx.self_light_color[0] as f64);
        scope.push("self_light_color_g", ctx.self_light_color[1] as f64);
        scope.push("self_light_color_b", ctx.self_light_color[2] as f64);

        // Component data - Material
        scope.push("self_material_color_r", ctx.self_material_color[0] as f64);
        scope.push("self_material_color_g", ctx.self_material_color[1] as f64);
        scope.push("self_material_color_b", ctx.self_material_color[2] as f64);
        scope.push("self_material_color_a", ctx.self_material_color[3] as f64);

        // Variables
        let mut var_map = Map::new();
        for (key, value) in vars.iter_all() {
            let dyn_val = match value {
                ScriptValue::Float(v) => Dynamic::from(*v as f64),
                ScriptValue::Int(v) => Dynamic::from(*v),
                ScriptValue::Bool(v) => Dynamic::from(*v),
                ScriptValue::String(v) => Dynamic::from(v.clone()),
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
            var_map.insert(key.clone().into(), dyn_val);
        }
        scope.push("vars", var_map);

        // Output flags
        scope.push("_set_position", false);
        scope.push("_new_position_x", 0.0_f64);
        scope.push("_new_position_y", 0.0_f64);
        scope.push("_new_position_z", 0.0_f64);

        scope.push("_set_rotation", false);
        scope.push("_new_rotation_x", 0.0_f64);
        scope.push("_new_rotation_y", 0.0_f64);
        scope.push("_new_rotation_z", 0.0_f64);

        scope.push("_translate", false);
        scope.push("_translate_x", 0.0_f64);
        scope.push("_translate_y", 0.0_f64);
        scope.push("_translate_z", 0.0_f64);

        scope.push("_rotate", false);
        scope.push("_rotate_x", 0.0_f64);
        scope.push("_rotate_y", 0.0_f64);
        scope.push("_rotate_z", 0.0_f64);

        scope.push("_print_message", ImmutableString::new());

        // Parent info
        scope.push("has_parent", ctx.has_parent);
        scope.push("parent_position_x", ctx.parent_position.x as f64);
        scope.push("parent_position_y", ctx.parent_position.y as f64);
        scope.push("parent_position_z", ctx.parent_position.z as f64);
        scope.push("parent_rotation_x", ctx.parent_rotation.x as f64);
        scope.push("parent_rotation_y", ctx.parent_rotation.y as f64);
        scope.push("parent_rotation_z", ctx.parent_rotation.z as f64);
        scope.push("parent_scale_x", ctx.parent_scale.x as f64);
        scope.push("parent_scale_y", ctx.parent_scale.y as f64);
        scope.push("parent_scale_z", ctx.parent_scale.z as f64);

        scope.push("_parent_set_position", false);
        scope.push("_parent_new_position_x", 0.0_f64);
        scope.push("_parent_new_position_y", 0.0_f64);
        scope.push("_parent_new_position_z", 0.0_f64);
        scope.push("_parent_set_rotation", false);
        scope.push("_parent_new_rotation_x", 0.0_f64);
        scope.push("_parent_new_rotation_y", 0.0_f64);
        scope.push("_parent_new_rotation_z", 0.0_f64);
        scope.push("_parent_translate", false);
        scope.push("_parent_translate_x", 0.0_f64);
        scope.push("_parent_translate_y", 0.0_f64);
        scope.push("_parent_translate_z", 0.0_f64);

        // Children
        let mut dollar_map = Map::new();
        let mut children_names: Vec<Dynamic> = Vec::new();
        for child in &ctx.children {
            let mut child_map = Map::new();
            child_map.insert("position_x".into(), Dynamic::from(child.position.x as f64));
            child_map.insert("position_y".into(), Dynamic::from(child.position.y as f64));
            child_map.insert("position_z".into(), Dynamic::from(child.position.z as f64));
            child_map.insert("rotation_x".into(), Dynamic::from(child.rotation.x as f64));
            child_map.insert("rotation_y".into(), Dynamic::from(child.rotation.y as f64));
            child_map.insert("rotation_z".into(), Dynamic::from(child.rotation.z as f64));
            child_map.insert("scale_x".into(), Dynamic::from(child.scale.x as f64));
            child_map.insert("scale_y".into(), Dynamic::from(child.scale.y as f64));
            child_map.insert("scale_z".into(), Dynamic::from(child.scale.z as f64));
            child_map.insert("name".into(), Dynamic::from(child.name.clone()));
            dollar_map.insert(child.name.clone().into(), Dynamic::from(child_map));
            children_names.push(Dynamic::from(child.name.clone()));
        }
        scope.push("$", dollar_map);
        scope.push("children", children_names);
        scope.push("_child_commands", rhai::Array::new());

        // Environment - General
        scope.push("_set_sky_mode", false);
        scope.push("_sky_mode", 1_i64);  // 0=Color, 1=Procedural, 2=Panorama
        scope.push("_set_clear_color", false);
        scope.push("_clear_color_r", 0.4_f64);
        scope.push("_clear_color_g", 0.6_f64);
        scope.push("_clear_color_b", 0.9_f64);
        scope.push("_set_ambient_brightness", false);
        scope.push("_ambient_brightness", 300.0_f64);
        scope.push("_set_ambient_color", false);
        scope.push("_ambient_color_r", 1.0_f64);
        scope.push("_ambient_color_g", 1.0_f64);
        scope.push("_ambient_color_b", 1.0_f64);
        scope.push("_set_ev100", false);
        scope.push("_ev100", 9.7_f64);

        // Environment - Procedural Sky
        scope.push("_set_sky_top_color", false);
        scope.push("_sky_top_r", 0.15_f64);
        scope.push("_sky_top_g", 0.35_f64);
        scope.push("_sky_top_b", 0.65_f64);
        scope.push("_set_sky_horizon_color", false);
        scope.push("_sky_horizon_r", 0.55_f64);
        scope.push("_sky_horizon_g", 0.70_f64);
        scope.push("_sky_horizon_b", 0.85_f64);
        scope.push("_set_sky_curve", false);
        scope.push("_sky_curve", 0.15_f64);
        scope.push("_set_ground_bottom_color", false);
        scope.push("_ground_bottom_r", 0.2_f64);
        scope.push("_ground_bottom_g", 0.17_f64);
        scope.push("_ground_bottom_b", 0.13_f64);
        scope.push("_set_ground_horizon_color", false);
        scope.push("_ground_horizon_r", 0.55_f64);
        scope.push("_ground_horizon_g", 0.55_f64);
        scope.push("_ground_horizon_b", 0.52_f64);
        scope.push("_set_ground_curve", false);
        scope.push("_ground_curve", 0.02_f64);

        // Environment - Sun
        scope.push("_set_sun_angles", false);
        scope.push("_sun_azimuth", 0.0_f64);
        scope.push("_sun_elevation", 45.0_f64);
        scope.push("_set_sun_color", false);
        scope.push("_sun_color_r", 1.0_f64);
        scope.push("_sun_color_g", 0.95_f64);
        scope.push("_sun_color_b", 0.85_f64);
        scope.push("_set_sun_energy", false);
        scope.push("_sun_energy", 1.0_f64);
        scope.push("_set_sun_disk_scale", false);
        scope.push("_sun_disk_scale", 1.0_f64);

        // Environment - Fog
        scope.push("_set_fog", false);
        scope.push("_fog_enabled", false);
        scope.push("_fog_start", 10.0_f64);
        scope.push("_fog_end", 100.0_f64);
        scope.push("_set_fog_color", false);
        scope.push("_fog_color_r", 0.5_f64);
        scope.push("_fog_color_g", 0.5_f64);
        scope.push("_fog_color_b", 0.5_f64);

        // Commands array
        scope.push("_commands", rhai::Array::new());
    }

    fn read_scope_changes(&self, scope: &Scope, ctx: &mut RhaiScriptContext) {
        // Position
        if scope.get_value::<bool>("_set_position").unwrap_or(false) {
            let x = scope.get_value::<f64>("_new_position_x").unwrap_or(0.0) as f32;
            let y = scope.get_value::<f64>("_new_position_y").unwrap_or(0.0) as f32;
            let z = scope.get_value::<f64>("_new_position_z").unwrap_or(0.0) as f32;
            ctx.new_position = Some(Vec3::new(x, y, z));
        }

        // Rotation
        if scope.get_value::<bool>("_set_rotation").unwrap_or(false) {
            let x = scope.get_value::<f64>("_new_rotation_x").unwrap_or(0.0) as f32;
            let y = scope.get_value::<f64>("_new_rotation_y").unwrap_or(0.0) as f32;
            let z = scope.get_value::<f64>("_new_rotation_z").unwrap_or(0.0) as f32;
            ctx.new_rotation = Some(Vec3::new(x, y, z));
        }

        // Translation
        if scope.get_value::<bool>("_translate").unwrap_or(false) {
            let x = scope.get_value::<f64>("_translate_x").unwrap_or(0.0) as f32;
            let y = scope.get_value::<f64>("_translate_y").unwrap_or(0.0) as f32;
            let z = scope.get_value::<f64>("_translate_z").unwrap_or(0.0) as f32;
            ctx.translation = Some(Vec3::new(x, y, z));
        }

        // Rotation delta
        if scope.get_value::<bool>("_rotate").unwrap_or(false) {
            let x = scope.get_value::<f64>("_rotate_x").unwrap_or(0.0) as f32;
            let y = scope.get_value::<f64>("_rotate_y").unwrap_or(0.0) as f32;
            let z = scope.get_value::<f64>("_rotate_z").unwrap_or(0.0) as f32;
            ctx.rotation_delta = Some(Vec3::new(x, y, z));
        }

        // Print
        if let Some(msg) = scope.get_value::<ImmutableString>("_print_message") {
            if !msg.is_empty() {
                ctx.print_message = Some(msg.to_string());
            }
        }

        // Parent changes
        if scope.get_value::<bool>("_parent_set_position").unwrap_or(false) {
            let x = scope.get_value::<f64>("_parent_new_position_x").unwrap_or(0.0) as f32;
            let y = scope.get_value::<f64>("_parent_new_position_y").unwrap_or(0.0) as f32;
            let z = scope.get_value::<f64>("_parent_new_position_z").unwrap_or(0.0) as f32;
            ctx.parent_new_position = Some(Vec3::new(x, y, z));
        }
        if scope.get_value::<bool>("_parent_set_rotation").unwrap_or(false) {
            let x = scope.get_value::<f64>("_parent_new_rotation_x").unwrap_or(0.0) as f32;
            let y = scope.get_value::<f64>("_parent_new_rotation_y").unwrap_or(0.0) as f32;
            let z = scope.get_value::<f64>("_parent_new_rotation_z").unwrap_or(0.0) as f32;
            ctx.parent_new_rotation = Some(Vec3::new(x, y, z));
        }
        if scope.get_value::<bool>("_parent_translate").unwrap_or(false) {
            let x = scope.get_value::<f64>("_parent_translate_x").unwrap_or(0.0) as f32;
            let y = scope.get_value::<f64>("_parent_translate_y").unwrap_or(0.0) as f32;
            let z = scope.get_value::<f64>("_parent_translate_z").unwrap_or(0.0) as f32;
            ctx.parent_translation = Some(Vec3::new(x, y, z));
        }

        // Child commands
        if let Some(commands) = scope.get_value::<rhai::Array>("_child_commands") {
            for cmd_dyn in commands {
                if let Some(cmd_map) = cmd_dyn.try_cast::<Map>() {
                    let cmd_type = cmd_map.get("_child_cmd").and_then(|v| v.clone().try_cast::<ImmutableString>());
                    let name = cmd_map.get("_child_name").and_then(|v| v.clone().try_cast::<ImmutableString>());
                    let x = cmd_map.get("_child_x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("_child_y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("_child_z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;

                    if let (Some(cmd), Some(child_name)) = (cmd_type, name) {
                        let change = ctx.child_changes.entry(child_name.to_string()).or_insert(ChildChange::default());
                        match cmd.as_str() {
                            "set_position" => change.new_position = Some(Vec3::new(x, y, z)),
                            "set_rotation" => change.new_rotation = Some(Vec3::new(x, y, z)),
                            "translate" => change.translation = Some(Vec3::new(x, y, z)),
                            _ => {}
                        }
                    }
                }
            }
        }

        // Environment - General
        if scope.get_value::<bool>("_set_sky_mode").unwrap_or(false) {
            ctx.env_sky_mode = Some(scope.get_value::<i64>("_sky_mode").unwrap_or(1) as u8);
        }
        if scope.get_value::<bool>("_set_clear_color").unwrap_or(false) {
            let r = scope.get_value::<f64>("_clear_color_r").unwrap_or(0.4) as f32;
            let g = scope.get_value::<f64>("_clear_color_g").unwrap_or(0.6) as f32;
            let b = scope.get_value::<f64>("_clear_color_b").unwrap_or(0.9) as f32;
            ctx.env_clear_color = Some((r, g, b));
        }
        if scope.get_value::<bool>("_set_ambient_brightness").unwrap_or(false) {
            ctx.env_ambient_brightness = Some(scope.get_value::<f64>("_ambient_brightness").unwrap_or(300.0) as f32);
        }
        if scope.get_value::<bool>("_set_ambient_color").unwrap_or(false) {
            let r = scope.get_value::<f64>("_ambient_color_r").unwrap_or(1.0) as f32;
            let g = scope.get_value::<f64>("_ambient_color_g").unwrap_or(1.0) as f32;
            let b = scope.get_value::<f64>("_ambient_color_b").unwrap_or(1.0) as f32;
            ctx.env_ambient_color = Some((r, g, b));
        }
        if scope.get_value::<bool>("_set_ev100").unwrap_or(false) {
            ctx.env_ev100 = Some(scope.get_value::<f64>("_ev100").unwrap_or(9.7) as f32);
        }

        // Environment - Procedural Sky
        if scope.get_value::<bool>("_set_sky_top_color").unwrap_or(false) {
            let r = scope.get_value::<f64>("_sky_top_r").unwrap_or(0.15) as f32;
            let g = scope.get_value::<f64>("_sky_top_g").unwrap_or(0.35) as f32;
            let b = scope.get_value::<f64>("_sky_top_b").unwrap_or(0.65) as f32;
            ctx.env_sky_top_color = Some((r, g, b));
        }
        if scope.get_value::<bool>("_set_sky_horizon_color").unwrap_or(false) {
            let r = scope.get_value::<f64>("_sky_horizon_r").unwrap_or(0.55) as f32;
            let g = scope.get_value::<f64>("_sky_horizon_g").unwrap_or(0.70) as f32;
            let b = scope.get_value::<f64>("_sky_horizon_b").unwrap_or(0.85) as f32;
            ctx.env_sky_horizon_color = Some((r, g, b));
        }
        if scope.get_value::<bool>("_set_sky_curve").unwrap_or(false) {
            ctx.env_sky_curve = Some(scope.get_value::<f64>("_sky_curve").unwrap_or(0.15) as f32);
        }
        if scope.get_value::<bool>("_set_ground_bottom_color").unwrap_or(false) {
            let r = scope.get_value::<f64>("_ground_bottom_r").unwrap_or(0.2) as f32;
            let g = scope.get_value::<f64>("_ground_bottom_g").unwrap_or(0.17) as f32;
            let b = scope.get_value::<f64>("_ground_bottom_b").unwrap_or(0.13) as f32;
            ctx.env_ground_bottom_color = Some((r, g, b));
        }
        if scope.get_value::<bool>("_set_ground_horizon_color").unwrap_or(false) {
            let r = scope.get_value::<f64>("_ground_horizon_r").unwrap_or(0.55) as f32;
            let g = scope.get_value::<f64>("_ground_horizon_g").unwrap_or(0.55) as f32;
            let b = scope.get_value::<f64>("_ground_horizon_b").unwrap_or(0.52) as f32;
            ctx.env_ground_horizon_color = Some((r, g, b));
        }
        if scope.get_value::<bool>("_set_ground_curve").unwrap_or(false) {
            ctx.env_ground_curve = Some(scope.get_value::<f64>("_ground_curve").unwrap_or(0.02) as f32);
        }

        // Environment - Sun
        if scope.get_value::<bool>("_set_sun_angles").unwrap_or(false) {
            ctx.env_sun_azimuth = Some(scope.get_value::<f64>("_sun_azimuth").unwrap_or(0.0) as f32);
            ctx.env_sun_elevation = Some(scope.get_value::<f64>("_sun_elevation").unwrap_or(45.0) as f32);
        }
        if scope.get_value::<bool>("_set_sun_color").unwrap_or(false) {
            let r = scope.get_value::<f64>("_sun_color_r").unwrap_or(1.0) as f32;
            let g = scope.get_value::<f64>("_sun_color_g").unwrap_or(0.95) as f32;
            let b = scope.get_value::<f64>("_sun_color_b").unwrap_or(0.85) as f32;
            ctx.env_sun_color = Some((r, g, b));
        }
        if scope.get_value::<bool>("_set_sun_energy").unwrap_or(false) {
            ctx.env_sun_energy = Some(scope.get_value::<f64>("_sun_energy").unwrap_or(1.0) as f32);
        }
        if scope.get_value::<bool>("_set_sun_disk_scale").unwrap_or(false) {
            ctx.env_sun_disk_scale = Some(scope.get_value::<f64>("_sun_disk_scale").unwrap_or(1.0) as f32);
        }

        // Environment - Fog
        if scope.get_value::<bool>("_set_fog").unwrap_or(false) {
            ctx.env_fog_enabled = Some(scope.get_value::<bool>("_fog_enabled").unwrap_or(false));
            ctx.env_fog_start = Some(scope.get_value::<f64>("_fog_start").unwrap_or(10.0) as f32);
            ctx.env_fog_end = Some(scope.get_value::<f64>("_fog_end").unwrap_or(100.0) as f32);
        }
        if scope.get_value::<bool>("_set_fog_color").unwrap_or(false) {
            let r = scope.get_value::<f64>("_fog_color_r").unwrap_or(0.5) as f32;
            let g = scope.get_value::<f64>("_fog_color_g").unwrap_or(0.5) as f32;
            let b = scope.get_value::<f64>("_fog_color_b").unwrap_or(0.5) as f32;
            ctx.env_fog_color = Some((r, g, b));
        }

        // Read commands array (ECS, audio, debug, physics, etc.)
        self.read_commands_array(scope, ctx);
    }

    fn read_commands_array(&self, scope: &Scope, ctx: &mut RhaiScriptContext) {
        let Some(commands) = scope.get_value::<rhai::Array>("_commands") else { return };

        for cmd_dyn in commands {
            let Some(cmd_map) = cmd_dyn.try_cast::<Map>() else { continue };
            let Some(cmd_type) = cmd_map.get("_cmd").and_then(|v| v.clone().try_cast::<ImmutableString>()) else { continue };

            match cmd_type.as_str() {
                "spawn_entity" => {
                    let name = cmd_map.get("name").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_else(|| "Entity".to_string());
                    ctx.commands.push(RhaiCommand::SpawnEntity { name });
                }
                "spawn_primitive" => {
                    let name = cmd_map.get("name").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_else(|| "Primitive".to_string());
                    let primitive_type = cmd_map.get("primitive_type").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_else(|| "cube".to_string());
                    // Optional position
                    let position = if cmd_map.contains_key("x") {
                        let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                        let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                        let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                        Some(Vec3::new(x, y, z))
                    } else {
                        None
                    };
                    // Optional scale
                    let scale = if cmd_map.contains_key("sx") {
                        let sx = cmd_map.get("sx").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                        let sy = cmd_map.get("sy").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                        let sz = cmd_map.get("sz").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                        Some(Vec3::new(sx, sy, sz))
                    } else {
                        None
                    };
                    ctx.commands.push(RhaiCommand::SpawnPrimitive { name, primitive_type, position, scale });
                }
                "despawn_entity" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    ctx.commands.push(RhaiCommand::DespawnEntity { entity_id });
                }
                "despawn_self" => ctx.commands.push(RhaiCommand::DespawnSelf),
                "play_sound" => {
                    let path = cmd_map.get("path").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    let volume = cmd_map.get("volume").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let looping = cmd_map.get("looping").and_then(|v| v.clone().try_cast::<bool>()).unwrap_or(false);
                    ctx.commands.push(RhaiCommand::PlaySound { path, volume, looping });
                }
                "play_sound_3d" => {
                    let path = cmd_map.get("path").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    let volume = cmd_map.get("volume").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::PlaySound3D { path, volume, position: Vec3::new(x, y, z) });
                }
                "stop_all_sounds" => ctx.commands.push(RhaiCommand::StopAllSounds),
                "log" => {
                    let level = cmd_map.get("level").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_else(|| "info".to_string());
                    let message = cmd_map.get("message").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    ctx.commands.push(RhaiCommand::Log { level, message });
                }
                "draw_line" => {
                    let sx = cmd_map.get("sx").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let sy = cmd_map.get("sy").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let sz = cmd_map.get("sz").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let ex = cmd_map.get("ex").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let ey = cmd_map.get("ey").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let ez = cmd_map.get("ez").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let r = cmd_map.get("r").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let g = cmd_map.get("g").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let b = cmd_map.get("b").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let a = cmd_map.get("a").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let duration = cmd_map.get("duration").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::DrawLine { start: Vec3::new(sx, sy, sz), end: Vec3::new(ex, ey, ez), color: [r, g, b, a], duration });
                }
                "draw_sphere" => {
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let radius = cmd_map.get("radius").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let r = cmd_map.get("r").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let g = cmd_map.get("g").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let b = cmd_map.get("b").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let a = cmd_map.get("a").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let duration = cmd_map.get("duration").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::DrawSphere { center: Vec3::new(x, y, z), radius, color: [r, g, b, a], duration });
                }
                "draw_box" => {
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let hx = cmd_map.get("hx").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.5) as f32;
                    let hy = cmd_map.get("hy").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.5) as f32;
                    let hz = cmd_map.get("hz").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.5) as f32;
                    let r = cmd_map.get("r").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let g = cmd_map.get("g").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let b = cmd_map.get("b").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let a = cmd_map.get("a").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let duration = cmd_map.get("duration").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::DrawBox { center: Vec3::new(x, y, z), half_extents: Vec3::new(hx, hy, hz), color: [r, g, b, a], duration });
                }
                "apply_force" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::ApplyForce { entity_id, force: Vec3::new(x, y, z) });
                }
                "apply_impulse" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::ApplyImpulse { entity_id, impulse: Vec3::new(x, y, z) });
                }
                "set_velocity" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::SetVelocity { entity_id, velocity: Vec3::new(x, y, z) });
                }
                "start_timer" => {
                    let name = cmd_map.get("name").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    let duration = cmd_map.get("duration").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let repeat = cmd_map.get("repeat").and_then(|v| v.clone().try_cast::<bool>()).unwrap_or(false);
                    ctx.commands.push(RhaiCommand::StartTimer { name, duration, repeat });
                }
                "stop_timer" => {
                    let name = cmd_map.get("name").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    ctx.commands.push(RhaiCommand::StopTimer { name });
                }
                "pause_timer" => {
                    let name = cmd_map.get("name").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    ctx.commands.push(RhaiCommand::PauseTimer { name });
                }
                "resume_timer" => {
                    let name = cmd_map.get("name").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    ctx.commands.push(RhaiCommand::ResumeTimer { name });
                }

                // ECS - additional commands
                "set_entity_name" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    let name = cmd_map.get("name").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    ctx.commands.push(RhaiCommand::SetEntityName { entity_id, name });
                }
                "add_tag" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let tag = cmd_map.get("tag").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    ctx.commands.push(RhaiCommand::AddTag { entity_id, tag });
                }
                "remove_tag" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let tag = cmd_map.get("tag").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    ctx.commands.push(RhaiCommand::RemoveTag { entity_id, tag });
                }

                // Audio - additional commands
                "play_music" => {
                    let path = cmd_map.get("path").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    let volume = cmd_map.get("volume").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let fade_in = cmd_map.get("fade_in").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::PlayMusic { path, volume, fade_in });
                }
                "stop_music" => {
                    let fade_out = cmd_map.get("fade_out").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::StopMusic { fade_out });
                }
                "set_master_volume" => {
                    let volume = cmd_map.get("volume").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    ctx.commands.push(RhaiCommand::SetMasterVolume { volume });
                }

                // Debug - additional commands
                "draw_ray" => {
                    let ox = cmd_map.get("ox").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let oy = cmd_map.get("oy").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let oz = cmd_map.get("oz").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let dx = cmd_map.get("dx").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let dy = cmd_map.get("dy").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let dz = cmd_map.get("dz").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let length = cmd_map.get("length").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(10.0) as f32;
                    let r = cmd_map.get("r").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let g = cmd_map.get("g").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let b = cmd_map.get("b").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let a = cmd_map.get("a").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let duration = cmd_map.get("duration").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::DrawRay { origin: Vec3::new(ox, oy, oz), direction: Vec3::new(dx, dy, dz), length, color: [r, g, b, a], duration });
                }
                "draw_point" => {
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let size = cmd_map.get("size").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(5.0) as f32;
                    let r = cmd_map.get("r").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let g = cmd_map.get("g").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let b = cmd_map.get("b").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let a = cmd_map.get("a").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let duration = cmd_map.get("duration").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::DrawPoint { position: Vec3::new(x, y, z), size, color: [r, g, b, a], duration });
                }

                // Physics - additional commands
                "apply_torque" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::ApplyTorque { entity_id, torque: Vec3::new(x, y, z) });
                }
                "set_angular_velocity" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::SetAngularVelocity { entity_id, velocity: Vec3::new(x, y, z) });
                }
                "set_gravity_scale" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let scale = cmd_map.get("scale").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    ctx.commands.push(RhaiCommand::SetGravityScale { entity_id, scale });
                }
                "raycast" => {
                    let ox = cmd_map.get("ox").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let oy = cmd_map.get("oy").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let oz = cmd_map.get("oz").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let dx = cmd_map.get("dx").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let dy = cmd_map.get("dy").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(-1.0) as f32;
                    let dz = cmd_map.get("dz").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let max_distance = cmd_map.get("max_distance").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(100.0) as f32;
                    let result_var = cmd_map.get("result_var").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_else(|| "raycast_result".to_string());
                    ctx.commands.push(RhaiCommand::Raycast { origin: Vec3::new(ox, oy, oz), direction: Vec3::new(dx, dy, dz), max_distance, result_var });
                }

                // Scene commands
                "load_scene" => {
                    let path = cmd_map.get("path").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    ctx.commands.push(RhaiCommand::LoadScene { path });
                }
                "unload_scene" => {
                    let handle_id = cmd_map.get("handle_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    ctx.commands.push(RhaiCommand::UnloadScene { handle_id });
                }
                "spawn_prefab" => {
                    let path = cmd_map.get("path").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let rx = cmd_map.get("rx").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let ry = cmd_map.get("ry").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let rz = cmd_map.get("rz").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::SpawnPrefab { path, position: Vec3::new(x, y, z), rotation: Vec3::new(rx, ry, rz) });
                }
                "spawn_prefab_here" => {
                    // Spawn at the calling entity's current position
                    let path = cmd_map.get("path").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    let position = ctx.transform.position;
                    ctx.commands.push(RhaiCommand::SpawnPrefab { path, position, rotation: Vec3::ZERO });
                }

                // Animation commands
                "play_animation" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let name = cmd_map.get("name").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    let looping = cmd_map.get("looping").and_then(|v| v.clone().try_cast::<bool>()).unwrap_or(true);
                    let speed = cmd_map.get("speed").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    ctx.commands.push(RhaiCommand::PlayAnimation { entity_id, name, looping, speed });
                }
                "stop_animation" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    ctx.commands.push(RhaiCommand::StopAnimation { entity_id });
                }
                "set_animation_speed" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let speed = cmd_map.get("speed").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    ctx.commands.push(RhaiCommand::SetAnimationSpeed { entity_id, speed });
                }

                // Rendering commands
                "set_visibility" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let visible = cmd_map.get("visible").and_then(|v| v.clone().try_cast::<bool>()).unwrap_or(true);
                    ctx.commands.push(RhaiCommand::SetVisibility { entity_id, visible });
                }
                "set_material_color" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let r = cmd_map.get("r").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let g = cmd_map.get("g").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let b = cmd_map.get("b").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let a = cmd_map.get("a").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    ctx.commands.push(RhaiCommand::SetMaterialColor { entity_id, color: [r, g, b, a] });
                }
                "set_light_intensity" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let intensity = cmd_map.get("intensity").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    ctx.commands.push(RhaiCommand::SetLightIntensity { entity_id, intensity });
                }
                "set_light_color" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let r = cmd_map.get("r").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let g = cmd_map.get("g").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let b = cmd_map.get("b").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    ctx.commands.push(RhaiCommand::SetLightColor { entity_id, color: [r, g, b] });
                }

                // Camera commands
                "set_camera_target" | "camera_look_at" => {
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::SetCameraTarget { position: Vec3::new(x, y, z) });
                }
                "set_camera_zoom" => {
                    let zoom = cmd_map.get("zoom").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    ctx.commands.push(RhaiCommand::SetCameraZoom { zoom });
                }
                "screen_shake" => {
                    let intensity = cmd_map.get("intensity").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.5) as f32;
                    let duration = cmd_map.get("duration").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.5) as f32;
                    ctx.commands.push(RhaiCommand::ScreenShake { intensity, duration });
                }
                "camera_follow" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    let offset_x = cmd_map.get("offset_x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let offset_y = cmd_map.get("offset_y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(5.0) as f32;
                    let offset_z = cmd_map.get("offset_z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(10.0) as f32;
                    let smoothing = cmd_map.get("smoothing").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.1) as f32;
                    ctx.commands.push(RhaiCommand::CameraFollow {
                        entity_id,
                        offset: Vec3::new(offset_x, offset_y, offset_z),
                        smoothing,
                    });
                }
                "camera_follow_self" => {
                    // Use self entity for following
                    let offset_x = cmd_map.get("offset_x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let offset_y = cmd_map.get("offset_y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(5.0) as f32;
                    let offset_z = cmd_map.get("offset_z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(10.0) as f32;
                    let smoothing = cmd_map.get("smoothing").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.1) as f32;
                    ctx.commands.push(RhaiCommand::CameraFollow {
                        entity_id: ctx.self_entity_id,
                        offset: Vec3::new(offset_x, offset_y, offset_z),
                        smoothing,
                    });
                }
                "camera_stop_follow" => {
                    ctx.commands.push(RhaiCommand::StopCameraFollow);
                }

                // Component commands - Health
                "set_health" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let value = cmd_map.get("value").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(100.0) as f32;
                    ctx.commands.push(RhaiCommand::SetHealth { entity_id, value });
                }
                "set_max_health" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let value = cmd_map.get("value").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(100.0) as f32;
                    ctx.commands.push(RhaiCommand::SetMaxHealth { entity_id, value });
                }
                "damage" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let amount = cmd_map.get("amount").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(10.0) as f32;
                    ctx.commands.push(RhaiCommand::Damage { entity_id, amount });
                }
                "heal" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let amount = cmd_map.get("amount").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(10.0) as f32;
                    ctx.commands.push(RhaiCommand::Heal { entity_id, amount });
                }
                "set_invincible" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let invincible = cmd_map.get("invincible").and_then(|v| v.clone().try_cast::<bool>()).unwrap_or(true);
                    let duration = cmd_map.get("duration").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::SetInvincible { entity_id, invincible, duration });
                }
                "kill" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    ctx.commands.push(RhaiCommand::Kill { entity_id });
                }
                "revive" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    ctx.commands.push(RhaiCommand::Revive { entity_id });
                }

                // Component commands - Material emissive
                "set_material_emissive" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let r = cmd_map.get("r").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let g = cmd_map.get("g").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let b = cmd_map.get("b").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    ctx.commands.push(RhaiCommand::SetComponentField {
                        entity_id,
                        component_type: "material".to_string(),
                        field_name: "emissive".to_string(),
                        value: ComponentValue::Vec3([r, g, b]),
                    });
                }

                // Generic component field setting
                "set_component_field" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let component_type = cmd_map.get("component_type")
                        .and_then(|v| v.clone().into_immutable_string().ok())
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let field_name = cmd_map.get("field")
                        .and_then(|v| v.clone().into_immutable_string().ok())
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let value_type = cmd_map.get("value_type")
                        .and_then(|v| v.clone().into_immutable_string().ok())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "float".to_string());

                    let value = match value_type.as_str() {
                        "float" => ComponentValue::Float(
                            cmd_map.get("value").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32
                        ),
                        "int" => ComponentValue::Int(
                            cmd_map.get("value").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0)
                        ),
                        "bool" => ComponentValue::Bool(
                            cmd_map.get("value").and_then(|v| v.clone().try_cast::<bool>()).unwrap_or(false)
                        ),
                        "string" => ComponentValue::String(
                            cmd_map.get("value")
                                .and_then(|v| v.clone().into_immutable_string().ok())
                                .map(|s| s.to_string())
                                .unwrap_or_default()
                        ),
                        _ => ComponentValue::Float(0.0),
                    };

                    ctx.commands.push(RhaiCommand::SetComponentField {
                        entity_id,
                        component_type,
                        field_name,
                        value,
                    });
                }

                // Additional animation commands
                "pause_animation" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    ctx.commands.push(RhaiCommand::PauseAnimation { entity_id });
                }
                "resume_animation" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    ctx.commands.push(RhaiCommand::ResumeAnimation { entity_id });
                }

                // Sprite animation commands
                "play_sprite_animation" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let name = cmd_map.get("name").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    let looping = cmd_map.get("looping").and_then(|v| v.clone().try_cast::<bool>()).unwrap_or(true);
                    ctx.commands.push(RhaiCommand::PlaySpriteAnimation { entity_id, name, looping });
                }
                "set_sprite_frame" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let frame = cmd_map.get("frame").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0);
                    ctx.commands.push(RhaiCommand::SetSpriteFrame { entity_id, frame });
                }

                // Tween commands
                "tween" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let property = cmd_map.get("property").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    let target = cmd_map.get("target").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let duration = cmd_map.get("duration").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let easing = cmd_map.get("easing").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_else(|| "linear".to_string());
                    ctx.commands.push(RhaiCommand::Tween { entity_id, property, target, duration, easing });
                }
                "tween_position" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let duration = cmd_map.get("duration").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let easing = cmd_map.get("easing").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_else(|| "linear".to_string());
                    ctx.commands.push(RhaiCommand::TweenPosition { entity_id, target: Vec3::new(x, y, z), duration, easing });
                }
                "tween_rotation" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let duration = cmd_map.get("duration").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let easing = cmd_map.get("easing").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_else(|| "linear".to_string());
                    ctx.commands.push(RhaiCommand::TweenRotation { entity_id, target: Vec3::new(x, y, z), duration, easing });
                }
                "tween_scale" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).map(|id| id as u64);
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let duration = cmd_map.get("duration").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let easing = cmd_map.get("easing").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_else(|| "linear".to_string());
                    ctx.commands.push(RhaiCommand::TweenScale { entity_id, target: Vec3::new(x, y, z), duration, easing });
                }

                // Particle commands
                "particle_play" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    ctx.commands.push(RhaiCommand::ParticlePlay { entity_id });
                }
                "particle_pause" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    ctx.commands.push(RhaiCommand::ParticlePause { entity_id });
                }
                "particle_stop" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    ctx.commands.push(RhaiCommand::ParticleStop { entity_id });
                }
                "particle_reset" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    ctx.commands.push(RhaiCommand::ParticleReset { entity_id });
                }
                "particle_burst" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    let count = cmd_map.get("count").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(10) as u32;
                    ctx.commands.push(RhaiCommand::ParticleBurst { entity_id, count });
                }
                "particle_set_rate" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    let multiplier = cmd_map.get("multiplier").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    ctx.commands.push(RhaiCommand::ParticleSetRate { entity_id, multiplier });
                }
                "particle_set_scale" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    let multiplier = cmd_map.get("multiplier").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    ctx.commands.push(RhaiCommand::ParticleSetScale { entity_id, multiplier });
                }
                "particle_set_time_scale" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    let scale = cmd_map.get("scale").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    ctx.commands.push(RhaiCommand::ParticleSetTimeScale { entity_id, scale });
                }
                "particle_set_tint" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    let r = cmd_map.get("r").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let g = cmd_map.get("g").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let b = cmd_map.get("b").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    let a = cmd_map.get("a").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                    ctx.commands.push(RhaiCommand::ParticleSetTint { entity_id, r, g, b, a });
                }
                "particle_set_variable" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    let name = cmd_map.get("name").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_default();
                    let var_type = cmd_map.get("var_type").and_then(|v| v.clone().try_cast::<ImmutableString>()).map(|s| s.to_string()).unwrap_or_else(|| "float".to_string());
                    match var_type.as_str() {
                        "float" => {
                            let value = cmd_map.get("value").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                            ctx.commands.push(RhaiCommand::ParticleSetVariableFloat { entity_id, name, value });
                        }
                        "color" => {
                            let r = cmd_map.get("r").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                            let g = cmd_map.get("g").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                            let b = cmd_map.get("b").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                            let a = cmd_map.get("a").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(1.0) as f32;
                            ctx.commands.push(RhaiCommand::ParticleSetVariableColor { entity_id, name, r, g, b, a });
                        }
                        "vec3" => {
                            let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                            let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                            let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                            ctx.commands.push(RhaiCommand::ParticleSetVariableVec3 { entity_id, name, x, y, z });
                        }
                        _ => {}
                    }
                }
                "particle_emit_at" => {
                    let entity_id = cmd_map.get("entity_id").and_then(|v| v.clone().try_cast::<i64>()).unwrap_or(0) as u64;
                    let x = cmd_map.get("x").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let y = cmd_map.get("y").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let z = cmd_map.get("z").and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0) as f32;
                    let count = cmd_map.get("count").and_then(|v| v.clone().try_cast::<i64>()).map(|c| c as u32);
                    ctx.commands.push(RhaiCommand::ParticleEmitAt { entity_id, x, y, z, count });
                }

                _ => {}
            }
        }
    }
}
