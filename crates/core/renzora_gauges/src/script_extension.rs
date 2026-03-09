//! Gauge script extension — registers gauge functions (gauge_get, gauge_set,
//! gauge_damage, etc.) with the scripting system via the extension API.

use bevy::prelude::*;
use renzora_scripting::extension::{ExtensionData, ScriptExtension};
use renzora_scripting::macros::push_ext_command;
use std::collections::HashMap;

// ── Extension data ───────────────────────────────────────────────────────

/// Per-entity gauge data injected into scripts.
#[derive(Clone, Default)]
pub struct GaugeContextData {
    pub attributes: HashMap<String, f32>,
    pub all_attributes: HashMap<u64, HashMap<String, f32>>,
}

// ── Extension commands ───────────────────────────────────────────────────

renzora_scripting::script_extension_command! {
    #[derive(Debug)]
    pub enum GaugeCommand {
        Set { attribute: String, value: f32, target: Option<u64> },
        AddModifier { attribute: String, value: f32, target: Option<u64> },
        RemoveModifier { attribute: String, value: f32, target: Option<u64> },
        AddExprModifier { attribute: String, expression: String, target: Option<u64> },
        Instant { attribute: String, op: String, value: f32, target: Option<u64> },
        InstantExpr {
            attribute: String,
            op: String,
            expression: String,
            roles: Vec<(String, u64)>,
            target: Option<u64>,
        },
    }
}

// ── Common command functions (shared by both backends) ────────────────────

renzora_scripting::dual_register! {
    lua_fn = register_gauge_lua,
    rhai_fn = register_gauge_rhai,

    fn gauge_set(name: String, value: f64) {
        push_ext_command(GaugeCommand::Set {
            attribute: name, value: value as f32, target: None,
        });
    }

    fn gauge_add_modifier(name: String, value: f64) {
        push_ext_command(GaugeCommand::AddModifier {
            attribute: name, value: value as f32, target: None,
        });
    }

    fn gauge_remove_modifier(name: String, value: f64) {
        push_ext_command(GaugeCommand::RemoveModifier {
            attribute: name, value: value as f32, target: None,
        });
    }

    fn gauge_add_expr_modifier(name: String, expr: String) {
        push_ext_command(GaugeCommand::AddExprModifier {
            attribute: name, expression: expr, target: None,
        });
    }

    fn gauge_instant(name: String, op: String, value: f64) {
        push_ext_command(GaugeCommand::Instant {
            attribute: name, op, value: value as f32, target: None,
        });
    }

    fn gauge_damage(name: String, amount: f64) {
        push_ext_command(GaugeCommand::Instant {
            attribute: name, op: "subtract".into(), value: amount as f32, target: None,
        });
    }

    fn gauge_heal(name: String, amount: f64) {
        push_ext_command(GaugeCommand::Instant {
            attribute: name, op: "add".into(), value: amount as f32, target: None,
        });
    }

    fn gauge_set_target(target_id: i64, name: String, value: f64) {
        push_ext_command(GaugeCommand::Set {
            attribute: name, value: value as f32, target: Some(target_id as u64),
        });
    }

    fn gauge_damage_target(target_id: i64, name: String, amount: f64) {
        push_ext_command(GaugeCommand::Instant {
            attribute: name, op: "subtract".into(),
            value: amount as f32, target: Some(target_id as u64),
        });
    }

    fn gauge_heal_target(target_id: i64, name: String, amount: f64) {
        push_ext_command(GaugeCommand::Instant {
            attribute: name, op: "add".into(),
            value: amount as f32, target: Some(target_id as u64),
        });
    }
}

// ── Getter functions (language-specific, need access to context data) ────

