//! Rhai scripting engine integration

use bevy::prelude::*;
use rhai::{Dynamic, Engine, AST, Scope, Map, ImmutableString};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use super::{ScriptTime, ScriptTransform, ScriptValue, ScriptVariables, ScriptVariableDefinition};

/// Child node info for scripts
#[derive(Clone)]
pub struct ChildNodeInfo {
    pub entity: Entity,
    pub name: String,
    pub position: Vec3,
    pub rotation: Vec3, // euler degrees
    pub scale: Vec3,
}

/// Pending child transform change
#[derive(Clone)]
pub struct ChildChange {
    pub new_position: Option<Vec3>,
    pub new_rotation: Option<Vec3>,
    pub translation: Option<Vec3>,
}

/// Cached compiled Rhai script
#[derive(Clone)]
pub struct CompiledScript {
    pub ast: AST,
    pub path: PathBuf,
    pub name: String,
    pub last_modified: std::time::SystemTime,
    /// Script-defined props (from @prop comments)
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
    // Check for map (could be vec2, vec3, color, or prop definition with hint)
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

    // Try to call the props() function
    let result: Result<Dynamic, _> = engine.call_fn(&mut scope, ast, "props", ());

    let Ok(result) = result else {
        return props;
    };

    // Result should be a map
    let Some(map) = result.try_cast::<Map>() else {
        return props;
    };

    // Parse each entry in the map
    for (key, value) in map.iter() {
        let name = key.to_string();
        let display_name = to_display_name(&name);

        // Check if value is a map with 'default' and optional 'hint'
        if let Some(prop_map) = value.clone().try_cast::<Map>() {
            if prop_map.contains_key("default") {
                // Extended format: #{ default: value, hint: "description" }
                let default_val = prop_map.get("default").unwrap();
                let hint = prop_map.get("hint")
                    .and_then(|v| v.clone().try_cast::<ImmutableString>())
                    .map(|s| s.to_string());

                if let Some(script_value) = dynamic_to_script_value(default_val) {
                    let mut def = ScriptVariableDefinition::new(name, script_value)
                        .with_display_name(display_name);
                    if let Some(h) = hint {
                        def = def.with_hint(h);
                    }
                    props.push(def);
                }
                continue;
            }
        }

        // Simple format: just the value directly
        if let Some(script_value) = dynamic_to_script_value(value) {
            let def = ScriptVariableDefinition::new(name, script_value)
                .with_display_name(display_name);
            props.push(def);
        }
    }

    // Sort by name for consistent ordering
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

        // Register custom types and functions
        Self::register_api(&mut engine);

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

