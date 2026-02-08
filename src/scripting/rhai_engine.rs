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
use super::rhai_commands::RhaiCommand;
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
        vars: &mut ScriptVariables,
    ) {
        let mut scope = Scope::new();
        self.setup_scope(&mut scope, ctx, vars);

        // Drain any stale commands before executing
        rhai_api::drain_commands();

        match self.engine.call_fn::<Dynamic>(&mut scope, &script.ast, "on_ready", ()) {
            Ok(_) => {}
            Err(e) => {
                // Check if it's just a missing function (which is OK)
                let err_str = e.to_string();
                if !err_str.contains("Function not found") {
                    console_log(LogLevel::Error, "Script", format!("{} on_ready: {}", script.name, e));
                }
            }
        }

        self.read_back_variables(&scope, vars);
        self.process_command_buffer(ctx);
    }

    /// Execute on_update function
    pub fn call_on_update(
        &self,
        script: &CompiledScript,
        ctx: &mut RhaiScriptContext,
        vars: &mut ScriptVariables,
    ) {
        let mut scope = Scope::new();
        self.setup_scope(&mut scope, ctx, vars);

        // Drain any stale commands before executing
        rhai_api::drain_commands();

        match self.engine.call_fn::<Dynamic>(&mut scope, &script.ast, "on_update", ()) {
            Ok(_) => {}
            Err(e) => {
                let err_str = e.to_string();
                if !err_str.contains("Function not found") {
                    console_log(LogLevel::Error, "Script", format!("{} on_update: {}", script.name, e));
                }
            }
        }

        self.read_back_variables(&scope, vars);
        self.process_command_buffer(ctx);
    }

    /// Read back modified script variables from scope into ScriptVariables
    fn read_back_variables(&self, scope: &Scope, vars: &mut ScriptVariables) {
        // Collect names first to avoid borrow conflict
        let var_names: Vec<String> = vars.iter_all().map(|(k, _)| k.clone()).collect();
        for name in &var_names {
            if let Some(value) = scope.get_value::<Dynamic>(name) {
                if let Some(sv) = dynamic_to_script_value(&value) {
                    vars.set(name.clone(), sv);
                }
            }
        }
    }

    /// Process commands from the thread-local buffer (auto-pushed by API functions)
    fn process_command_buffer(&self, ctx: &mut RhaiScriptContext) {
        let commands = rhai_api::drain_commands();
        for cmd in commands {
            match cmd {
                // Self-transform commands — write directly to context fields
                RhaiCommand::SetPosition { x, y, z } => ctx.new_position = Some(Vec3::new(x, y, z)),
                RhaiCommand::SetRotation { x, y, z } => ctx.new_rotation = Some(Vec3::new(x, y, z)),
                RhaiCommand::SetScale { x, y, z } => ctx.new_scale = Some(Vec3::new(x, y, z)),
                RhaiCommand::Translate { x, y, z } => ctx.translation = Some(Vec3::new(x, y, z)),
                RhaiCommand::Rotate { x, y, z } => ctx.rotation_delta = Some(Vec3::new(x, y, z)),
                RhaiCommand::LookAt { x, y, z } => ctx.look_at_target = Some(Vec3::new(x, y, z)),

                // Parent transform commands
                RhaiCommand::ParentSetPosition { x, y, z } => ctx.parent_new_position = Some(Vec3::new(x, y, z)),
                RhaiCommand::ParentSetRotation { x, y, z } => ctx.parent_new_rotation = Some(Vec3::new(x, y, z)),
                RhaiCommand::ParentTranslate { x, y, z } => ctx.parent_translation = Some(Vec3::new(x, y, z)),

                // Child transform commands
                RhaiCommand::ChildSetPosition { name, x, y, z } => {
                    let change = ctx.child_changes.entry(name).or_insert(ChildChange::default());
                    change.new_position = Some(Vec3::new(x, y, z));
                }
                RhaiCommand::ChildSetRotation { name, x, y, z } => {
                    let change = ctx.child_changes.entry(name).or_insert(ChildChange::default());
                    change.new_rotation = Some(Vec3::new(x, y, z));
                }
                RhaiCommand::ChildTranslate { name, x, y, z } => {
                    let change = ctx.child_changes.entry(name).or_insert(ChildChange::default());
                    change.translation = Some(Vec3::new(x, y, z));
                }

                // Environment commands — write directly to context fields
                RhaiCommand::SetSunAngles { azimuth, elevation } => {
                    ctx.env_sun_azimuth = Some(azimuth);
                    ctx.env_sun_elevation = Some(elevation);
                }
                RhaiCommand::SetAmbientBrightness { brightness } => ctx.env_ambient_brightness = Some(brightness),
                RhaiCommand::SetAmbientColor { r, g, b } => ctx.env_ambient_color = Some((r, g, b)),
                RhaiCommand::SetSkyTopColor { r, g, b } => ctx.env_sky_top_color = Some((r, g, b)),
                RhaiCommand::SetSkyHorizonColor { r, g, b } => ctx.env_sky_horizon_color = Some((r, g, b)),
                RhaiCommand::SetFog { enabled, start, end } => {
                    ctx.env_fog_enabled = Some(enabled);
                    ctx.env_fog_start = Some(start);
                    ctx.env_fog_end = Some(end);
                }
                RhaiCommand::SetFogColor { r, g, b } => ctx.env_fog_color = Some((r, g, b)),
                RhaiCommand::SetEv100 { value } => ctx.env_ev100 = Some(value),

                // All other commands — push directly to ctx.commands
                other => ctx.commands.push(other),
            }
        }
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

        // Variables - push as direct scope variables AND in vars map for backward compat
        let mut var_map = Map::new();
        for (key, value) in vars.iter_all() {
            let dyn_val = match value {
                ScriptValue::Float(v) => Dynamic::from(*v as f64),
                ScriptValue::Int(v) => Dynamic::from(*v as i64),
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
            // Push as direct scope variable
            scope.push(key.as_str(), dyn_val.clone());
            var_map.insert(key.clone().into(), dyn_val);
        }
        scope.push("vars", var_map);


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
    }
}