#[cfg(feature = "lua")]
fn register_gauge_getters_lua(lua: &mlua::Lua) {
    use mlua::prelude::*;
    let g = lua.globals();

    let _ = g.set("gauge_get", lua.create_function(|lua, name: String| {
        let gauges: LuaTable = lua.globals().get("_gauge_attributes")?;
        Ok(gauges.get::<f64>(name).unwrap_or(0.0))
    }).unwrap());

    let _ = g.set("gauge_get_target", lua.create_function(|lua, (entity_id, name): (u64, String)| {
        let all: LuaTable = lua.globals().get("_all_gauge_attributes")?;
        let val: f64 = match all.get::<LuaValue>(entity_id.to_string()) {
            Ok(LuaValue::Table(t)) => t.get(name).unwrap_or(0.0),
            _ => 0.0,
        };
        Ok(val)
    }).unwrap());

    // Expression variants (varargs — can't use dual_register)
    let _ = g.set("gauge_damage_expr", lua.create_function(|_, args: LuaMultiValue| {
        let attr = lua_arg_string(&args, 0);
        let expr = lua_arg_string(&args, 1);
        let roles = lua_arg_roles(&args, 2);
        push_ext_command(GaugeCommand::InstantExpr {
            attribute: attr, op: "subtract".into(), expression: expr, roles, target: None,
        });
        Ok(())
    }).unwrap());

    let _ = g.set("gauge_heal_expr", lua.create_function(|_, args: LuaMultiValue| {
        let attr = lua_arg_string(&args, 0);
        let expr = lua_arg_string(&args, 1);
        let roles = lua_arg_roles(&args, 2);
        push_ext_command(GaugeCommand::InstantExpr {
            attribute: attr, op: "add".into(), expression: expr, roles, target: None,
        });
        Ok(())
    }).unwrap());

    let _ = g.set("gauge_instant_expr", lua.create_function(|_, (attr, op, expr, roles_table): (String, String, String, Option<LuaTable>)| {
        let roles = parse_roles_table(roles_table.as_ref());
        push_ext_command(GaugeCommand::InstantExpr { attribute: attr, op, expression: expr, roles, target: None });
        Ok(())
    }).unwrap());

    let _ = g.set("gauge_damage_target_expr", lua.create_function(|_, args: LuaMultiValue| {
        let target_id = match args.get(0) {
            Some(LuaValue::Number(n)) => *n as u64,
            Some(LuaValue::Integer(n)) => *n as u64,
            _ => return Ok(()),
        };
        let attr = lua_arg_string(&args, 1);
        let expr = lua_arg_string(&args, 2);
        let roles = lua_arg_roles(&args, 3);
        push_ext_command(GaugeCommand::InstantExpr {
            attribute: attr, op: "subtract".into(), expression: expr, roles, target: Some(target_id),
        });
        Ok(())
    }).unwrap());

    let _ = g.set("gauge_heal_target_expr", lua.create_function(|_, args: LuaMultiValue| {
        let target_id = match args.get(0) {
            Some(LuaValue::Number(n)) => *n as u64,
            Some(LuaValue::Integer(n)) => *n as u64,
            _ => return Ok(()),
        };
        let attr = lua_arg_string(&args, 1);
        let expr = lua_arg_string(&args, 2);
        let roles = lua_arg_roles(&args, 3);
        push_ext_command(GaugeCommand::InstantExpr {
            attribute: attr, op: "add".into(), expression: expr, roles, target: Some(target_id),
        });
        Ok(())
    }).unwrap());
}

#[cfg(feature = "rhai")]
fn register_gauge_getters_rhai(engine: &mut rhai::Engine) {
    use rhai::{ImmutableString, Map};

    engine.register_fn("gauge_get", |gauge_map: Map, name: ImmutableString| -> f64 {
        gauge_map.get(name.as_str()).and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0)
    });

    engine.register_fn("gauge_get_target", |all_gauges: Map, entity_id: i64, name: ImmutableString| -> f64 {
        let key = entity_id.to_string();
        match all_gauges.get(key.as_str()) {
            Some(v) => {
                if let Some(entity_map) = v.clone().try_cast::<Map>() {
                    entity_map.get(name.as_str()).and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0)
                } else { 0.0 }
            }
            None => 0.0,
        }
    });

    engine.register_fn("gauge_damage_expr", |name: ImmutableString, expr: ImmutableString, roles_map: Map| {
        let roles = parse_roles_map(&roles_map);
        push_ext_command(GaugeCommand::InstantExpr {
            attribute: name.to_string(), op: "subtract".into(), expression: expr.to_string(), roles, target: None,
        });
    });

    engine.register_fn("gauge_heal_expr", |name: ImmutableString, expr: ImmutableString, roles_map: Map| {
        let roles = parse_roles_map(&roles_map);
        push_ext_command(GaugeCommand::InstantExpr {
            attribute: name.to_string(), op: "add".into(), expression: expr.to_string(), roles, target: None,
        });
    });

    engine.register_fn("gauge_instant_expr", |attr: ImmutableString, op: ImmutableString, expr: ImmutableString, roles_map: Map| {
        let roles = parse_roles_map(&roles_map);
        push_ext_command(GaugeCommand::InstantExpr {
            attribute: attr.to_string(), op: op.to_string(), expression: expr.to_string(), roles, target: None,
        });
    });

    engine.register_fn("gauge_damage_target_expr", |entity_id: i64, name: ImmutableString, expr: ImmutableString, roles_map: Map| {
        let roles = parse_roles_map(&roles_map);
        push_ext_command(GaugeCommand::InstantExpr {
            attribute: name.to_string(), op: "subtract".into(), expression: expr.to_string(), roles, target: Some(entity_id as u64),
        });
    });

    engine.register_fn("gauge_heal_target_expr", |entity_id: i64, name: ImmutableString, expr: ImmutableString, roles_map: Map| {
        let roles = parse_roles_map(&roles_map);
        push_ext_command(GaugeCommand::InstantExpr {
            attribute: name.to_string(), op: "add".into(), expression: expr.to_string(), roles, target: Some(entity_id as u64),
        });
    });
}

