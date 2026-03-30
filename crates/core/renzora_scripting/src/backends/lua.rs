use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use mlua::prelude::*;

use crate::backend::{FileReader, ScriptBackend};
use crate::command::ScriptCommand;
use crate::component::{ScriptValue, ScriptVariableDefinition, ScriptVariables};
use crate::context::ScriptContext;

/// Cached compiled Lua script
struct CachedScript {
    source: String,
    path: PathBuf,
    name: String,
    last_modified: std::time::SystemTime,
    props: Vec<ScriptVariableDefinition>,
}

use super::{push_command, drain_commands};

pub struct LuaBackend {
    scripts_folder: Option<PathBuf>,
    cache: Arc<RwLock<HashMap<PathBuf, CachedScript>>>,
    file_reader: Option<FileReader>,
}

impl LuaBackend {
    pub fn new() -> Self {
        Self {
            scripts_folder: None,
            cache: Arc::new(RwLock::new(HashMap::new())),
            file_reader: None,
        }
    }

    fn create_lua(&self) -> Lua {
        let lua = Lua::new();
        register_api(&lua);
        lua
    }

    fn load_script(&self, path: &Path) -> Result<(), String> {
        // Check cache (skip mtime check if using VFS — archives have no mtime)
        if let Ok(cache) = self.cache.read() {
            if let Some(cached) = cache.get(path) {
                if self.file_reader.is_some() {
                    // VFS mode: script is from rpak, no mtime to compare
                    return Ok(());
                }
                if let Ok(meta) = std::fs::metadata(path) {
                    if let Ok(modified) = meta.modified() {
                        if modified == cached.last_modified {
                            return Ok(());
                        }
                    }
                }
            }
        }

        // Try VFS file reader first, then fall back to filesystem
        let source = if let Some(ref reader) = self.file_reader {
            if let Some(s) = reader(path) {
                s
            } else {
                std::fs::read_to_string(path)
                    .map_err(|e| format!("Failed to read script '{}': {}", path.display(), e))?
            }
        } else {
            std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read script '{}': {}", path.display(), e))?
        };

        // Parse props by running the script in a temporary Lua state
        let props = self.parse_props(&source);

        let name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let last_modified = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        if let Ok(mut cache) = self.cache.write() {
            cache.insert(path.to_path_buf(), CachedScript {
                source,
                path: path.to_path_buf(),
                name,
                last_modified,
                props,
            });
        }

        Ok(())
    }

    fn parse_props(&self, source: &str) -> Vec<ScriptVariableDefinition> {
        let lua = self.create_lua();
        let mut props = Vec::new();

        // Execute the script to define functions
        if lua.load(source).exec().is_err() {
            return props;
        }

        // Call props() if it exists
        let globals = lua.globals();
        let props_fn: Result<LuaFunction, _> = globals.get("props");
        let Ok(func) = props_fn else { return props };

        let result: Result<LuaTable, _> = func.call(());
        let Ok(table) = result else { return props };

        for pair in table.pairs::<String, LuaValue>() {
            let Ok((name, value)) = pair else { continue };
            let display_name = to_display_name(&name);

            // Check if it's a table with "default" or "value" key
            if let LuaValue::Table(ref prop_table) = value {
                let default_val = prop_table.get::<LuaValue>("value")
                    .or_else(|_| prop_table.get::<LuaValue>("default"));

                if let Ok(ref default_val) = default_val {
                    if let Some(sv) = lua_to_script_value(default_val) {
                        let hint: Option<String> = prop_table.get("hint").ok();
                        let mut def = ScriptVariableDefinition::new(name, sv)
                            .with_display_name(display_name);
                        if let Some(h) = hint {
                            def = def.with_hint(h);
                        }
                        props.push(def);
                        continue;
                    }
                }
            }

            if let Some(sv) = lua_to_script_value(&value) {
                props.push(
                    ScriptVariableDefinition::new(name, sv)
                        .with_display_name(display_name),
                );
            }
        }

        props.sort_by(|a, b| a.name.cmp(&b.name));
        props
    }

