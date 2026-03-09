//! Gauge script extension — registers gauge functions (gauge_get, gauge_set,
//! gauge_damage, etc.) with the scripting system via the extension API.

use bevy::prelude::*;
use renzora_scripting::extension::{ExtensionData, ScriptExtension, ScriptExtensionCommand};
use renzora_scripting::ScriptCommand;
use std::collections::HashMap;


// ── Extension data types ─────────────────────────────────────────────────

/// Per-entity gauge data injected into scripts.
#[derive(Clone, Default)]
pub struct GaugeContextData {
    /// This entity's attributes.
    pub attributes: HashMap<String, f32>,
    /// All entities' attributes (entity_bits -> { name -> value }).
    pub all_attributes: HashMap<u64, HashMap<String, f32>>,
}

// ── Extension commands ───────────────────────────────────────────────────

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

impl ScriptExtensionCommand for GaugeCommand {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Helper: push a gauge command to the script command buffer.
fn push_gauge(cmd: GaugeCommand) {
    renzora_scripting::backends::push_command(ScriptCommand::Extension(Box::new(cmd)));
}

// ── Extension implementation ─────────────────────────────────────────────

pub struct GaugeScriptExtension;

impl ScriptExtension for GaugeScriptExtension {
    fn name(&self) -> &str {
        "Gauges"
    }

    fn populate_context(
        &self,
        world: &World,
        entity: Entity,
        data: &mut ExtensionData,
    ) {
        let mut ctx_data = GaugeContextData::default();

        // Read from GaugesSnapshot (updated each frame, uses &World)
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
        register_lua_gauge_api(lua);
    }

    #[cfg(feature = "lua")]
    fn setup_lua_context(&self, lua: &mlua::Lua, data: &ExtensionData) {
        setup_lua_gauge_globals(lua, data);
    }

    #[cfg(feature = "rhai")]
    fn register_rhai_functions(&self, engine: &mut rhai::Engine) {
        register_rhai_gauge_api(engine);
    }