// ── Extension implementation ─────────────────────────────────────────────

pub struct GaugeScriptExtension;

impl ScriptExtension for GaugeScriptExtension {
    fn name(&self) -> &str { "Gauges" }

    fn populate_context(&self, world: &World, entity: Entity, data: &mut ExtensionData) {
        let mut ctx_data = GaugeContextData::default();
        if let Some(snapshot) = world.get_resource::<crate::GaugesSnapshot>() {
            for entry in &snapshot.entries {
                let map: HashMap<String, f32> = entry.attributes.iter().cloned().collect();
                if entry.entity == entity {
                    ctx_data.attributes = map.clone();
                }
                ctx_data.all_attributes.insert(entry.entity.to_bits(), map);
            }
        }
        data.insert(ctx_data);
    }

    #[cfg(feature = "lua")]
    fn register_lua_functions(&self, lua: &mlua::Lua) {
        register_gauge_lua(lua);
        register_gauge_getters_lua(lua);
    }

    #[cfg(feature = "lua")]
    fn setup_lua_context(&self, lua: &mlua::Lua, data: &ExtensionData) {
        let Some(d) = data.get::<GaugeContextData>() else { return };
        renzora_scripting::macros::lua_set_map(lua, "_gauge_attributes", &d.attributes);
        renzora_scripting::macros::lua_set_nested_map(lua, "_all_gauge_attributes", &d.all_attributes);
    }

    #[cfg(feature = "rhai")]
    fn register_rhai_functions(&self, engine: &mut rhai::Engine) {
        register_gauge_rhai(engine);
        register_gauge_getters_rhai(engine);
    }

    #[cfg(feature = "rhai")]
    fn setup_rhai_scope(&self, scope: &mut rhai::Scope, data: &ExtensionData) {
        let Some(d) = data.get::<GaugeContextData>() else { return };
        renzora_scripting::macros::rhai_set_map(scope, "_gauge_attributes", &d.attributes);
        renzora_scripting::macros::rhai_set_nested_map(scope, "_all_gauge_attributes", &d.all_attributes);
    }
}

// ── Lua helpers ──────────────────────────────────────────────────────────

#[cfg(feature = "lua")]
fn lua_arg_string(args: &mlua::MultiValue, idx: usize) -> String {
    match args.get(idx) {
        Some(mlua::Value::String(s)) => s.to_string_lossy().to_string(),
        Some(mlua::Value::Number(n)) => n.to_string(),
        Some(mlua::Value::Integer(n)) => n.to_string(),
        _ => String::new(),
    }
}

#[cfg(feature = "lua")]
fn lua_arg_roles(args: &mlua::MultiValue, idx: usize) -> Vec<(String, u64)> {
    match args.get(idx) {
        Some(mlua::Value::Number(n)) => vec![("attacker".into(), *n as u64)],
        Some(mlua::Value::Integer(n)) => vec![("attacker".into(), *n as u64)],
        Some(mlua::Value::Table(t)) => parse_roles_table(Some(t)),
        _ => vec![],
    }
}

#[cfg(feature = "lua")]
fn parse_roles_table(table: Option<&mlua::Table>) -> Vec<(String, u64)> {
    let Some(t) = table else { return vec![] };
    let mut roles = vec![];
    if let Ok(pairs) = t.pairs::<String, mlua::Value>().collect::<Result<Vec<_>, _>>() {
        for (k, v) in pairs {
            match v {
                mlua::Value::Number(n) => roles.push((k, n as u64)),
                mlua::Value::Integer(n) => roles.push((k, n as u64)),
                _ => {}
            }
        }
    }
    roles
}

#[cfg(feature = "rhai")]
fn parse_roles_map(map: &rhai::Map) -> Vec<(String, u64)> {
    map.iter()
        .filter_map(|(k, v)| v.as_int().ok().map(|id| (k.to_string(), id as u64)))
        .collect()
}