    fn execute_hook(
        &self,
        path: &Path,
        hook: &str,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        self.load_script(path)?;

        let source = {
            let cache = self.cache.read().map_err(|e| e.to_string())?;
            let cached = cache.get(path)
                .ok_or_else(|| format!("Script not in cache: {}", path.display()))?;
            cached.source.clone()
        };

        let lua = self.create_lua();

        // Register extension functions (once per state creation)
        if let Some(extensions) = ctx.extensions() {
            extensions.register_lua_functions(&lua);
        }

        // Set up globals
        set_context_globals(&lua, ctx, vars);

        // Set up extension context (per-frame data)
        if let Some(extensions) = ctx.extensions() {
            extensions.setup_lua_context(&lua, &ctx.extension_data);
        }

        // Drain stale commands
        drain_commands();

        // Load and execute the script to define functions
        lua.load(&source).exec()
            .map_err(|e| format!("Lua error: {}", e))?;

        // Call the hook function
        let globals = lua.globals();
        let func: Result<LuaFunction, _> = globals.get(hook);
        if let Ok(func) = func {
            func.call::<()>(())
                .map_err(|e| {
                    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                    format!("{} {}: {}", name, hook, e)
                })?;
        }

        // Read back variables
        read_back_variables(&lua, vars);

        Ok(drain_commands())
    }
}

impl ScriptBackend for LuaBackend {
    fn name(&self) -> &str { "Lua" }

    fn extensions(&self) -> &[&str] { &["lua"] }

    fn set_scripts_folder(&mut self, path: PathBuf) {
        self.scripts_folder = Some(path);
    }

    fn set_file_reader(&mut self, reader: FileReader) {
        self.file_reader = Some(reader);
    }

    fn get_available_scripts(&self) -> Vec<(String, PathBuf)> {
        let Some(folder) = &self.scripts_folder else { return Vec::new() };
        let mut scripts = Vec::new();
        if let Ok(entries) = std::fs::read_dir(folder) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "lua") {
                    let name = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    scripts.push((name, path));
                }
            }
        }
        scripts
    }

    fn get_script_props(&self, path: &Path) -> Vec<ScriptVariableDefinition> {
        let _ = self.load_script(path);
        let cache = self.cache.read().ok();
        cache.and_then(|c| c.get(path).map(|s| s.props.clone()))
            .unwrap_or_default()
    }

    fn call_on_ready(
        &self,
        path: &Path,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        self.execute_hook(path, "on_ready", ctx, vars)
    }

    fn call_on_update(
        &self,
        path: &Path,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        self.execute_hook(path, "on_update", ctx, vars)
    }

    fn needs_reload(&self, path: &Path) -> bool {
        let cache = match self.cache.read() { Ok(c) => c, Err(_) => return false };
        // Not in cache = never loaded yet, not a "reload" scenario
        let Some(cached) = cache.get(path) else { return false };
        // VFS/rpak scripts don't change at runtime — no reload needed once cached
        if self.file_reader.is_some() { return false; }
        let Ok(meta) = std::fs::metadata(path) else { return false };
        let Ok(modified) = meta.modified() else { return false };
        modified != cached.last_modified
    }

    fn reload(&self, path: &Path) -> Result<(), String> {
        if let Ok(mut cache) = self.cache.write() {
            cache.remove(path);
        }
        self.load_script(path)
    }

    fn eval_expression(&self, expr: &str) -> Result<String, String> {
        let lua = self.create_lua();
        drain_commands();
        match lua.load(expr).eval::<LuaValue>() {
            Ok(val) => {
                let _ = drain_commands();
                Ok(lua_value_to_string(&val))
            }
            Err(e) => Err(format!("{}", e)),
        }
    }
}

// =============================================================================
// Lua API registration
// =============================================================================