    /// Get available script files from the scripts folder
    pub fn get_available_scripts(&self) -> Vec<(String, PathBuf)> {
        let Some(folder) = &self.scripts_folder else {
            return Vec::new();
        };

        let mut scripts = Vec::new();

        if let Ok(entries) = std::fs::read_dir(folder) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "rhai") {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    scripts.push((name, path));
                }
            }
        }

        scripts.sort_by(|a, b| a.0.cmp(&b.0));
        scripts
    }

    /// Get the props defined in a script file
    pub fn get_script_props(&self, path: &Path) -> Vec<ScriptVariableDefinition> {
        // Try to load/get cached script
        if let Ok(compiled) = self.load_script(path) {
            compiled.props
        } else {
            Vec::new()
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

        // Read and compile
        let source = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read script: {}", e))?;

        let ast = self.engine.compile(&source)
            .map_err(|e| format!("Failed to compile script: {}", e))?;

        // Parse props by executing props() function
        let props = parse_script_props(&self.engine, &ast);

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let last_modified = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        let compiled = CompiledScript {
            ast,
            path: path.to_path_buf(),
            name,
            last_modified,
            props,
        };

        // Cache it
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
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                true
            }
        } else {
            false
        };

        if needs_reload {
            self.load_script(path).ok()
        } else {
            None
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

        if let Err(e) = self.engine.call_fn::<()>(&mut scope, &script.ast, "on_ready", ()) {
            bevy::log::error!("[Rhai] on_ready error: {}", e);
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

        if let Err(e) = self.engine.call_fn::<()>(&mut scope, &script.ast, "on_update", ()) {
            bevy::log::error!("[Rhai] on_update error: {}", e);
        }

        self.read_scope_changes(&scope, ctx);
    }

    fn setup_scope(&self, scope: &mut Scope, ctx: &RhaiScriptContext, vars: &ScriptVariables) {
        // Time info
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

        // Input
        scope.push("input_x", ctx.input_movement.x as f64);
        scope.push("input_y", ctx.input_movement.y as f64);
        scope.push("mouse_x", ctx.mouse_position.x as f64);
        scope.push("mouse_y", ctx.mouse_position.y as f64);
        scope.push("mouse_delta_x", ctx.mouse_delta.x as f64);
        scope.push("mouse_delta_y", ctx.mouse_delta.y as f64);

        // Gamepad input
        scope.push("gamepad_left_x", ctx.gamepad_left_stick.x as f64);
        scope.push("gamepad_left_y", ctx.gamepad_left_stick.y as f64);
        scope.push("gamepad_right_x", ctx.gamepad_right_stick.x as f64);
        scope.push("gamepad_right_y", ctx.gamepad_right_stick.y as f64);
        scope.push("gamepad_left_trigger", ctx.gamepad_left_trigger as f64);
        scope.push("gamepad_right_trigger", ctx.gamepad_right_trigger as f64);
        // Gamepad buttons: South(A), East(B), North(Y), West(X), L1, R1, Select, Start, L3, R3, DPad
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

        // Variables as a map
        let mut var_map = Map::new();
        for (key, value) in vars.iter_all() {
            let dynamic_value = match value {
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
            var_map.insert(key.clone().into(), dynamic_value);
        }
        scope.push("vars", var_map);

        // Output commands (will be read back)
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

        scope.push("_print_message", ImmutableString::new());

        // Parent/child info (read-only)
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

        // Parent transform output commands
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

        // Children nodes - Godot-style $ChildName access
        // Creates a $ map containing all children by name
        let mut dollar_map = Map::new();
        let mut children_names: Vec<Dynamic> = Vec::new();

        for child in &ctx.children {
            // Create child data map
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

            // Add to $ map for $ChildName access
            dollar_map.insert(child.name.clone().into(), Dynamic::from(child_map));
            children_names.push(Dynamic::from(child.name.clone()));
        }

        scope.push("$", dollar_map);
        scope.push("children", children_names);

        // Child command output (for collecting changes)
        scope.push("_child_commands", rhai::Array::new());

        // Environment output commands
        scope.push("_set_sun_angles", false);
        scope.push("_sun_azimuth", 0.0_f64);
        scope.push("_sun_elevation", 45.0_f64);

        scope.push("_set_ambient_brightness", false);
        scope.push("_ambient_brightness", 300.0_f64);

        scope.push("_set_ambient_color", false);
        scope.push("_ambient_color_r", 1.0_f64);
        scope.push("_ambient_color_g", 1.0_f64);
        scope.push("_ambient_color_b", 1.0_f64);

        scope.push("_set_sky_top_color", false);
        scope.push("_sky_top_r", 0.15_f64);
        scope.push("_sky_top_g", 0.35_f64);
        scope.push("_sky_top_b", 0.65_f64);

        scope.push("_set_sky_horizon_color", false);
        scope.push("_sky_horizon_r", 0.55_f64);
        scope.push("_sky_horizon_g", 0.70_f64);
        scope.push("_sky_horizon_b", 0.85_f64);

        scope.push("_set_fog", false);
        scope.push("_fog_enabled", false);
        scope.push("_fog_start", 10.0_f64);
        scope.push("_fog_end", 100.0_f64);

        scope.push("_set_fog_color", false);
        scope.push("_fog_color_r", 0.5_f64);
        scope.push("_fog_color_g", 0.5_f64);
        scope.push("_fog_color_b", 0.5_f64);

        scope.push("_set_exposure", false);
        scope.push("_exposure", 1.0_f64);
    }

    fn read_scope_changes(&self, scope: &Scope, ctx: &mut RhaiScriptContext) {
        // Check for position changes
        if scope.get_value::<bool>("_set_position").unwrap_or(false) {
            let x = scope.get_value::<f64>("_new_position_x").unwrap_or(0.0) as f32;
            let y = scope.get_value::<f64>("_new_position_y").unwrap_or(0.0) as f32;
            let z = scope.get_value::<f64>("_new_position_z").unwrap_or(0.0) as f32;
            ctx.new_position = Some(Vec3::new(x, y, z));
        }

        // Check for rotation changes
        if scope.get_value::<bool>("_set_rotation").unwrap_or(false) {
            let x = scope.get_value::<f64>("_new_rotation_x").unwrap_or(0.0) as f32;
            let y = scope.get_value::<f64>("_new_rotation_y").unwrap_or(0.0) as f32;
            let z = scope.get_value::<f64>("_new_rotation_z").unwrap_or(0.0) as f32;
            ctx.new_rotation = Some(Vec3::new(x, y, z));
        }

        // Check for translation
        let translate_flag = scope.get_value::<bool>("_translate").unwrap_or(false);
        if translate_flag {
            let x = scope.get_value::<f64>("_translate_x").unwrap_or(0.0) as f32;
            let y = scope.get_value::<f64>("_translate_y").unwrap_or(0.0) as f32;
            let z = scope.get_value::<f64>("_translate_z").unwrap_or(0.0) as f32;
            if x.abs() > 0.0001 || z.abs() > 0.0001 {
                bevy::log::info!("[Rhai] _translate=true, x={}, y={}, z={}", x, y, z);
            }
            ctx.translation = Some(Vec3::new(x, y, z));
        }

        // Check for print
        if let Some(msg) = scope.get_value::<ImmutableString>("_print_message") {
            if !msg.is_empty() {
                ctx.print_message = Some(msg.to_string());
            }
        }

        // Check for parent position changes
        if scope.get_value::<bool>("_parent_set_position").unwrap_or(false) {
            let x = scope.get_value::<f64>("_parent_new_position_x").unwrap_or(0.0) as f32;
            let y = scope.get_value::<f64>("_parent_new_position_y").unwrap_or(0.0) as f32;
            let z = scope.get_value::<f64>("_parent_new_position_z").unwrap_or(0.0) as f32;
            ctx.parent_new_position = Some(Vec3::new(x, y, z));
        }

        // Check for parent rotation changes
        if scope.get_value::<bool>("_parent_set_rotation").unwrap_or(false) {
            let x = scope.get_value::<f64>("_parent_new_rotation_x").unwrap_or(0.0) as f32;
            let y = scope.get_value::<f64>("_parent_new_rotation_y").unwrap_or(0.0) as f32;
            let z = scope.get_value::<f64>("_parent_new_rotation_z").unwrap_or(0.0) as f32;
            ctx.parent_new_rotation = Some(Vec3::new(x, y, z));
        }

        // Check for parent translation
        if scope.get_value::<bool>("_parent_translate").unwrap_or(false) {
            let x = scope.get_value::<f64>("_parent_translate_x").unwrap_or(0.0) as f32;
            let y = scope.get_value::<f64>("_parent_translate_y").unwrap_or(0.0) as f32;
            let z = scope.get_value::<f64>("_parent_translate_z").unwrap_or(0.0) as f32;
            ctx.parent_translation = Some(Vec3::new(x, y, z));
        }

        // Check for child commands (from _child_commands array)
        if let Some(commands) = scope.get_value::<rhai::Array>("_child_commands") {
            for cmd_dyn in commands {
                if let Some(cmd_map) = cmd_dyn.try_cast::<Map>() {
                    let cmd_type = cmd_map.get("_child_cmd")
                        .and_then(|v| v.clone().try_cast::<ImmutableString>());
                    let name = cmd_map.get("_child_name")
                        .and_then(|v| v.clone().try_cast::<ImmutableString>());
                    let x = cmd_map.get("_child_x")
                        .and_then(|v| v.clone().try_cast::<f64>())
                        .unwrap_or(0.0) as f32;
                    let y = cmd_map.get("_child_y")
                        .and_then(|v| v.clone().try_cast::<f64>())
                        .unwrap_or(0.0) as f32;
                    let z = cmd_map.get("_child_z")
                        .and_then(|v| v.clone().try_cast::<f64>())
                        .unwrap_or(0.0) as f32;

                    if let (Some(cmd), Some(child_name)) = (cmd_type, name) {
                        let name_str = child_name.to_string();
                        let change = ctx.child_changes.entry(name_str).or_insert(ChildChange {
                            new_position: None,
                            new_rotation: None,
                            translation: None,
                        });

                        match cmd.as_str() {
                            "set_position" => {
                                change.new_position = Some(Vec3::new(x, y, z));
                            }
                            "set_rotation" => {
                                change.new_rotation = Some(Vec3::new(x, y, z));
                            }
                            "translate" => {
                                change.translation = Some(Vec3::new(x, y, z));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Check for environment changes
        if scope.get_value::<bool>("_set_sun_angles").unwrap_or(false) {
            let azimuth = scope.get_value::<f64>("_sun_azimuth").unwrap_or(0.0) as f32;
            let elevation = scope.get_value::<f64>("_sun_elevation").unwrap_or(45.0) as f32;
            ctx.env_sun_azimuth = Some(azimuth);
            ctx.env_sun_elevation = Some(elevation);
        }

        if scope.get_value::<bool>("_set_ambient_brightness").unwrap_or(false) {
            let val = scope.get_value::<f64>("_ambient_brightness").unwrap_or(300.0) as f32;
            ctx.env_ambient_brightness = Some(val);
        }

        if scope.get_value::<bool>("_set_ambient_color").unwrap_or(false) {
            let r = scope.get_value::<f64>("_ambient_color_r").unwrap_or(1.0) as f32;
            let g = scope.get_value::<f64>("_ambient_color_g").unwrap_or(1.0) as f32;
            let b = scope.get_value::<f64>("_ambient_color_b").unwrap_or(1.0) as f32;
            ctx.env_ambient_color = Some((r, g, b));
        }

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

        if scope.get_value::<bool>("_set_fog").unwrap_or(false) {
            let enabled = scope.get_value::<bool>("_fog_enabled").unwrap_or(false);
            let start = scope.get_value::<f64>("_fog_start").unwrap_or(10.0) as f32;
            let end = scope.get_value::<f64>("_fog_end").unwrap_or(100.0) as f32;
            ctx.env_fog_enabled = Some(enabled);
            ctx.env_fog_start = Some(start);
            ctx.env_fog_end = Some(end);
        }

        if scope.get_value::<bool>("_set_fog_color").unwrap_or(false) {
            let r = scope.get_value::<f64>("_fog_color_r").unwrap_or(0.5) as f32;
            let g = scope.get_value::<f64>("_fog_color_g").unwrap_or(0.5) as f32;
            let b = scope.get_value::<f64>("_fog_color_b").unwrap_or(0.5) as f32;
            ctx.env_fog_color = Some((r, g, b));
        }

        if scope.get_value::<bool>("_set_exposure").unwrap_or(false) {
            let val = scope.get_value::<f64>("_exposure").unwrap_or(1.0) as f32;
            ctx.env_exposure = Some(val);
        }
    }

    fn register_api(engine: &mut Engine) {
        // Helper functions that scripts can call

        // set_position(x, y, z) - sets new position
        engine.register_fn("set_position", |x: f64, y: f64, z: f64| -> Map {
            let mut m = Map::new();
            m.insert("_set_position".into(), Dynamic::from(true));
            m.insert("_new_position_x".into(), Dynamic::from(x));
            m.insert("_new_position_y".into(), Dynamic::from(y));
            m.insert("_new_position_z".into(), Dynamic::from(z));
            m
        });

        // set_rotation(x, y, z) - sets rotation in degrees
        engine.register_fn("set_rotation", |x: f64, y: f64, z: f64| -> Map {
            let mut m = Map::new();
            m.insert("_set_rotation".into(), Dynamic::from(true));
            m.insert("_new_rotation_x".into(), Dynamic::from(x));
            m.insert("_new_rotation_y".into(), Dynamic::from(y));
            m.insert("_new_rotation_z".into(), Dynamic::from(z));
            m
        });

        // translate(x, y, z) - moves by delta
        engine.register_fn("translate", |x: f64, y: f64, z: f64| -> Map {
            let mut m = Map::new();
            m.insert("_translate".into(), Dynamic::from(true));
            m.insert("_translate_x".into(), Dynamic::from(x));
            m.insert("_translate_y".into(), Dynamic::from(y));
            m.insert("_translate_z".into(), Dynamic::from(z));
            m
        });

        // print(msg) - prints to console
        engine.register_fn("print_log", |msg: ImmutableString| -> Map {
            let mut m = Map::new();
            m.insert("_print_message".into(), Dynamic::from(msg));
            m
        });

        // Math helpers
        engine.register_fn("lerp", |a: f64, b: f64, t: f64| -> f64 {
            a + (b - a) * t
        });

        engine.register_fn("clamp", |value: f64, min: f64, max: f64| -> f64 {
            value.max(min).min(max)
        });

        engine.register_fn("sin", |x: f64| -> f64 { x.sin() });
        engine.register_fn("cos", |x: f64| -> f64 { x.cos() });
        engine.register_fn("tan", |x: f64| -> f64 { x.tan() });
        engine.register_fn("sqrt", |x: f64| -> f64 { x.sqrt() });
        engine.register_fn("abs", |x: f64| -> f64 { x.abs() });
        engine.register_fn("floor", |x: f64| -> f64 { x.floor() });
        engine.register_fn("ceil", |x: f64| -> f64 { x.ceil() });
        engine.register_fn("round", |x: f64| -> f64 { x.round() });
        engine.register_fn("min", |a: f64, b: f64| -> f64 { a.min(b) });
        engine.register_fn("max", |a: f64, b: f64| -> f64 { a.max(b) });
        engine.register_fn("pow", |base: f64, exp: f64| -> f64 { base.powf(exp) });

        // Degrees/radians conversion
        engine.register_fn("deg_to_rad", |deg: f64| -> f64 { deg.to_radians() });
        engine.register_fn("rad_to_deg", |rad: f64| -> f64 { rad.to_degrees() });

        // Parent control functions (Godot-style)

        // parent.set_position(x, y, z) - sets parent's position
        engine.register_fn("parent_set_position", |x: f64, y: f64, z: f64| -> Map {
            let mut m = Map::new();
            m.insert("_parent_set_position".into(), Dynamic::from(true));
            m.insert("_parent_new_position_x".into(), Dynamic::from(x));
            m.insert("_parent_new_position_y".into(), Dynamic::from(y));
            m.insert("_parent_new_position_z".into(), Dynamic::from(z));
            m
        });

        // parent.set_rotation(x, y, z) - sets parent's rotation in degrees
        engine.register_fn("parent_set_rotation", |x: f64, y: f64, z: f64| -> Map {
            let mut m = Map::new();
            m.insert("_parent_set_rotation".into(), Dynamic::from(true));
            m.insert("_parent_new_rotation_x".into(), Dynamic::from(x));
            m.insert("_parent_new_rotation_y".into(), Dynamic::from(y));
            m.insert("_parent_new_rotation_z".into(), Dynamic::from(z));
            m
        });

        // parent.translate(x, y, z) - moves parent by delta
        engine.register_fn("parent_translate", |x: f64, y: f64, z: f64| -> Map {
            let mut m = Map::new();
            m.insert("_parent_translate".into(), Dynamic::from(true));
            m.insert("_parent_translate_x".into(), Dynamic::from(x));
            m.insert("_parent_translate_y".into(), Dynamic::from(y));
            m.insert("_parent_translate_z".into(), Dynamic::from(z));
            m
        });

        // Child control functions (Godot-style $ChildName access)
        // Usage: $ChildName.set_position(x, y, z) or call directly with name

        // set_child_position("ChildName", x, y, z) - sets child's position
        engine.register_fn("set_child_position", |name: ImmutableString, x: f64, y: f64, z: f64| -> Map {
            let mut m = Map::new();
            m.insert("_child_cmd".into(), Dynamic::from("set_position"));
            m.insert("_child_name".into(), Dynamic::from(name));
            m.insert("_child_x".into(), Dynamic::from(x));
            m.insert("_child_y".into(), Dynamic::from(y));
            m.insert("_child_z".into(), Dynamic::from(z));
            m
        });

        // set_child_rotation("ChildName", x, y, z) - sets child's rotation in degrees
        engine.register_fn("set_child_rotation", |name: ImmutableString, x: f64, y: f64, z: f64| -> Map {
            let mut m = Map::new();
            m.insert("_child_cmd".into(), Dynamic::from("set_rotation"));
            m.insert("_child_name".into(), Dynamic::from(name));
            m.insert("_child_x".into(), Dynamic::from(x));
            m.insert("_child_y".into(), Dynamic::from(y));
            m.insert("_child_z".into(), Dynamic::from(z));
            m
        });

        // child_translate("ChildName", x, y, z) - moves child by delta
        engine.register_fn("child_translate", |name: ImmutableString, x: f64, y: f64, z: f64| -> Map {
            let mut m = Map::new();
            m.insert("_child_cmd".into(), Dynamic::from("translate"));
            m.insert("_child_name".into(), Dynamic::from(name));
            m.insert("_child_x".into(), Dynamic::from(x));
            m.insert("_child_y".into(), Dynamic::from(y));
            m.insert("_child_z".into(), Dynamic::from(z));
            m
        });

        // Environment control functions

        // set_sun_angles(azimuth, elevation) - sets sun position in degrees
        engine.register_fn("set_sun_angles", |azimuth: f64, elevation: f64| -> Map {
            let mut m = Map::new();
            m.insert("_set_sun_angles".into(), Dynamic::from(true));
            m.insert("_sun_azimuth".into(), Dynamic::from(azimuth));
            m.insert("_sun_elevation".into(), Dynamic::from(elevation));
            m
        });

        // set_ambient_brightness(value) - sets ambient light brightness
        engine.register_fn("set_ambient_brightness", |value: f64| -> Map {
            let mut m = Map::new();
            m.insert("_set_ambient_brightness".into(), Dynamic::from(true));
            m.insert("_ambient_brightness".into(), Dynamic::from(value));
            m
        });

        // set_ambient_color(r, g, b) - sets ambient light color
        engine.register_fn("set_ambient_color", |r: f64, g: f64, b: f64| -> Map {
            let mut m = Map::new();
            m.insert("_set_ambient_color".into(), Dynamic::from(true));
            m.insert("_ambient_color_r".into(), Dynamic::from(r));
            m.insert("_ambient_color_g".into(), Dynamic::from(g));
            m.insert("_ambient_color_b".into(), Dynamic::from(b));
            m
        });

        // set_sky_top_color(r, g, b) - sets procedural sky top color
        engine.register_fn("set_sky_top_color", |r: f64, g: f64, b: f64| -> Map {
            let mut m = Map::new();
            m.insert("_set_sky_top_color".into(), Dynamic::from(true));
            m.insert("_sky_top_r".into(), Dynamic::from(r));
            m.insert("_sky_top_g".into(), Dynamic::from(g));
            m.insert("_sky_top_b".into(), Dynamic::from(b));
            m
        });

        // set_sky_horizon_color(r, g, b) - sets procedural sky horizon color
        engine.register_fn("set_sky_horizon_color", |r: f64, g: f64, b: f64| -> Map {
            let mut m = Map::new();
            m.insert("_set_sky_horizon_color".into(), Dynamic::from(true));
            m.insert("_sky_horizon_r".into(), Dynamic::from(r));
            m.insert("_sky_horizon_g".into(), Dynamic::from(g));
            m.insert("_sky_horizon_b".into(), Dynamic::from(b));
            m
        });

        // set_fog(enabled, start, end) - configures fog
        engine.register_fn("set_fog", |enabled: bool, start: f64, end: f64| -> Map {
            let mut m = Map::new();
            m.insert("_set_fog".into(), Dynamic::from(true));
            m.insert("_fog_enabled".into(), Dynamic::from(enabled));
            m.insert("_fog_start".into(), Dynamic::from(start));
            m.insert("_fog_end".into(), Dynamic::from(end));
            m
        });

        // set_fog_color(r, g, b) - sets fog color
        engine.register_fn("set_fog_color", |r: f64, g: f64, b: f64| -> Map {
            let mut m = Map::new();
            m.insert("_set_fog_color".into(), Dynamic::from(true));
            m.insert("_fog_color_r".into(), Dynamic::from(r));
            m.insert("_fog_color_g".into(), Dynamic::from(g));
            m.insert("_fog_color_b".into(), Dynamic::from(b));
            m
        });

        // set_exposure(value) - sets camera exposure
        engine.register_fn("set_exposure", |value: f64| -> Map {
            let mut m = Map::new();
            m.insert("_set_exposure".into(), Dynamic::from(true));
            m.insert("_exposure".into(), Dynamic::from(value));
            m
        });
    }
}

/// Context passed to Rhai scripts (simplified version for scope)
pub struct RhaiScriptContext {
    pub time: ScriptTime,
    pub transform: ScriptTransform,
    pub input_movement: Vec2,
    pub mouse_position: Vec2,
    pub mouse_delta: Vec2,

    // Gamepad input (gamepad 0)
    pub gamepad_left_stick: Vec2,
    pub gamepad_right_stick: Vec2,
    pub gamepad_left_trigger: f32,
    pub gamepad_right_trigger: f32,
    pub gamepad_buttons: [bool; 16], // Common buttons

    // Parent/child info
    pub has_parent: bool,
    pub parent_entity: Option<Entity>,
    pub parent_position: Vec3,
    pub parent_rotation: Vec3, // euler degrees
    pub parent_scale: Vec3,

    // Children info
    pub children: Vec<ChildNodeInfo>,

    // Outputs - Transform
    pub new_position: Option<Vec3>,
    pub new_rotation: Option<Vec3>,
    pub translation: Option<Vec3>,
    pub print_message: Option<String>,

    // Outputs - Parent transform
    pub parent_new_position: Option<Vec3>,
    pub parent_new_rotation: Option<Vec3>,
    pub parent_translation: Option<Vec3>,

    // Outputs - Child transforms (name -> changes)
    pub child_changes: HashMap<String, ChildChange>,

    // Outputs - Environment
    pub env_sun_azimuth: Option<f32>,
    pub env_sun_elevation: Option<f32>,
    pub env_ambient_brightness: Option<f32>,
    pub env_ambient_color: Option<(f32, f32, f32)>,
    pub env_sky_top_color: Option<(f32, f32, f32)>,
    pub env_sky_horizon_color: Option<(f32, f32, f32)>,
    pub env_fog_enabled: Option<bool>,
    pub env_fog_color: Option<(f32, f32, f32)>,
    pub env_fog_start: Option<f32>,
    pub env_fog_end: Option<f32>,
    pub env_exposure: Option<f32>,
}

impl RhaiScriptContext {
    pub fn new(time: ScriptTime, transform: ScriptTransform) -> Self {
        Self {
            time,
            transform,
            input_movement: Vec2::ZERO,
            mouse_position: Vec2::ZERO,
            mouse_delta: Vec2::ZERO,
            gamepad_left_stick: Vec2::ZERO,
            gamepad_right_stick: Vec2::ZERO,
            gamepad_left_trigger: 0.0,
            gamepad_right_trigger: 0.0,
            gamepad_buttons: [false; 16],
            has_parent: false,
            parent_entity: None,
            parent_position: Vec3::ZERO,
            parent_rotation: Vec3::ZERO,
            parent_scale: Vec3::ONE,
            children: Vec::new(),
            new_position: None,
            new_rotation: None,
            translation: None,
            print_message: None,
            parent_new_position: None,
            parent_new_rotation: None,
            parent_translation: None,
            child_changes: HashMap::new(),
            env_sun_azimuth: None,
            env_sun_elevation: None,
            env_ambient_brightness: None,
            env_ambient_color: None,
            env_sky_top_color: None,
            env_sky_horizon_color: None,
            env_fog_enabled: None,
            env_fog_color: None,
            env_fog_start: None,
            env_fog_end: None,
            env_exposure: None,
        }
    }
}