    #[cfg(feature = "rhai")]
    fn setup_rhai_scope(&self, scope: &mut rhai::Scope, data: &ExtensionData) {
        setup_rhai_gauge_scope(scope, data);
    }
}

// ── Lua bindings ─────────────────────────────────────────────────────────

#[cfg(feature = "lua")]
fn register_lua_gauge_api(lua: &mlua::Lua) {
    use mlua::prelude::*;

    let globals = lua.globals();

    // gauge_get("health") -> number
    let _ = globals.set("gauge_get", lua.create_function(|lua, name: String| {
        let gauges: LuaTable = lua.globals().get("_gauge_attributes")?;
        let val: f64 = gauges.get(name).unwrap_or(0.0);
        Ok(val)
    }).unwrap());

    // gauge_set("health", 100)
    let _ = globals.set("gauge_set", lua.create_function(|_, (name, value): (String, f64)| {
        push_gauge(GaugeCommand::Set { attribute: name, value: value as f32, target: None });
        Ok(())
    }).unwrap());

    // gauge_add_modifier("strength", 5.0)
    let _ = globals.set("gauge_add_modifier", lua.create_function(|_, (name, value): (String, f64)| {
        push_gauge(GaugeCommand::AddModifier { attribute: name, value: value as f32, target: None });
        Ok(())
    }).unwrap());

    // gauge_remove_modifier("strength", 5.0)
    let _ = globals.set("gauge_remove_modifier", lua.create_function(|_, (name, value): (String, f64)| {
        push_gauge(GaugeCommand::RemoveModifier { attribute: name, value: value as f32, target: None });
        Ok(())
    }).unwrap());

    // gauge_add_expr_modifier("health", "strength * 2")
    let _ = globals.set("gauge_add_expr_modifier", lua.create_function(|_, (name, expr): (String, String)| {
        push_gauge(GaugeCommand::AddExprModifier { attribute: name, expression: expr, target: None });
        Ok(())
    }).unwrap());

    // gauge_instant("health", "add", 10.0)
    let _ = globals.set("gauge_instant", lua.create_function(|_, (name, op, value): (String, String, f64)| {
        push_gauge(GaugeCommand::Instant { attribute: name, op, value: value as f32, target: None });
        Ok(())
    }).unwrap());

    // gauge_damage("health", 10) -- flat
    // gauge_damage("health", "damage@attacker * (1 - resistance)", attacker_id) -- expression
    let _ = globals.set("gauge_damage", lua.create_function(|_, args: LuaMultiValue| {
        let attr = lua_arg_string(&args, 0);
        match args.get(1) {
            Some(LuaValue::Number(n)) => {
                push_gauge(GaugeCommand::Instant { attribute: attr, op: "subtract".into(), value: *n as f32, target: None });
            }
            Some(LuaValue::Integer(n)) => {
                push_gauge(GaugeCommand::Instant { attribute: attr, op: "subtract".into(), value: *n as f32, target: None });
            }
            Some(LuaValue::String(expr)) => {
                let roles = lua_arg_roles(&args, 2);
                push_gauge(GaugeCommand::InstantExpr {
                    attribute: attr, op: "subtract".into(),
                    expression: expr.to_string_lossy().to_string(), roles, target: None,
                });
            }
            _ => {}
        }
        Ok(())
    }).unwrap());

    // gauge_heal("health", 25) -- flat
    // gauge_heal("health", "regen@healer", healer_id) -- expression
    let _ = globals.set("gauge_heal", lua.create_function(|_, args: LuaMultiValue| {
        let attr = lua_arg_string(&args, 0);
        match args.get(1) {
            Some(LuaValue::Number(n)) => {
                push_gauge(GaugeCommand::Instant { attribute: attr, op: "add".into(), value: *n as f32, target: None });
            }
            Some(LuaValue::Integer(n)) => {
                push_gauge(GaugeCommand::Instant { attribute: attr, op: "add".into(), value: *n as f32, target: None });
            }
            Some(LuaValue::String(expr)) => {
                let roles = lua_arg_roles(&args, 2);
                push_gauge(GaugeCommand::InstantExpr {
                    attribute: attr, op: "add".into(),
                    expression: expr.to_string_lossy().to_string(), roles, target: None,
                });
            }
            _ => {}
        }
        Ok(())
    }).unwrap());

    // gauge_instant_expr("attr", "op", "expr", { role = entity_id })
    let _ = globals.set("gauge_instant_expr", lua.create_function(|_, (attr, op, expr, roles_table): (String, String, String, Option<LuaTable>)| {
        let roles = parse_roles_table(roles_table.as_ref());
        push_gauge(GaugeCommand::InstantExpr { attribute: attr, op, expression: expr, roles, target: None });
        Ok(())
    }).unwrap());

    // === Targeted variants ===

    // gauge_get_target(entity_id, "health")
    let _ = globals.set("gauge_get_target", lua.create_function(|lua, (entity_id, name): (u64, String)| {
        let gauges: LuaTable = lua.globals().get("_all_gauge_attributes")?;
        let entity_key = entity_id.to_string();
        let val: f64 = match gauges.get::<LuaValue>(entity_key) {
            Ok(LuaValue::Table(t)) => t.get(name).unwrap_or(0.0),
            _ => 0.0,
        };
        Ok(val)
    }).unwrap());

    // gauge_set_target(entity_id, "health", 100)
    let _ = globals.set("gauge_set_target", lua.create_function(|_, (entity_id, name, value): (u64, String, f64)| {
        push_gauge(GaugeCommand::Set { attribute: name, value: value as f32, target: Some(entity_id) });
        Ok(())
    }).unwrap());

    // gauge_damage_target(entity_id, "health", 10)
    let _ = globals.set("gauge_damage_target", lua.create_function(|_, (target_id, attr, amount): (u64, String, f64)| {
        push_gauge(GaugeCommand::Instant { attribute: attr, op: "subtract".into(), value: amount as f32, target: Some(target_id) });
        Ok(())
    }).unwrap());

    // gauge_damage_target_expr(entity_id, "health", "damage@attacker * (1 - resistance)", attacker_id)
    let _ = globals.set("gauge_damage_target_expr", lua.create_function(|_, args: LuaMultiValue| {
        let target_id = match args.get(0) {
            Some(LuaValue::Number(n)) => *n as u64,
            Some(LuaValue::Integer(n)) => *n as u64,
            _ => return Ok(()),
        };
        let attr = lua_arg_string(&args, 1);
        let expr = lua_arg_string(&args, 2);
        let roles = lua_arg_roles(&args, 3);
        push_gauge(GaugeCommand::InstantExpr {
            attribute: attr, op: "subtract".into(),
            expression: expr, roles, target: Some(target_id),
        });
        Ok(())
    }).unwrap());

    // gauge_heal_target(entity_id, "health", 25)
    let _ = globals.set("gauge_heal_target", lua.create_function(|_, (target_id, attr, amount): (u64, String, f64)| {
        push_gauge(GaugeCommand::Instant { attribute: attr, op: "add".into(), value: amount as f32, target: Some(target_id) });
        Ok(())
    }).unwrap());

    // gauge_heal_target_expr(entity_id, "health", "regen@healer", healer_id)
    let _ = globals.set("gauge_heal_target_expr", lua.create_function(|_, args: LuaMultiValue| {
        let target_id = match args.get(0) {
            Some(LuaValue::Number(n)) => *n as u64,
            Some(LuaValue::Integer(n)) => *n as u64,
            _ => return Ok(()),
        };
        let attr = lua_arg_string(&args, 1);
        let expr = lua_arg_string(&args, 2);
        let roles = lua_arg_roles(&args, 3);
        push_gauge(GaugeCommand::InstantExpr {
            attribute: attr, op: "add".into(),
            expression: expr, roles, target: Some(target_id),
        });
        Ok(())
    }).unwrap());
}

#[cfg(feature = "lua")]
fn setup_lua_gauge_globals(lua: &mlua::Lua, data: &ExtensionData) {
    let Some(gauge_data) = data.get::<GaugeContextData>() else { return };
    let g = lua.globals();

    // Self attributes
    if let Ok(gauge_table) = lua.create_table() {
        for (name, value) in &gauge_data.attributes {
            let _ = gauge_table.set(name.clone(), *value as f64);
        }
        let _ = g.set("_gauge_attributes", gauge_table);
    }

    // All entities' attributes
    if let Ok(all_table) = lua.create_table() {
        for (entity_bits, attrs) in &gauge_data.all_attributes {
            if let Ok(entity_table) = lua.create_table() {
                for (name, value) in attrs {
                    let _ = entity_table.set(name.clone(), *value as f64);
                }
                let _ = all_table.set(entity_bits.to_string(), entity_table);
            }
        }
        let _ = g.set("_all_gauge_attributes", all_table);
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

// ── Rhai bindings ────────────────────────────────────────────────────────

#[cfg(feature = "rhai")]
fn register_rhai_gauge_api(engine: &mut rhai::Engine) {
    use rhai::{ImmutableString, Map};

    engine.register_fn("gauge_get", |gauge_map: Map, name: ImmutableString| -> f64 {
        gauge_map.get(name.as_str()).and_then(|v| v.clone().try_cast::<f64>()).unwrap_or(0.0)
    });
    engine.register_fn("gauge_set", |name: ImmutableString, value: f64| {
        push_gauge(GaugeCommand::Set { attribute: name.to_string(), value: value as f32, target: None });
    });
    engine.register_fn("gauge_add_modifier", |name: ImmutableString, value: f64| {
        push_gauge(GaugeCommand::AddModifier { attribute: name.to_string(), value: value as f32, target: None });
    });
    engine.register_fn("gauge_remove_modifier", |name: ImmutableString, value: f64| {
        push_gauge(GaugeCommand::RemoveModifier { attribute: name.to_string(), value: value as f32, target: None });
    });
    engine.register_fn("gauge_add_expr_modifier", |name: ImmutableString, expr: ImmutableString| {
        push_gauge(GaugeCommand::AddExprModifier { attribute: name.to_string(), expression: expr.to_string(), target: None });
    });
    engine.register_fn("gauge_instant", |name: ImmutableString, op: ImmutableString, value: f64| {
        push_gauge(GaugeCommand::Instant { attribute: name.to_string(), op: op.to_string(), value: value as f32, target: None });
    });
    engine.register_fn("gauge_damage", |name: ImmutableString, amount: f64| {
        push_gauge(GaugeCommand::Instant { attribute: name.to_string(), op: "subtract".into(), value: amount as f32, target: None });
    });
    engine.register_fn("gauge_damage_expr", |name: ImmutableString, expr: ImmutableString, roles_map: Map| {
        let roles = parse_roles_map(&roles_map);
        push_gauge(GaugeCommand::InstantExpr { attribute: name.to_string(), op: "subtract".into(), expression: expr.to_string(), roles, target: None });
    });
    engine.register_fn("gauge_heal", |name: ImmutableString, amount: f64| {
        push_gauge(GaugeCommand::Instant { attribute: name.to_string(), op: "add".into(), value: amount as f32, target: None });
    });
    engine.register_fn("gauge_heal_expr", |name: ImmutableString, expr: ImmutableString, roles_map: Map| {
        let roles = parse_roles_map(&roles_map);
        push_gauge(GaugeCommand::InstantExpr { attribute: name.to_string(), op: "add".into(), expression: expr.to_string(), roles, target: None });
    });
    engine.register_fn("gauge_instant_expr", |attr: ImmutableString, op: ImmutableString, expr: ImmutableString, roles_map: Map| {
        let roles = parse_roles_map(&roles_map);
        push_gauge(GaugeCommand::InstantExpr { attribute: attr.to_string(), op: op.to_string(), expression: expr.to_string(), roles, target: None });
    });

    // === Targeted variants ===
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
    engine.register_fn("gauge_set_target", |entity_id: i64, name: ImmutableString, value: f64| {
        push_gauge(GaugeCommand::Set { attribute: name.to_string(), value: value as f32, target: Some(entity_id as u64) });
    });
    engine.register_fn("gauge_damage_target", |entity_id: i64, name: ImmutableString, amount: f64| {
        push_gauge(GaugeCommand::Instant { attribute: name.to_string(), op: "subtract".into(), value: amount as f32, target: Some(entity_id as u64) });
    });
    engine.register_fn("gauge_damage_target_expr", |entity_id: i64, name: ImmutableString, expr: ImmutableString, roles_map: Map| {
        let roles = parse_roles_map(&roles_map);
        push_gauge(GaugeCommand::InstantExpr { attribute: name.to_string(), op: "subtract".into(), expression: expr.to_string(), roles, target: Some(entity_id as u64) });
    });
    engine.register_fn("gauge_heal_target", |entity_id: i64, name: ImmutableString, amount: f64| {
        push_gauge(GaugeCommand::Instant { attribute: name.to_string(), op: "add".into(), value: amount as f32, target: Some(entity_id as u64) });
    });
    engine.register_fn("gauge_heal_target_expr", |entity_id: i64, name: ImmutableString, expr: ImmutableString, roles_map: Map| {
        let roles = parse_roles_map(&roles_map);
        push_gauge(GaugeCommand::InstantExpr { attribute: name.to_string(), op: "add".into(), expression: expr.to_string(), roles, target: Some(entity_id as u64) });
    });
}

#[cfg(feature = "rhai")]
fn setup_rhai_gauge_scope(scope: &mut rhai::Scope, data: &ExtensionData) {
    use rhai::{Dynamic, Map};

    let Some(gauge_data) = data.get::<GaugeContextData>() else { return };

    // Self attributes
    let mut gauge_map = Map::new();
    for (name, value) in &gauge_data.attributes {
        gauge_map.insert(name.clone().into(), Dynamic::from(*value as f64));
    }
    scope.push("_gauge_attributes", gauge_map);

    // All entities' attributes
    let mut all_gauge_map = Map::new();
    for (entity_bits, attrs) in &gauge_data.all_attributes {
        let mut entity_map = Map::new();
        for (name, value) in attrs {
            entity_map.insert(name.clone().into(), Dynamic::from(*value as f64));
        }
        all_gauge_map.insert(entity_bits.to_string().into(), Dynamic::from(entity_map));
    }
    scope.push("_all_gauge_attributes", all_gauge_map);
}

#[cfg(feature = "rhai")]
fn parse_roles_map(map: &rhai::Map) -> Vec<(String, u64)> {
    map.iter()
        .filter_map(|(k, v)| {
            v.as_int().ok().map(|id| (k.to_string(), id as u64))
        })
        .collect()
}