fn register_api(lua: &Lua) {
    let globals = lua.globals();

    // -- Transform --
    register_fn3(lua, &globals, "set_position", |x, y, z| {
        push_command(ScriptCommand::SetPosition { x, y, z });
    });
    register_fn3(lua, &globals, "set_rotation", |x, y, z| {
        push_command(ScriptCommand::SetRotation { x, y, z });
    });
    register_fn3(lua, &globals, "set_scale", |x, y, z| {
        push_command(ScriptCommand::SetScale { x, y, z });
    });
    register_fn1(lua, &globals, "set_scale_uniform", |s: f32| {
        push_command(ScriptCommand::SetScale { x: s, y: s, z: s });
    });
    register_fn3(lua, &globals, "translate", |x, y, z| {
        push_command(ScriptCommand::Translate { x, y, z });
    });
    register_fn3(lua, &globals, "rotate", |x, y, z| {
        push_command(ScriptCommand::Rotate { x, y, z });
    });
    register_fn3(lua, &globals, "look_at", |x, y, z| {
        push_command(ScriptCommand::LookAt { x, y, z });
    });

    // -- Parent transform --
    register_fn3(lua, &globals, "parent_set_position", |x, y, z| {
        push_command(ScriptCommand::ParentSetPosition { x, y, z });
    });
    register_fn3(lua, &globals, "parent_set_rotation", |x, y, z| {
        push_command(ScriptCommand::ParentSetRotation { x, y, z });
    });
    register_fn3(lua, &globals, "parent_translate", |x, y, z| {
        push_command(ScriptCommand::ParentTranslate { x, y, z });
    });

    // -- Child transform --
    let _ = globals.set("set_child_position", lua.create_function(|_, (name, x, y, z): (String, f32, f32, f32)| {
        push_command(ScriptCommand::ChildSetPosition { name, x, y, z });
        Ok(())
    }).unwrap());
    let _ = globals.set("set_child_rotation", lua.create_function(|_, (name, x, y, z): (String, f32, f32, f32)| {
        push_command(ScriptCommand::ChildSetRotation { name, x, y, z });
        Ok(())
    }).unwrap());
    let _ = globals.set("child_translate", lua.create_function(|_, (name, x, y, z): (String, f32, f32, f32)| {
        push_command(ScriptCommand::ChildTranslate { name, x, y, z });
        Ok(())
    }).unwrap());

    // -- Input --
    let _ = globals.set("is_key_pressed", lua.create_function(|lua, key: String| {
        let keys: LuaTable = lua.globals().get("_keys_pressed")?;
        let pressed: bool = keys.get(key).unwrap_or(false);
        Ok(pressed)
    }).unwrap());
    let _ = globals.set("is_key_just_pressed", lua.create_function(|lua, key: String| {
        let keys: LuaTable = lua.globals().get("_keys_just_pressed")?;
        let pressed: bool = keys.get(key).unwrap_or(false);
        Ok(pressed)
    }).unwrap());
    let _ = globals.set("is_key_just_released", lua.create_function(|lua, key: String| {
        let keys: LuaTable = lua.globals().get("_keys_just_released")?;
        let pressed: bool = keys.get(key).unwrap_or(false);
        Ok(pressed)
    }).unwrap());

    // -- Audio --
    let _ = globals.set("play_sound", lua.create_function(|_, args: LuaMultiValue| {
        let path: String = args.get(0).and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default();
        let volume: f32 = args.get(1).and_then(|v| v.as_f32()).unwrap_or(1.0);
        let bus: String = args.get(2).and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_else(|| "Sfx".into());
        push_command(ScriptCommand::PlaySound { path, volume, looping: false, bus });
        Ok(())
    }).unwrap());
    let _ = globals.set("play_sound_looping", lua.create_function(|_, (path, volume): (String, f32)| {
        push_command(ScriptCommand::PlaySound { path, volume, looping: true, bus: "Sfx".into() });
        Ok(())
    }).unwrap());
    let _ = globals.set("play_music", lua.create_function(|_, args: LuaMultiValue| {
        let path: String = args.get(0).and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default();
        let volume: f32 = args.get(1).and_then(|v| v.as_f32()).unwrap_or(1.0);
        let fade_in: f32 = args.get(2).and_then(|v| v.as_f32()).unwrap_or(0.0);
        push_command(ScriptCommand::PlayMusic { path, volume, fade_in, bus: "Music".into() });
        Ok(())
    }).unwrap());
    let _ = globals.set("stop_music", lua.create_function(|_, fade_out: Option<f32>| {
        push_command(ScriptCommand::StopMusic { fade_out: fade_out.unwrap_or(0.0) });
        Ok(())
    }).unwrap());
    let _ = globals.set("stop_all_sounds", lua.create_function(|_, ()| {
        push_command(ScriptCommand::StopAllSounds);
        Ok(())
    }).unwrap());

    // -- Physics --
    let _ = globals.set("apply_force", lua.create_function(|_, (x, y, z): (f32, f32, f32)| {
        push_command(ScriptCommand::ApplyForce { entity_id: None, force: bevy::prelude::Vec3::new(x, y, z) });
        Ok(())
    }).unwrap());
    let _ = globals.set("apply_impulse", lua.create_function(|_, (x, y, z): (f32, f32, f32)| {
        push_command(ScriptCommand::ApplyImpulse { entity_id: None, impulse: bevy::prelude::Vec3::new(x, y, z) });
        Ok(())
    }).unwrap());
    let _ = globals.set("set_velocity", lua.create_function(|_, (x, y, z): (f32, f32, f32)| {
        push_command(ScriptCommand::SetVelocity { entity_id: None, velocity: bevy::prelude::Vec3::new(x, y, z) });
        Ok(())
    }).unwrap());
    let _ = globals.set("set_gravity_scale", lua.create_function(|_, scale: f32| {
        push_command(ScriptCommand::SetGravityScale { entity_id: None, scale });
        Ok(())
    }).unwrap());

    // -- Timers --
    let _ = globals.set("start_timer", lua.create_function(|_, (name, duration, repeat): (String, f32, Option<bool>)| {
        push_command(ScriptCommand::StartTimer { name, duration, repeat: repeat.unwrap_or(false) });
        Ok(())
    }).unwrap());
    let _ = globals.set("stop_timer", lua.create_function(|_, name: String| {
        push_command(ScriptCommand::StopTimer { name });
        Ok(())
    }).unwrap());

    // -- Debug --
    let _ = globals.set("print_log", lua.create_function(|_, msg: String| {
        push_command(ScriptCommand::Log { level: "Info".into(), message: msg });
        Ok(())
    }).unwrap());
    let _ = globals.set("draw_line", lua.create_function(|_, (sx, sy, sz, ex, ey, ez, duration): (f32, f32, f32, f32, f32, f32, Option<f32>)| {
        push_command(ScriptCommand::DrawLine {
            start: bevy::prelude::Vec3::new(sx, sy, sz),
            end: bevy::prelude::Vec3::new(ex, ey, ez),
            color: [1.0, 0.0, 0.0, 1.0],
            duration: duration.unwrap_or(0.0),
        });
        Ok(())
    }).unwrap());

    // -- Rendering --
    let _ = globals.set("set_visibility", lua.create_function(|_, visible: bool| {
        push_command(ScriptCommand::SetVisibility { entity_id: None, visible });
        Ok(())
    }).unwrap());
    let _ = globals.set("set_material_color", lua.create_function(|_, (r, g, b, a): (f32, f32, f32, Option<f32>)| {
        push_command(ScriptCommand::SetMaterialColor { entity_id: None, color: [r, g, b, a.unwrap_or(1.0)] });
        Ok(())
    }).unwrap());

    // -- Animation --
    let _ = globals.set("play_animation", lua.create_function(|_, (name, looping, speed): (String, Option<bool>, Option<f32>)| {
        push_command(ScriptCommand::PlayAnimation { entity_id: None, name, looping: looping.unwrap_or(true), speed: speed.unwrap_or(1.0) });
        Ok(())
    }).unwrap());
    let _ = globals.set("stop_animation", lua.create_function(|_, ()| {
        push_command(ScriptCommand::StopAnimation { entity_id: None });
        Ok(())
    }).unwrap());
    let _ = globals.set("pause_animation", lua.create_function(|_, ()| {
        push_command(ScriptCommand::PauseAnimation { entity_id: None });
        Ok(())
    }).unwrap());
    let _ = globals.set("resume_animation", lua.create_function(|_, ()| {
        push_command(ScriptCommand::ResumeAnimation { entity_id: None });
        Ok(())
    }).unwrap());
    let _ = globals.set("set_animation_speed", lua.create_function(|_, speed: f32| {
        push_command(ScriptCommand::SetAnimationSpeed { entity_id: None, speed });
        Ok(())
    }).unwrap());
    let _ = globals.set("crossfade_animation", lua.create_function(|_, (name, duration, looping): (String, f32, Option<bool>)| {
        push_command(ScriptCommand::CrossfadeAnimation { entity_id: None, name, duration, looping: looping.unwrap_or(true) });
        Ok(())
    }).unwrap());
    let _ = globals.set("set_anim_param", lua.create_function(|_, (name, value): (String, f32)| {
        push_command(ScriptCommand::SetAnimationParam { entity_id: None, name, value });
        Ok(())
    }).unwrap());
    let _ = globals.set("set_anim_bool", lua.create_function(|_, (name, value): (String, bool)| {
        push_command(ScriptCommand::SetAnimationBoolParam { entity_id: None, name, value });
        Ok(())
    }).unwrap());
    let _ = globals.set("trigger_anim", lua.create_function(|_, name: String| {
        push_command(ScriptCommand::TriggerAnimation { entity_id: None, name });
        Ok(())
    }).unwrap());
    let _ = globals.set("set_layer_weight", lua.create_function(|_, (layer_name, weight): (String, f32)| {
        push_command(ScriptCommand::SetAnimationLayerWeight { entity_id: None, layer_name, weight });
        Ok(())
    }).unwrap());

    // -- Camera --
    let _ = globals.set("screen_shake", lua.create_function(|_, (intensity, duration): (f32, f32)| {
        push_command(ScriptCommand::ScreenShake { intensity, duration });
        Ok(())
    }).unwrap());

    // -- ECS --
    let _ = globals.set("spawn_entity", lua.create_function(|_, name: String| {
        push_command(ScriptCommand::SpawnEntity { name });
        Ok(())
    }).unwrap());
    let _ = globals.set("despawn_self", lua.create_function(|_, ()| {
        push_command(ScriptCommand::DespawnSelf);
        Ok(())
    }).unwrap());

    // -- Scene --
    let _ = globals.set("load_scene", lua.create_function(|_, path: String| {
        push_command(ScriptCommand::LoadScene { path });
        Ok(())
    }).unwrap());

    // -- Environment --
    let _ = globals.set("set_sun_angles", lua.create_function(|_, (azimuth, elevation): (f32, f32)| {
        push_command(ScriptCommand::SetSunAngles { azimuth, elevation });
        Ok(())
    }).unwrap());
    let _ = globals.set("set_fog", lua.create_function(|_, (enabled, start, end): (bool, f32, f32)| {
        push_command(ScriptCommand::SetFog { enabled, start, end });
        Ok(())
    }).unwrap());

    // -- Generic Reflection (set/set_on) --
    // set("ComponentType.field.subfield", value) — on self entity
    let _ = globals.set("set", lua.create_function(|_, (path, value): (String, LuaValue)| {
        let (component, field) = parse_component_path(&path)
            .ok_or_else(|| mlua::Error::runtime(format!("Invalid path '{}'. Use 'Component.field'", path)))?;
        push_command(ScriptCommand::SetComponentField {
            entity_id: None,
            entity_name: None,
            component_type: component,
            field_path: field,
            value: lua_to_property_value(&value),
        });
        Ok(())
    }).unwrap());

    // set_on("EntityName", "ComponentType.field.subfield", value) — on named entity
    let _ = globals.set("set_on", lua.create_function(|_, (entity_name, path, value): (String, String, LuaValue)| {
        let (component, field) = parse_component_path(&path)
            .ok_or_else(|| mlua::Error::runtime(format!("Invalid path '{}'. Use 'Component.field'", path)))?;
        push_command(ScriptCommand::SetComponentField {
            entity_id: None,
            entity_name: Some(entity_name),
            component_type: component,
            field_path: field,
            value: lua_to_property_value(&value),
        });
        Ok(())
    }).unwrap());

    // -- Generic Reflection (get/get_on) --
    // get("Component.field") — read from self entity
    let _ = globals.set("get", lua.create_function(|lua, path: String| {
        let (component, field) = parse_component_path(&path)
            .ok_or_else(|| mlua::Error::runtime(format!("Invalid path '{}'. Use 'Component.field'", path)))?;
        match crate::get_handler::call_get(None, &component, &field) {
            Some(v) => property_value_to_lua_result(lua, v),
            None => Ok(LuaValue::Nil),
        }
    }).unwrap());

    // get_on("EntityName", "Component.field") — read from named entity
    let _ = globals.set("get_on", lua.create_function(|lua, (entity_name, path): (String, String)| {
        let (component, field) = parse_component_path(&path)
            .ok_or_else(|| mlua::Error::runtime(format!("Invalid path '{}'. Use 'Component.field'", path)))?;
        match crate::get_handler::call_get(Some(&entity_name), &component, &field) {
            Some(v) => property_value_to_lua_result(lua, v),
            None => Ok(LuaValue::Nil),
        }
    }).unwrap());

    // -- Math helpers --
    let _ = globals.set("vec3", lua.create_function(|lua, (x, y, z): (f32, f32, f32)| {
        let t = lua.create_table()?;
        t.set("x", x)?;
        t.set("y", y)?;
        t.set("z", z)?;
        Ok(t)
    }).unwrap());
    let _ = globals.set("vec2", lua.create_function(|lua, (x, y): (f32, f32)| {
        let t = lua.create_table()?;
        t.set("x", x)?;
        t.set("y", y)?;
        Ok(t)
    }).unwrap());
    let _ = globals.set("lerp", lua.create_function(|_, (a, b, t): (f32, f32, f32)| {
        Ok(a + (b - a) * t)
    }).unwrap());
    let _ = globals.set("clamp", lua.create_function(|_, (v, min, max): (f32, f32, f32)| {
        Ok(v.max(min).min(max))
    }).unwrap());
}

// Helper to register a 3-arg (f32, f32, f32) -> () function
fn register_fn3(lua: &Lua, globals: &LuaTable, name: &str, f: fn(f32, f32, f32)) {
    let _ = globals.set(name, lua.create_function(move |_, (x, y, z): (f32, f32, f32)| {
        f(x, y, z);
        Ok(())
    }).unwrap());
}

fn register_fn1(lua: &Lua, globals: &LuaTable, name: &str, f: fn(f32)) {
    let _ = globals.set(name, lua.create_function(move |_, v: f32| {
        f(v);
        Ok(())
    }).unwrap());
}

// =============================================================================
// Context marshalling
// =============================================================================

fn set_context_globals(lua: &Lua, ctx: &ScriptContext, vars: &ScriptVariables) {
    let g = lua.globals();

    // Time
    let _ = g.set("delta", ctx.time.delta as f64);
    let _ = g.set("elapsed", ctx.time.elapsed);

    // Transform
    let _ = g.set("position_x", ctx.transform.position.x as f64);
    let _ = g.set("position_y", ctx.transform.position.y as f64);
    let _ = g.set("position_z", ctx.transform.position.z as f64);
    let euler = ctx.transform.euler_degrees();
    let _ = g.set("rotation_x", euler.x as f64);
    let _ = g.set("rotation_y", euler.y as f64);
    let _ = g.set("rotation_z", euler.z as f64);
    let _ = g.set("scale_x", ctx.transform.scale.x as f64);
    let _ = g.set("scale_y", ctx.transform.scale.y as f64);
    let _ = g.set("scale_z", ctx.transform.scale.z as f64);

    // Input
    let _ = g.set("input_x", ctx.input_movement.x as f64);
    let _ = g.set("input_y", ctx.input_movement.y as f64);
    let _ = g.set("mouse_x", ctx.mouse_position.x as f64);
    let _ = g.set("mouse_y", ctx.mouse_position.y as f64);
    let _ = g.set("mouse_delta_x", ctx.mouse_delta.x as f64);
    let _ = g.set("mouse_delta_y", ctx.mouse_delta.y as f64);
    let _ = g.set("camera_yaw", ctx.camera_yaw as f64);

    // Mouse buttons
    let _ = g.set("mouse_left", ctx.mouse_buttons_pressed[0]);
    let _ = g.set("mouse_right", ctx.mouse_buttons_pressed[1]);
    let _ = g.set("mouse_middle", ctx.mouse_buttons_pressed[2]);
    let _ = g.set("mouse_left_just_pressed", ctx.mouse_buttons_just_pressed[0]);
    let _ = g.set("mouse_right_just_pressed", ctx.mouse_buttons_just_pressed[1]);
    let _ = g.set("mouse_scroll", ctx.mouse_scroll as f64);

    // Gamepad
    let _ = g.set("gamepad_left_x", ctx.gamepad_left_stick.x as f64);
    let _ = g.set("gamepad_left_y", ctx.gamepad_left_stick.y as f64);
    let _ = g.set("gamepad_right_x", ctx.gamepad_right_stick.x as f64);
    let _ = g.set("gamepad_right_y", ctx.gamepad_right_stick.y as f64);
    let _ = g.set("gamepad_left_trigger", ctx.gamepad_left_trigger as f64);
    let _ = g.set("gamepad_right_trigger", ctx.gamepad_right_trigger as f64);
    // Buttons: South(X/A), East(O/B), West(□/X), North(△/Y),
    //          L1, R1, L2, R2, Select, Start, L3, R3,
    //          DPadUp, DPadDown, DPadLeft, DPadRight
    let _ = g.set("gamepad_south", ctx.gamepad_buttons[0]);
    let _ = g.set("gamepad_east", ctx.gamepad_buttons[1]);
    let _ = g.set("gamepad_west", ctx.gamepad_buttons[2]);
    let _ = g.set("gamepad_north", ctx.gamepad_buttons[3]);
    let _ = g.set("gamepad_l1", ctx.gamepad_buttons[4]);
    let _ = g.set("gamepad_r1", ctx.gamepad_buttons[5]);
    let _ = g.set("gamepad_l2", ctx.gamepad_buttons[6]);
    let _ = g.set("gamepad_r2", ctx.gamepad_buttons[7]);
    let _ = g.set("gamepad_select", ctx.gamepad_buttons[8]);
    let _ = g.set("gamepad_start", ctx.gamepad_buttons[9]);
    let _ = g.set("gamepad_l3", ctx.gamepad_buttons[10]);
    let _ = g.set("gamepad_r3", ctx.gamepad_buttons[11]);
    let _ = g.set("gamepad_dpad_up", ctx.gamepad_buttons[12]);
    let _ = g.set("gamepad_dpad_down", ctx.gamepad_buttons[13]);
    let _ = g.set("gamepad_dpad_left", ctx.gamepad_buttons[14]);
    let _ = g.set("gamepad_dpad_right", ctx.gamepad_buttons[15]);

    // Entity
    let _ = g.set("self_entity_id", ctx.self_entity_id as i64);
    let _ = g.set("self_entity_name", ctx.self_entity_name.clone());

    // Keyboard maps
    if let Ok(keys_table) = lua.create_table() {
        for (key, &pressed) in &ctx.keys_pressed {
            let _ = keys_table.set(key.clone(), pressed);
        }
        let _ = g.set("_keys_pressed", keys_table);
    }
    if let Ok(keys_table) = lua.create_table() {
        for (key, &pressed) in &ctx.keys_just_pressed {
            let _ = keys_table.set(key.clone(), pressed);
        }
        let _ = g.set("_keys_just_pressed", keys_table);
    }
    if let Ok(keys_table) = lua.create_table() {
        for (key, &released) in &ctx.keys_just_released {
            let _ = keys_table.set(key.clone(), released);
        }
        let _ = g.set("_keys_just_released", keys_table);
    }

    // Collisions
    let _ = g.set("is_colliding", !ctx.active_collisions.is_empty());

    // Timers
    if let Ok(t) = lua.create_table() {
        for (i, name) in ctx.timers_just_finished.iter().enumerate() {
            let _ = t.set(i + 1, name.clone());
        }
        let _ = g.set("timers_finished", t);
    }

    // Health
    let _ = g.set("self_health", ctx.self_health as f64);
    let _ = g.set("self_max_health", ctx.self_max_health as f64);

    // Parent
    let _ = g.set("has_parent", ctx.has_parent);
    let _ = g.set("parent_position_x", ctx.parent_position.x as f64);
    let _ = g.set("parent_position_y", ctx.parent_position.y as f64);
    let _ = g.set("parent_position_z", ctx.parent_position.z as f64);

    // Script variables as globals
    for (key, value) in vars.iter_all() {
        match value {
            ScriptValue::Float(v) => { let _ = g.set(key.as_str(), *v as f64); }
            ScriptValue::Int(v) => { let _ = g.set(key.as_str(), *v as i64); }
            ScriptValue::Bool(v) => { let _ = g.set(key.as_str(), *v); }
            ScriptValue::String(v) => { let _ = g.set(key.as_str(), v.clone()); }
            ScriptValue::Entity(v) => { let _ = g.set(key.as_str(), v.clone()); }
            ScriptValue::Vec2(v) => {
                if let Ok(t) = lua.create_table() {
                    let _ = t.set("x", v.x as f64);
                    let _ = t.set("y", v.y as f64);
                    let _ = g.set(key.as_str(), t);
                }
            }
            ScriptValue::Vec3(v) => {
                if let Ok(t) = lua.create_table() {
                    let _ = t.set("x", v.x as f64);
                    let _ = t.set("y", v.y as f64);
                    let _ = t.set("z", v.z as f64);
                    let _ = g.set(key.as_str(), t);
                }
            }
            ScriptValue::Color(v) => {
                if let Ok(t) = lua.create_table() {
                    let _ = t.set("r", v.x as f64);
                    let _ = t.set("g", v.y as f64);
                    let _ = t.set("b", v.z as f64);
                    let _ = t.set("a", v.w as f64);
                    let _ = g.set(key.as_str(), t);
                }
            }
        }
    }
}

fn read_back_variables(lua: &Lua, vars: &mut ScriptVariables) {
    let g = lua.globals();
    let var_names: Vec<String> = vars.iter_all().map(|(k, _)| k.clone()).collect();
    for name in &var_names {
        if let Ok(value) = g.get::<LuaValue>(name.as_str()) {
            if let Some(sv) = lua_to_script_value(&value) {
                vars.set(name.clone(), sv);
            }
        }
    }
}

fn lua_to_script_value(value: &LuaValue) -> Option<ScriptValue> {
    match value {
        LuaValue::Number(n) => Some(ScriptValue::Float(*n as f32)),
        LuaValue::Integer(n) => Some(ScriptValue::Int(*n as i32)),
        LuaValue::Boolean(b) => Some(ScriptValue::Bool(*b)),
        LuaValue::String(s) => Some(ScriptValue::String(s.to_str().ok()?.to_string())),
        LuaValue::Table(t) => {
            // Check for vec2/vec3/color
            if let (Ok(x), Ok(y)) = (t.get::<f64>("x"), t.get::<f64>("y")) {
                if let Ok(z) = t.get::<f64>("z") {
                    return Some(ScriptValue::Vec3(bevy::prelude::Vec3::new(x as f32, y as f32, z as f32)));
                }
                return Some(ScriptValue::Vec2(bevy::prelude::Vec2::new(x as f32, y as f32)));
            }
            if let (Ok(r), Ok(g), Ok(b)) = (t.get::<f64>("r"), t.get::<f64>("g"), t.get::<f64>("b")) {
                let a: f64 = t.get("a").unwrap_or(1.0);
                return Some(ScriptValue::Color(bevy::prelude::Vec4::new(r as f32, g as f32, b as f32, a as f32)));
            }
            None
        }
        _ => None,
    }
}

fn lua_value_to_string(value: &LuaValue) -> String {
    match value {
        LuaValue::Nil => "nil".into(),
        LuaValue::Boolean(b) => b.to_string(),
        LuaValue::Integer(n) => n.to_string(),
        LuaValue::Number(n) => n.to_string(),
        LuaValue::String(s) => s.to_str().map(|s| s.to_string()).unwrap_or_default(),
        _ => format!("{:?}", value),
    }
}

/// Parse "ComponentType.field.subfield" into ("ComponentType", "field.subfield")
fn parse_component_path(path: &str) -> Option<(String, String)> {
    let dot = path.find('.')?;
    let component = path[..dot].to_string();
    let field = path[dot + 1..].to_string();
    if component.is_empty() || field.is_empty() {
        return None;
    }
    Some((component, field))
}

/// Convert a Lua value to PropertyValue for reflection writes.
fn lua_to_property_value(value: &LuaValue) -> crate::command::PropertyValue {
    use crate::command::PropertyValue;
    match value {
        LuaValue::Number(n) => PropertyValue::Float(*n as f32),
        LuaValue::Integer(n) => PropertyValue::Int(*n),
        LuaValue::Boolean(b) => PropertyValue::Bool(*b),
        LuaValue::String(s) => PropertyValue::String(s.to_str().map(|s| s.to_string()).unwrap_or_default()),
        LuaValue::Table(t) => {
            // Check for vec3 {x, y, z}
            if let (Ok(x), Ok(y), Ok(z)) = (t.get::<f64>("x"), t.get::<f64>("y"), t.get::<f64>("z")) {
                return PropertyValue::Vec3([x as f32, y as f32, z as f32]);
            }
            // Check for color {r, g, b, a}
            if let (Ok(r), Ok(g), Ok(b)) = (t.get::<f64>("r"), t.get::<f64>("g"), t.get::<f64>("b")) {
                let a: f64 = t.get("a").unwrap_or(1.0);
                return PropertyValue::Color([r as f32, g as f32, b as f32, a as f32]);
            }
            // Check for array-style {r, g, b, a} or {x, y, z}
            if let (Ok(v1), Ok(v2), Ok(v3)) = (t.get::<f64>(1), t.get::<f64>(2), t.get::<f64>(3)) {
                if let Ok(v4) = t.get::<f64>(4) {
                    return PropertyValue::Color([v1 as f32, v2 as f32, v3 as f32, v4 as f32]);
                }
                return PropertyValue::Vec3([v1 as f32, v2 as f32, v3 as f32]);
            }
            PropertyValue::Float(0.0)
        }
        _ => PropertyValue::Float(0.0),
    }
}

/// Convert a PropertyValue to a Lua value (requires Lua context for strings/tables).
fn property_value_to_lua_result(lua: &Lua, value: crate::command::PropertyValue) -> LuaResult<LuaValue> {
    use crate::command::PropertyValue;
    match value {
        PropertyValue::Float(v) => Ok(LuaValue::Number(v as f64)),
        PropertyValue::Int(v) => Ok(LuaValue::Integer(v)),
        PropertyValue::Bool(v) => Ok(LuaValue::Boolean(v)),
        PropertyValue::String(v) => Ok(LuaValue::String(lua.create_string(&v)?)),
        PropertyValue::Vec3(v) => {
            let t = lua.create_table()?;
            t.set("x", v[0] as f64)?;
            t.set("y", v[1] as f64)?;
            t.set("z", v[2] as f64)?;
            Ok(LuaValue::Table(t))
        }
        PropertyValue::Color(v) => {
            let t = lua.create_table()?;
            t.set("r", v[0] as f64)?;
            t.set("g", v[1] as f64)?;
            t.set("b", v[2] as f64)?;
            t.set("a", v[3] as f64)?;
            Ok(LuaValue::Table(t))
        }
    }
}

/// Extract a string argument from a LuaMultiValue by index.

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
