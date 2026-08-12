//! The Lua interpreter.
//!
//! Moved out of the engine wholesale. What changed in the move is narrow, and
//! worth knowing because it is the shape any language backend takes:
//!
//! * **No file I/O.** The host reads the script and hands over `source` plus a
//!   `version`; a VM is rebuilt when the version moves. That is the whole of
//!   hot reload from this side, and it is what lets an exported game run
//!   scripts out of an rpak archive without this crate knowing archives exist.
//! * **No `ScriptContext`.** Globals are set from [`Ctx`], whose frame half the
//!   host encodes once per frame rather than once per entity.
//! * **Reads go back through the host.** `get(...)` and friends call across the
//!   boundary; see [`crate::host`] for why that needs a thread-local.
//!
//! Everything else — the ~1,100-line `register_api`, the draw context, the prop
//! parser — is unchanged, because none of it ever touched Bevy. It built
//! `ScriptCommand`s, and `ScriptCommand` is now defined at the boundary.

#![allow(unused_mut, dead_code, unused_variables)]

use std::collections::HashMap;
use std::sync::Mutex;

use mlua::prelude::*;

use renzora_plugin::script::{
    ActionValue, Backend, Binding, BindingKind, Ctx, GamepadSnapshot, Hook, ParamKind, PropValue,
    ScriptCommand, ScriptReply, ScriptRef, ScriptValue, VarDef, GAMEPAD_BUTTON_NAMES,
};

use crate::buffers::{drain_commands, drain_draws, push_command};
use crate::host;

/// Persistent Lua VM for one (entity, script) pair.
///
/// Creating a `Lua` state and registering the API costs ~hundreds of
/// `create_function` calls; doing that per script per entity per frame is what
/// makes scripted scenes drop frames at hundreds of entities. This pays it once
/// per entity-script lifetime instead.
struct LuaInstance {
    lua: Lua,
    /// The host's version of the source this VM compiled. On mismatch the VM is
    /// dropped and rebuilt, which is how a hot reload reaches Lua.
    source_version: u64,
    /// Which binding generation the VM was built with. A plugin registering an
    /// extension after a VM exists would otherwise leave that VM without the
    /// new functions until its source happened to change.
    bindings_generation: u64,
}

/// The Lua backend.
#[derive(Default)]
pub struct LuaBackend {
    /// Keyed by `(entity, path)` — two entities running the same script get
    /// separate VMs, which is what makes a script's globals per-entity state.
    ///
    /// `mlua::Lua` is `Send` but not `Sync`, so the `Mutex` is what lets this
    /// live behind the generated `static`. It is never contended in practice:
    /// the engine runs scripts from one exclusive system.
    instances: Mutex<HashMap<(u64, String), LuaInstance>>,
    bindings: Vec<Binding>,
    /// Bumped whenever `bindings` is replaced, so existing VMs know to rebuild.
    bindings_generation: u64,
}

impl LuaBackend {
    /// Get or build the VM for this script, then run `invoke` against it.
    fn with_vm<F>(
        &self,
        script: &ScriptRef,
        ctx: &Ctx,
        reply: &mut ScriptReply,
        invoke: F,
    ) -> Result<(), String>
    where
        F: FnOnce(&Lua) -> Result<(), String>,
    {
        let key = (script.entity, script.path.to_string());
        let mut instances = self.instances.lock().map_err(|e| e.to_string())?;

        let stale = match instances.get(&key) {
            None => true,
            Some(i) => {
                i.source_version != script.version
                    || i.bindings_generation != self.bindings_generation
            }
        };
        if stale {
            let lua = Lua::new();
            register_api(&lua);
            register_bindings(&lua, &self.bindings);
            lua.load(script.source)
                .exec()
                .map_err(|e| format!("Lua error: {e}"))?;
            instances.insert(
                key.clone(),
                LuaInstance {
                    lua,
                    source_version: script.version,
                    bindings_generation: self.bindings_generation,
                },
            );
        }

        let instance = instances
            .get(&key)
            .ok_or_else(|| "Lua instance vanished".to_string())?;
        let lua = &instance.lua;

        // Per-frame: overwrite the context globals in place, so the cost scales
        // with context size rather than with API surface.
        set_context_globals(lua, ctx, script.vars);

        // Drain anything left over so this hook only sees its own output.
        drain_commands();
        drain_draws();

        invoke(lua)?;

        reply.vars = read_back_variables(lua, script.vars);
        reply.commands = drain_commands();
        reply.draws = drain_draws();
        Ok(())
    }

    /// Call a hook by name, if the script defines it.
    fn call_hook(
        &self,
        script: &ScriptRef,
        ctx: &Ctx,
        reply: &mut ScriptReply,
        hook: &str,
        args: impl IntoLuaMulti,
    ) -> Result<(), String> {
        self.with_vm(script, ctx, reply, |lua| {
            let globals = lua.globals();
            // A script that does not define the hook is the common case, not an
            // error — most define two of the nine.
            let Ok(func) = globals.get::<LuaFunction>(hook) else {
                return Ok(());
            };
            func.call::<()>(args).map_err(|e| {
                let name = script.path.rsplit(['/', '\\']).next().unwrap_or("script");
                format!("{name} {hook}: {e}")
            })
        })
    }

    /// Parse the props a script declares, by running it in a throwaway VM.
    fn read_props(&self, source: &str) -> Vec<VarDef> {
        let lua = Lua::new();
        register_api(&lua);
        register_bindings(&lua, &self.bindings);
        let mut props = Vec::new();

        if lua.load(source).exec().is_err() {
            return props;
        }

        let globals = lua.globals();
        let Ok(func) = globals.get::<LuaFunction>("props") else {
            return props;
        };
        let Ok(table) = func.call::<LuaTable>(()) else {
            return props;
        };

        for pair in table.pairs::<String, LuaValue>() {
            let Ok((name, value)) = pair else { continue };
            let display_name = to_display_name(&name);

            // A prop may be a bare value, or a table carrying `default`/`value`
            // plus optional `hint` and `tab`.
            if let LuaValue::Table(ref prop_table) = value {
                let default_val = prop_table
                    .get::<LuaValue>("value")
                    .or_else(|_| prop_table.get::<LuaValue>("default"));

                if let Ok(ref default_val) = default_val {
                    if let Some(sv) = lua_to_script_value(default_val) {
                        props.push(VarDef {
                            name,
                            display_name,
                            default_value: sv,
                            hint: prop_table.get("hint").ok(),
                            tab: prop_table.get("tab").ok(),
                        });
                        continue;
                    }
                }
            }

            if let Some(sv) = lua_to_script_value(&value) {
                props.push(VarDef {
                    name,
                    display_name,
                    default_value: sv,
                    hint: None,
                    tab: None,
                });
            }
        }

        props.sort_by(|a, b| a.name.cmp(&b.name));
        props
    }
}

impl Backend for LuaBackend {
    const NAME: &'static str = "Lua";
    // `.blueprint`/`.bp` graphs are compiled to Lua by the host before the
    // source reaches here — `renzora_blueprint` links Bevy and cannot cross the
    // boundary — so this backend claims them but only ever sees Lua text.
    const EXTENSIONS: &'static [&'static str] = &["lua", "blueprint", "bp"];

    fn set_bindings(&mut self, bindings: &[Binding]) {
        self.bindings = bindings.to_vec();
        self.bindings_generation = self.bindings_generation.wrapping_add(1);
    }

    fn props(&mut self, script: &ScriptRef) -> Vec<VarDef> {
        self.read_props(script.source)
    }

    fn hook(
        &mut self,
        script: &ScriptRef,
        hook: Hook,
        ctx: &Ctx,
        reply: &mut ScriptReply,
    ) -> Result<(), String> {
        // Host reads are reachable from inside Lua only through a thread-local,
        // because those functions were registered when the VM was built and the
        // call table is valid only now. Scoped so it cannot outlive the call.
        let _guard = host::enter(ctx.host);
        let name = hook.fn_name();
        match hook {
            Hook::Ready | Hook::Update => self.call_hook(script, ctx, reply, name, ()),
            Hook::Rpc {
                name: rpc,
                from,
                args,
            } => self.with_vm(script, ctx, reply, |lua| {
                let globals = lua.globals();
                let Ok(func) = globals.get::<LuaFunction>("on_rpc") else {
                    return Ok(());
                };
                let table = args_table(lua, args).map_err(|e| e.to_string())?;
                func.call::<()>((rpc, table, from))
                    .map_err(|e| format!("on_rpc: {e}"))
            }),
            Hook::Ui {
                name: ui,
                entity_bits,
                args,
            } => self.with_vm(script, ctx, reply, |lua| {
                let globals = lua.globals();
                let Ok(func) = globals.get::<LuaFunction>("on_ui") else {
                    return Ok(());
                };
                let table = args_table(lua, args).map_err(|e| e.to_string())?;
                func.call::<()>((ui, table, entity_bits))
                    .map_err(|e| format!("on_ui: {e}"))
            }),
            Hook::Draw { width, height } => self.with_vm(script, ctx, reply, |lua| {
                let globals = lua.globals();
                let Ok(func) = globals.get::<LuaFunction>("on_draw") else {
                    return Ok(());
                };
                let g = build_draw_context(lua, width, height).map_err(|e| e.to_string())?;
                func.call::<()>(g).map_err(|e| format!("on_draw: {e}"))
            }),
            Hook::AnimationEvent {
                name: ev,
                entity_bits,
            } => self.call_hook(script, ctx, reply, name, (ev, entity_bits)),
            Hook::Http {
                callback,
                status,
                body,
            } => self.call_hook(script, ctx, reply, name, (callback, status, body)),
            Hook::PlayerEvent { id, .. } => self.call_hook(script, ctx, reply, name, id),
            // `fn_name` already picked on_scene_loaded vs on_scene_load_failed,
            // so the failure case just carries the extra reason argument.
            Hook::SceneEvent { path, error } => match error {
                None => self.call_hook(script, ctx, reply, name, path),
                Some(err) => self.call_hook(script, ctx, reply, name, (path, err)),
            },
            Hook::Event { name: ev, args } => self.with_vm(script, ctx, reply, |lua| {
                let globals = lua.globals();
                let Ok(func) = globals.get::<LuaFunction>("on_event") else {
                    return Ok(());
                };
                let table = args_table(lua, args).map_err(|e| e.to_string())?;
                func.call::<()>((ev, table))
                    .map_err(|e| format!("on_event: {e}"))
            }),
        }
    }

    fn eval(&mut self, expr: &str) -> Result<String, String> {
        let lua = Lua::new();
        register_api(&lua);
        register_bindings(&lua, &self.bindings);
        match lua.load(expr).eval::<LuaValue>() {
            Ok(v) => Ok(lua_value_to_string(&v)),
            Err(e) => Err(format!("{e}")),
        }
    }

    /// Drop cached VMs. An empty `path` means any script, and a zero `entity`
    /// means any entity, so a despawn sends `("", bits)` and a detached script
    /// sends `(path, bits)`.
    fn evict(&mut self, path: &str, entity: u64) {
        if let Ok(mut instances) = self.instances.lock() {
            instances.retain(|(eid, p), _| {
                let path_matches = path.is_empty() || p == path;
                let entity_matches = entity == 0 || *eid == entity;
                !(path_matches && entity_matches)
            });
        }
    }
}

/// Build the Lua table an `on_rpc`/`on_ui` hook receives.
fn args_table(lua: &Lua, args: &[(String, ActionValue)]) -> LuaResult<LuaTable> {
    let table = lua.create_table()?;
    for (k, v) in args {
        table.set(k.as_str(), action_value_to_lua(lua, v)?)?;
    }
    Ok(table)
}


/// Parse a `#RRGGBB` / `#RRGGBBAA` hex string to sRGB `[r,g,b,a]` in 0..1. Anything
/// unparseable falls back to opaque white so a typo is visible, not invisible.
fn parse_hex(s: &str) -> [f32; 4] {
    let s = s.trim().trim_start_matches('#');
    let ch = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok().map(|v| v as f32 / 255.0);
    match s.len() {
        6 => [
            ch(0).unwrap_or(1.0),
            ch(2).unwrap_or(1.0),
            ch(4).unwrap_or(1.0),
            1.0,
        ],
        8 => [
            ch(0).unwrap_or(1.0),
            ch(2).unwrap_or(1.0),
            ch(4).unwrap_or(1.0),
            ch(6).unwrap_or(1.0),
        ],
        _ => [1.0, 1.0, 1.0, 1.0],
    }
}

/// Build the `g` canvas context passed to `on_draw(g)`: `g.width`/`g.height` plus
/// the complete-shape methods, each of which records a [`renzora::DrawCmd`]. Called
/// with dot syntax (`g.arc(...)`, not `g:arc(...)`) — the functions take no `self`.
/// Colours are `#hex` strings; the trailing thickness arg is optional.
fn build_draw_context(lua: &Lua, width: f32, height: f32) -> mlua::Result<mlua::Table> {
    use renzora_plugin::script::DrawCmd;
    let g = lua.create_table()?;
    g.set("width", width)?;
    g.set("height", height)?;
    g.set(
        "line",
        lua.create_function(
            |_, (x1, y1, x2, y2, color, thickness): (f32, f32, f32, f32, String, Option<f32>)| {
                crate::buffers::push_draw(DrawCmd::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    color: parse_hex(&color),
                    thickness: thickness.unwrap_or(2.0),
                });
                Ok(())
            },
        )?,
    )?;
    g.set(
        "arc",
        lua.create_function(
            |_,
             (cx, cy, r, start, end, color, thickness): (
                f32,
                f32,
                f32,
                f32,
                f32,
                String,
                Option<f32>,
            )| {
                crate::buffers::push_draw(DrawCmd::Arc {
                    cx,
                    cy,
                    r,
                    start,
                    end,
                    color: parse_hex(&color),
                    thickness: thickness.unwrap_or(2.0),
                });
                Ok(())
            },
        )?,
    )?;
    g.set(
        "circle",
        lua.create_function(|_, (cx, cy, r, color): (f32, f32, f32, String)| {
            crate::buffers::push_draw(DrawCmd::Circle {
                cx,
                cy,
                r,
                color: parse_hex(&color),
            });
            Ok(())
        })?,
    )?;
    g.set(
        "rect",
        lua.create_function(|_, (x, y, w, h, color): (f32, f32, f32, f32, String)| {
            crate::buffers::push_draw(DrawCmd::Rect {
                x,
                y,
                w,
                h,
                color: parse_hex(&color),
            });
            Ok(())
        })?,
    )?;
    g.set(
        "text",
        lua.create_function(|_, (x, y, text, size, color): (f32, f32, String, f32, String)| {
            crate::buffers::push_draw(DrawCmd::Text {
                x,
                y,
                text,
                size,
                color: parse_hex(&color),
            });
            Ok(())
        })?,
    )?;
    g.set(
        "triangle",
        lua.create_function(
            |_, (x1, y1, x2, y2, x3, y3, color): (f32, f32, f32, f32, f32, f32, String)| {
                crate::buffers::push_draw(DrawCmd::Triangle {
                    x1,
                    y1,
                    x2,
                    y2,
                    x3,
                    y3,
                    color: parse_hex(&color),
                });
                Ok(())
            },
        )?,
    )?;
    // `g.poly(points, color)` — points is a flat table `{x1,y1, x2,y2, ...}`. A
    // convex polygon fans from vertex 0 into (n-2) filled triangles.
    g.set(
        "poly",
        lua.create_function(|_, (points, color): (mlua::Table, String)| {
            let col = parse_hex(&color);
            let n_coords = points.raw_len();
            let mut xs: Vec<f32> = Vec::with_capacity(n_coords);
            for i in 1..=n_coords {
                xs.push(points.get::<f32>(i).unwrap_or(0.0));
            }
            let n = xs.len() / 2;
            if n >= 3 {
                let pt = |i: usize| (xs[i * 2], xs[i * 2 + 1]);
                let (x0, y0) = pt(0);
                for i in 1..(n - 1) {
                    let (xa, ya) = pt(i);
                    let (xb, yb) = pt(i + 1);
                    crate::buffers::push_draw(DrawCmd::Triangle {
                        x1: x0,
                        y1: y0,
                        x2: xa,
                        y2: ya,
                        x3: xb,
                        y3: yb,
                        color: col,
                    });
                }
            }
            Ok(())
        })?,
    )?;
    Ok(g)
}

// =============================================================================
// Lua API registration
// =============================================================================

/// Build a Lua function for every function a domain crate declared.
///
/// This is the whole of what `renzora_physics` and friends used to write by
/// hand, once, for every language — see [`crate::extension`]. Done at VM
/// creation rather than per frame, because the set only changes when a plugin
/// registers an extension and the VM is rebuilt on a source change anyway.
fn register_bindings(lua: &Lua, bindings: &[Binding]) {
    let globals = lua.globals();
    for b in bindings {
        let f = match &b.kind {
            BindingKind::Action { action } => action_fn(lua, b, action),
            BindingKind::Read { component, field } => read_fn(lua, b, component, field),
            BindingKind::Translate => translate_fn(lua),
        };
        match f {
            Ok(f) => {
                let _ = globals.set(b.name.as_str(), f);
            }
            // A binding that will not build is one missing script function, not
            // a reason to abandon the rest of them.
            Err(e) => renzora_plugin::warn!(
                "[Scripting] could not build binding `{}`: {}",
                b.name,
                e
            ),
        }
    }
}

/// Read one script argument as the parameter kind says to.
///
/// A missing or wrong-typed argument becomes the type's zero rather than an
/// error, matching what the hand-written bindings did: `mlua`'s `f32` coercion
/// turned `apply_force(1, 2)` into a third argument of `0.0`, and scripts
/// depend on that.
fn arg_value(args: &LuaMultiValue, i: usize, kind: ParamKind) -> ActionValue {
    let v = args.get(i);
    match kind {
        ParamKind::Float => ActionValue::Float(arg_f32(v)),
        ParamKind::Int => ActionValue::Int(match v {
            Some(LuaValue::Integer(n)) => *n,
            Some(LuaValue::Number(n)) => *n as i64,
            _ => 0,
        }),
        ParamKind::Bool => ActionValue::Bool(matches!(v, Some(LuaValue::Boolean(true)))),
        ParamKind::Str => ActionValue::String(arg_string(v)),
        // Consumes three script arguments; see `ParamKind::arity`.
        ParamKind::Vec3 => ActionValue::Vec3([
            arg_f32(args.get(i)),
            arg_f32(args.get(i + 1)),
            arg_f32(args.get(i + 2)),
        ]),
    }
}

fn arg_f32(v: Option<&LuaValue>) -> f32 {
    match v {
        Some(LuaValue::Number(n)) => *n as f32,
        Some(LuaValue::Integer(n)) => *n as f32,
        _ => 0.0,
    }
}

fn arg_string(v: Option<&LuaValue>) -> String {
    match v {
        Some(LuaValue::String(s)) => s.to_str().map(|s| s.to_string()).unwrap_or_default(),
        Some(other) => lua_value_to_string(other),
        None => String::new(),
    }
}

/// Pack the declared parameters and push a `ScriptCommand::Action`.
fn action_fn(lua: &Lua, b: &Binding, action: &str) -> LuaResult<LuaFunction> {
    let params = b.params.clone();
    let action = action.to_string();
    lua.create_function(move |_, args: LuaMultiValue| {
        let mut out = Vec::with_capacity(params.len());
        let mut i = 0;
        for p in &params {
            out.push((p.name.clone(), arg_value(&args, i, p.kind)));
            i += p.kind.arity();
        }
        push_command(ScriptCommand::Action {
            name: action.clone(),
            target_entity: None,
            args: out,
        });
        Ok(())
    })
}

/// Read a reflected field, substituting the call's arguments into the path.
fn read_fn(lua: &Lua, b: &Binding, component: &str, field: &str) -> LuaResult<LuaFunction> {
    let params = b.params.clone();
    let component = component.to_string();
    let field = field.to_string();
    lua.create_function(move |lua, args: LuaMultiValue| {
        // Placeholders are positional over the *script* arguments, not the
        // packed ones, so a `Vec3` parameter does not shift the numbering.
        let subs: Vec<String> = (0..params.len().max(args.len()))
            .map(|i| arg_string(args.get(i)))
            .collect();
        let component = renzora_plugin::script::substitute(&component, &subs);
        let field = renzora_plugin::script::substitute(&field, &subs);
        match host::call_get(None, &component, &field) {
            Some(v) => property_value_to_lua_result(lua, v),
            None => Ok(LuaValue::Nil),
        }
    })
}

/// Look a key up in the localization table.
fn translate_fn(lua: &Lua) -> LuaResult<LuaFunction> {
    lua.create_function(|_, key: String| Ok(host::translate(&key)))
}

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
    let _ = globals.set(
        "goto_camera_preset",
        lua.create_function(|_, name: String| {
            push_command(ScriptCommand::GotoCameraPreset { name });
            Ok(())
        })
        .unwrap(),
    );

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
    let _ = globals.set(
        "set_child_position",
        lua.create_function(|_, (name, x, y, z): (String, f32, f32, f32)| {
            push_command(ScriptCommand::ChildSetPosition { name, x, y, z });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "set_child_rotation",
        lua.create_function(|_, (name, x, y, z): (String, f32, f32, f32)| {
            push_command(ScriptCommand::ChildSetRotation { name, x, y, z });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "child_translate",
        lua.create_function(|_, (name, x, y, z): (String, f32, f32, f32)| {
            push_command(ScriptCommand::ChildTranslate { name, x, y, z });
            Ok(())
        })
        .unwrap(),
    );

    // -- Input --
    let _ = globals.set(
        "is_key_pressed",
        lua.create_function(|lua, key: String| {
            let keys: LuaTable = lua.globals().get("_keys_pressed")?;
            let pressed: bool = keys.get(key).unwrap_or(false);
            Ok(pressed)
        })
        .unwrap(),
    );
    let _ = globals.set(
        "is_key_just_pressed",
        lua.create_function(|lua, key: String| {
            let keys: LuaTable = lua.globals().get("_keys_just_pressed")?;
            let pressed: bool = keys.get(key).unwrap_or(false);
            Ok(pressed)
        })
        .unwrap(),
    );
    let _ = globals.set(
        "is_key_just_released",
        lua.create_function(|lua, key: String| {
            let keys: LuaTable = lua.globals().get("_keys_just_released")?;
            let pressed: bool = keys.get(key).unwrap_or(false);
            Ok(pressed)
        })
        .unwrap(),
    );

    // Action-based input — reads the InputMap's ActionState by name so scripts
    // work identically with keyboard and gamepad.
    let _ = globals.set(
        "input_button_pressed",
        lua.create_function(|lua, name: String| {
            let t: LuaTable = lua.globals().get("_action_pressed")?;
            Ok(t.get::<bool>(name).unwrap_or(false))
        })
        .unwrap(),
    );
    let _ = globals.set(
        "input_button_just_pressed",
        lua.create_function(|lua, name: String| {
            let t: LuaTable = lua.globals().get("_action_just_pressed")?;
            Ok(t.get::<bool>(name).unwrap_or(false))
        })
        .unwrap(),
    );
    let _ = globals.set(
        "input_button_just_released",
        lua.create_function(|lua, name: String| {
            let t: LuaTable = lua.globals().get("_action_just_released")?;
            Ok(t.get::<bool>(name).unwrap_or(false))
        })
        .unwrap(),
    );
    let _ = globals.set(
        "input_axis_1d",
        lua.create_function(|lua, name: String| {
            let t: LuaTable = lua.globals().get("_action_axis_1d")?;
            Ok(t.get::<f64>(name).unwrap_or(0.0))
        })
        .unwrap(),
    );
    // Returns two values (x, y). Use: `local mx, my = input_axis_2d("move")`.
    let _ = globals.set(
        "input_axis_2d",
        lua.create_function(|lua, name: String| {
            let t: LuaTable = lua.globals().get("_action_axis_2d")?;
            if let Ok(pair) = t.get::<LuaTable>(name) {
                let x: f64 = pair.get(1).unwrap_or(0.0);
                let y: f64 = pair.get(2).unwrap_or(0.0);
                Ok((x, y))
            } else {
                Ok((0.0, 0.0))
            }
        })
        .unwrap(),
    );

    // Multi-gamepad — reads the per-execution `_gamepads` table, keyed by
    // stable pad slot id (0 = first pad). The legacy `gamepad_*` globals keep
    // mirroring the first connected pad.
    let _ = globals.set(
        "gamepad_count",
        lua.create_function(|lua, ()| {
            let n: i64 = lua.globals().get("_gamepad_count").unwrap_or(0);
            Ok(n)
        })
        .unwrap(),
    );
    let _ = globals.set(
        "gamepad_connected",
        lua.create_function(|lua, pad: i64| {
            let t: LuaTable = lua.globals().get("_gamepads")?;
            Ok(t.get::<LuaTable>(pad).is_ok())
        })
        .unwrap(),
    );
    // Axis names: "left_x", "left_y", "right_x", "right_y",
    //             "left_trigger", "right_trigger".
    let _ = globals.set(
        "gamepad_axis",
        lua.create_function(|lua, (pad, name): (i64, String)| {
            let t: LuaTable = lua.globals().get("_gamepads")?;
            let Ok(pad_t) = t.get::<LuaTable>(pad) else {
                return Ok(0.0f64);
            };
            Ok(pad_t.get::<f64>(name).unwrap_or(0.0))
        })
        .unwrap(),
    );
    // Returns two values (x, y): `local x, y = gamepad_left_stick(1)`.
    let _ = globals.set(
        "gamepad_left_stick",
        lua.create_function(|lua, pad: i64| {
            let t: LuaTable = lua.globals().get("_gamepads")?;
            let Ok(pad_t) = t.get::<LuaTable>(pad) else {
                return Ok((0.0f64, 0.0f64));
            };
            Ok((
                pad_t.get::<f64>("left_x").unwrap_or(0.0),
                pad_t.get::<f64>("left_y").unwrap_or(0.0),
            ))
        })
        .unwrap(),
    );
    let _ = globals.set(
        "gamepad_right_stick",
        lua.create_function(|lua, pad: i64| {
            let t: LuaTable = lua.globals().get("_gamepads")?;
            let Ok(pad_t) = t.get::<LuaTable>(pad) else {
                return Ok((0.0f64, 0.0f64));
            };
            Ok((
                pad_t.get::<f64>("right_x").unwrap_or(0.0),
                pad_t.get::<f64>("right_y").unwrap_or(0.0),
            ))
        })
        .unwrap(),
    );
    // Button names: "south", "east", "west", "north", "l1", "r1", "l2", "r2",
    //               "select", "start", "l3", "r3", "dpad_up", "dpad_down",
    //               "dpad_left", "dpad_right".
    let _ = globals.set(
        "gamepad_button",
        lua.create_function(|lua, (pad, name): (i64, String)| {
            let t: LuaTable = lua.globals().get("_gamepads")?;
            let Ok(pad_t) = t.get::<LuaTable>(pad) else {
                return Ok(false);
            };
            let Ok(buttons) = pad_t.get::<LuaTable>("buttons") else {
                return Ok(false);
            };
            Ok(buttons.get::<bool>(name).unwrap_or(false))
        })
        .unwrap(),
    );
    let _ = globals.set(
        "gamepad_button_just_pressed",
        lua.create_function(|lua, (pad, name): (i64, String)| {
            let t: LuaTable = lua.globals().get("_gamepads")?;
            let Ok(pad_t) = t.get::<LuaTable>(pad) else {
                return Ok(false);
            };
            let Ok(buttons) = pad_t.get::<LuaTable>("just_pressed") else {
                return Ok(false);
            };
            Ok(buttons.get::<bool>(name).unwrap_or(false))
        })
        .unwrap(),
    );

    // -- Audio --
    let _ = globals.set(
        "play_sound",
        lua.create_function(|_, args: LuaMultiValue| {
            let path: String = args.front()
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default();
            let volume: f32 = args.get(1).and_then(|v| v.as_f32()).unwrap_or(1.0);
            let bus: String = args
                .get(2)
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "Sfx".into());
            push_command(ScriptCommand::PlaySound {
                path,
                volume,
                looping: false,
                bus,
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "play_sound_looping",
        lua.create_function(|_, (path, volume): (String, f32)| {
            push_command(ScriptCommand::PlaySound {
                path,
                volume,
                looping: true,
                bus: "Sfx".into(),
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "play_music",
        lua.create_function(|_, args: LuaMultiValue| {
            let path: String = args.front()
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default();
            let volume: f32 = args.get(1).and_then(|v| v.as_f32()).unwrap_or(1.0);
            let fade_in: f32 = args.get(2).and_then(|v| v.as_f32()).unwrap_or(0.0);
            push_command(ScriptCommand::PlayMusic {
                path,
                volume,
                fade_in,
                bus: "Music".into(),
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "stop_music",
        lua.create_function(|_, fade_out: Option<f32>| {
            push_command(ScriptCommand::StopMusic {
                fade_out: fade_out.unwrap_or(0.0),
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "stop_all_sounds",
        lua.create_function(|_, ()| {
            push_command(ScriptCommand::StopAllSounds);
            Ok(())
        })
        .unwrap(),
    );
    // play_audio([entity_name]) — fire a one-shot from an entity's AudioPlayer
    // component (random clip from its pool + jitter). No name = this entity.
    let _ = globals.set(
        "play_audio",
        lua.create_function(|_, target: Option<String>| {
            push_command(ScriptCommand::Action {
                name: "play_audio_player".to_string(),
                target_entity: target.filter(|s| !s.is_empty()),
                args: Vec::new(),
            });
            Ok(())
        })
        .unwrap(),
    );

    // -- Physics --
    let _ = globals.set(
        "apply_force",
        lua.create_function(|_, (x, y, z): (f32, f32, f32)| {
            push_command(ScriptCommand::ApplyForce {
                entity_id: None,
                force: [x, y, z],
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "apply_impulse",
        lua.create_function(|_, (x, y, z): (f32, f32, f32)| {
            push_command(ScriptCommand::ApplyImpulse {
                entity_id: None,
                impulse: [x, y, z],
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "set_velocity",
        lua.create_function(|_, (x, y, z): (f32, f32, f32)| {
            push_command(ScriptCommand::SetVelocity {
                entity_id: None,
                velocity: [x, y, z],
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "set_gravity_scale",
        lua.create_function(|_, scale: f32| {
            push_command(ScriptCommand::SetGravityScale {
                entity_id: None,
                scale,
            });
            Ok(())
        })
        .unwrap(),
    );

    // -- Timers --
    let _ = globals.set(
        "start_timer",
        lua.create_function(|_, (name, duration, repeat): (String, f32, Option<bool>)| {
            push_command(ScriptCommand::StartTimer {
                name,
                duration,
                repeat: repeat.unwrap_or(false),
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "stop_timer",
        lua.create_function(|_, name: String| {
            push_command(ScriptCommand::StopTimer { name });
            Ok(())
        })
        .unwrap(),
    );

    // -- Debug --
    let _ = globals.set(
        "print_log",
        lua.create_function(|_, msg: String| {
            push_command(ScriptCommand::Log {
                level: "Info".into(),
                message: msg,
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "draw_line",
        lua.create_function(
            |_, (sx, sy, sz, ex, ey, ez, duration): (f32, f32, f32, f32, f32, f32, Option<f32>)| {
                push_command(ScriptCommand::DrawLine {
                    start: [sx, sy, sz],
                    end: [ex, ey, ez],
                    color: [1.0, 0.0, 0.0, 1.0],
                    duration: duration.unwrap_or(0.0),
                });
                Ok(())
            },
        )
        .unwrap(),
    );

    // -- Rendering --
    let _ = globals.set(
        "set_visibility",
        lua.create_function(|_, visible: bool| {
            push_command(ScriptCommand::SetVisibility {
                entity_id: None,
                visible,
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "set_material_color",
        lua.create_function(|_, (r, g, b, a): (f32, f32, f32, Option<f32>)| {
            push_command(ScriptCommand::SetMaterialColor {
                entity_id: None,
                color: [r, g, b, a.unwrap_or(1.0)],
            });
            Ok(())
        })
        .unwrap(),
    );

    // -- Animation --
    let _ = globals.set(
        "play_animation",
        lua.create_function(
            |_, (name, looping, speed): (String, Option<bool>, Option<f32>)| {
                push_command(ScriptCommand::PlayAnimation {
                    entity_id: None,
                    name,
                    looping: looping.unwrap_or(true),
                    speed: speed.unwrap_or(1.0),
                });
                Ok(())
            },
        )
        .unwrap(),
    );
    let _ = globals.set(
        "stop_animation",
        lua.create_function(|_, ()| {
            push_command(ScriptCommand::StopAnimation { entity_id: None });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "pause_animation",
        lua.create_function(|_, ()| {
            push_command(ScriptCommand::PauseAnimation { entity_id: None });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "resume_animation",
        lua.create_function(|_, ()| {
            push_command(ScriptCommand::ResumeAnimation { entity_id: None });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "set_animation_speed",
        lua.create_function(|_, speed: f32| {
            push_command(ScriptCommand::SetAnimationSpeed {
                entity_id: None,
                speed,
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "seek_animation",
        lua.create_function(|_, time: f32| {
            push_command(ScriptCommand::SeekAnimation {
                entity_id: None,
                time,
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "get_animation_time",
        lua.create_function(|_, ()| {
            Ok(
                match host::call_get(None, "AnimatorReadState", "time") {
                    Some(PropValue::Float(f)) => f,
                    _ => 0.0,
                },
            )
        })
        .unwrap(),
    );
    let _ = globals.set(
        "is_animation_playing",
        lua.create_function(|_, ()| {
            Ok(matches!(
                host::call_get(None, "AnimatorReadState", "playing"),
                Some(PropValue::Bool(true))
            ))
        })
        .unwrap(),
    );
    let _ = globals.set(
        "crossfade_animation",
        lua.create_function(
            |_, (name, duration, looping): (String, f32, Option<bool>)| {
                push_command(ScriptCommand::CrossfadeAnimation {
                    entity_id: None,
                    name,
                    duration,
                    looping: looping.unwrap_or(true),
                });
                Ok(())
            },
        )
        .unwrap(),
    );
    let _ = globals.set(
        "set_anim_param",
        lua.create_function(|_, (name, value): (String, f32)| {
            push_command(ScriptCommand::SetAnimationParam {
                entity_id: None,
                name,
                value,
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "set_anim_bool",
        lua.create_function(|_, (name, value): (String, bool)| {
            push_command(ScriptCommand::SetAnimationBoolParam {
                entity_id: None,
                name,
                value,
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "trigger_anim",
        lua.create_function(|_, name: String| {
            push_command(ScriptCommand::TriggerAnimation {
                entity_id: None,
                name,
            });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "set_layer_weight",
        lua.create_function(|_, (layer_name, weight): (String, f32)| {
            push_command(ScriptCommand::SetAnimationLayerWeight {
                entity_id: None,
                layer_name,
                weight,
            });
            Ok(())
        })
        .unwrap(),
    );

    // -- Cursor --
    let _ = globals.set(
        "lock_cursor",
        lua.create_function(|_, ()| {
            push_command(ScriptCommand::LockCursor);
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "unlock_cursor",
        lua.create_function(|_, ()| {
            push_command(ScriptCommand::UnlockCursor);
            Ok(())
        })
        .unwrap(),
    );

    // -- Camera --
    let _ = globals.set(
        "screen_shake",
        lua.create_function(|_, (intensity, duration): (f32, f32)| {
            push_command(ScriptCommand::ScreenShake {
                intensity,
                duration,
            });
            Ok(())
        })
        .unwrap(),
    );

    // -- ECS --
    let _ = globals.set(
        "spawn_entity",
        lua.create_function(|_, name: String| {
            push_command(ScriptCommand::SpawnEntity { name });
            Ok(())
        })
        .unwrap(),
    );
    // spawn_primitive(name, kind, x, y, z, [r, g, b])
    //   kind: "cube" | "sphere" | "wall" | … (any id in ShapeRegistry)
    //   r/g/b: optional, default to the shape's registered tint.
    //
    // Useful for procedural-generation scripts (voxel maps, particle
    // emitters, etc.) — the spawned entity gets a `MeshPrimitive`
    // component which the engine's rehydration system picks up next
    // frame and turns into a real `Mesh3d` + `MeshMaterial3d`.
    //
    // Typed tuple args go through mlua's `FromLua` impl for `f32`,
    // which coerces both Lua integer and float into f32. The earlier
    // `LuaValue::as_f32()` path silently dropped integers (Lua's
    // numeric for-loops yield integers) and every cube landed at the
    // origin.
    let _ = globals.set(
        "spawn_primitive",
        lua.create_function(
            |_,
             (name, kind, x, y, z, r, g, b): (
                String,
                String,
                f32,
                f32,
                f32,
                Option<f32>,
                Option<f32>,
                Option<f32>,
            )| {
                let color = match (r, g, b) {
                    (Some(r), Some(g), Some(b)) => Some([r, g, b, 1.0]),
                    _ => None,
                };
                push_command(ScriptCommand::SpawnPrimitive {
                    name,
                    primitive_type: kind,
                    position: Some([x, y, z]),
                    scale: None,
                    color,
                });
                Ok(())
            },
        )
        .unwrap(),
    );
    let _ = globals.set(
        "despawn_self",
        lua.create_function(|_, ()| {
            push_command(ScriptCommand::DespawnSelf);
            Ok(())
        })
        .unwrap(),
    );
    // despawn_by_prefix("chunk_3_5_") — evicts every entity whose
    // Name starts with the prefix. Used by streaming-world scripts
    // that name spawned entities by chunk coordinate so the script
    // can release a chunk in a single call instead of looping over
    // every cube it spawned.
    let _ = globals.set(
        "despawn_by_prefix",
        lua.create_function(|_, prefix: String| {
            push_command(ScriptCommand::DespawnByPrefix { prefix });
            Ok(())
        })
        .unwrap(),
    );

    // -- Scene --
    let _ = globals.set(
        "load_scene",
        lua.create_function(|_, path: String| {
            push_command(ScriptCommand::LoadScene { path });
            Ok(())
        })
        .unwrap(),
    );

    // -- Environment --
    let _ = globals.set(
        "set_sun_angles",
        lua.create_function(|_, (azimuth, elevation): (f32, f32)| {
            push_command(ScriptCommand::SetSunAngles { azimuth, elevation });
            Ok(())
        })
        .unwrap(),
    );
    let _ = globals.set(
        "set_fog",
        lua.create_function(|_, (enabled, start, end): (bool, f32, f32)| {
            push_command(ScriptCommand::SetFog {
                enabled,
                start,
                end,
            });
            Ok(())
        })
        .unwrap(),
    );

    // -- Generic Reflection (set/set_on) --
    // set("ComponentType.field.subfield", value) — on self entity
    let _ = globals.set(
        "set",
        lua.create_function(|_, (path, value): (String, LuaValue)| {
            let (component, field) = parse_component_path(&path).ok_or_else(|| {
                mlua::Error::runtime(format!("Invalid path '{}'. Use 'Component.field'", path))
            })?;
            let value = lua_to_property_value(&value).ok_or_else(|| {
                mlua::Error::runtime(format!("set('{}', …): unsupported value", path))
            })?;
            push_command(ScriptCommand::SetComponentField {
                entity_id: None,
                entity_name: None,
                component_type: component,
                field_path: field,
                value,
            });
            Ok(())
        })
        .unwrap(),
    );

    // set_on("EntityName", "ComponentType.field.subfield", value) — on named entity
    let _ = globals.set(
        "set_on",
        lua.create_function(
            |_, (entity_name, path, value): (String, String, LuaValue)| {
                let (component, field) = parse_component_path(&path).ok_or_else(|| {
                    mlua::Error::runtime(format!("Invalid path '{}'. Use 'Component.field'", path))
                })?;
                let value = lua_to_property_value(&value).ok_or_else(|| {
                    mlua::Error::runtime(format!("set_on('{}', …): unsupported value", path))
                })?;
                push_command(ScriptCommand::SetComponentField {
                    entity_id: None,
                    entity_name: Some(entity_name),
                    component_type: component,
                    field_path: field,
                    value,
                });
                Ok(())
            },
        )
        .unwrap(),
    );

    // -- Generic Reflection (get/get_on) --
    // get("Component.field") — read from self entity
    let _ = globals.set(
        "get",
        lua.create_function(|lua, path: String| {
            let (component, field) = parse_component_path(&path).ok_or_else(|| {
                mlua::Error::runtime(format!("Invalid path '{}'. Use 'Component.field'", path))
            })?;
            match host::call_get(None, &component, &field) {
                Some(v) => property_value_to_lua_result(lua, v),
                None => Ok(LuaValue::Nil),
            }
        })
        .unwrap(),
    );

    // get_on("EntityName", "Component.field") — read from named entity
    let _ = globals.set(
        "get_on",
        lua.create_function(|lua, (entity_name, path): (String, String)| {
            let (component, field) = parse_component_path(&path).ok_or_else(|| {
                mlua::Error::runtime(format!("Invalid path '{}'. Use 'Component.field'", path))
            })?;
            match host::call_get(Some(&entity_name), &component, &field) {
                Some(v) => property_value_to_lua_result(lua, v),
                None => Ok(LuaValue::Nil),
            }
        })
        .unwrap(),
    );

    // -- Script Actions (generic events for domain crates) --
    // action("name", { key = value, ... }) — triggers a ScriptAction event
    let _ = globals.set(
        "action",
        lua.create_function(|_, (name, args): (String, Option<LuaTable>)| {
            let mut map = Vec::new();
            if let Some(tbl) = args {
                for (k, v) in tbl.pairs::<String, LuaValue>().flatten() {
                    map.push((k, lua_to_action_value(&v)));
                }
            }
            push_command(ScriptCommand::Action {
                name,
                target_entity: None,
                args: map,
            });
            Ok(())
        })
        .unwrap(),
    );

    // -- Broadcast events --
    // emit("name", { key = value, ... }) — every script's on_event(name, args)
    // fires next frame, as do Rust observers of `renzora::GameEvent`.
    //
    // Use this when the sender shouldn't have to know who is listening ("the
    // boss died"); use set_on/get_on when you know exactly which entity you
    // mean ("turn the music down"). Delivery is next-frame, so a script cannot
    // observe its own emit within the same hook.
    let _ = globals.set(
        "emit",
        lua.create_function(|_, (name, args): (String, Option<LuaTable>)| {
            let mut map = Vec::new();
            if let Some(tbl) = args {
                for (k, v) in tbl.pairs::<String, LuaValue>().flatten() {
                    map.push((k, lua_to_action_value(&v)));
                }
            }
            push_command(ScriptCommand::Emit { name, args: map });
            Ok(())
        })
        .unwrap(),
    );

    // -- HTTP (async) --
    // http_get(url [, callback]) — fire a GET; the response is delivered to
    // on_http(callback, status, body) next frame. callback defaults to "get".
    let _ = globals.set(
        "http_get",
        lua.create_function(|_, (url, callback): (String, Option<String>)| {
            push_command(ScriptCommand::HttpRequest {
                method: "GET".into(),
                url,
                body: None,
                callback: callback.unwrap_or_else(|| "get".into()),
            });
            Ok(())
        })
        .unwrap(),
    );
    // http_post(url, body [, callback]) — POST a JSON body string. Response →
    // on_http(callback, status, body). callback defaults to "post".
    let _ = globals.set(
        "http_post",
        lua.create_function(
            |_, (url, body, callback): (String, String, Option<String>)| {
                push_command(ScriptCommand::HttpRequest {
                    method: "POST".into(),
                    url,
                    body: Some(body),
                    callback: callback.unwrap_or_else(|| "post".into()),
                });
                Ok(())
            },
        )
        .unwrap(),
    );
    // json_parse(str) -> table — decode a JSON string into a Lua table/value.
    // Returns nil on parse error.
    let _ = globals.set(
        "json_parse",
        lua.create_function(|lua, s: String| {
            match serde_json::from_str::<serde_json::Value>(&s) {
                Ok(v) => json_to_lua(lua, &v),
                Err(_) => Ok(LuaValue::Nil),
            }
        })
        .unwrap(),
    );

    // -- Network status --
    // net_is_server() — true on the dedicated/host server. Gate
    // server-authoritative logic with this so it doesn't also run on clients.
    let _ = globals.set(
        "net_is_server",
        lua.create_function(|lua, ()| Ok(lua.globals().get::<bool>("_net_is_server").unwrap_or(false)))
            .unwrap(),
    );
    // net_is_client() — true when networking is active and this is not the server.
    let _ = globals.set(
        "net_is_client",
        lua.create_function(|lua, ()| {
            let is_server = lua.globals().get::<bool>("_net_is_server").unwrap_or(false);
            let connected = lua.globals().get::<bool>("_net_is_connected").unwrap_or(false);
            Ok(connected && !is_server)
        })
        .unwrap(),
    );
    // net_is_connected() — connected to a server (client) or running (server).
    let _ = globals.set(
        "net_is_connected",
        lua.create_function(|lua, ()| {
            Ok(lua.globals().get::<bool>("_net_is_connected").unwrap_or(false))
        })
        .unwrap(),
    );
    // net_player_count() — connected client count (server only; 0 elsewhere).
    let _ = globals.set(
        "net_player_count",
        lua.create_function(|lua, ()| Ok(lua.globals().get::<i64>("_net_player_count").unwrap_or(0)))
            .unwrap(),
    );

    // rpc("name", { key = value, ... }) — fire a networked RPC. Emits a
    // `net_rpc` action carrying the RPC name in the reserved `__rpc` key;
    // renzora_network sends it over the wire and remote peers invoke their
    // `on_rpc(name, args)` hook. The reserved key must match
    // `renzora_network::rpc::RPC_NAME_KEY`.
    let _ = globals.set(
        "rpc",
        lua.create_function(|_, (name, args): (String, Option<LuaTable>)| {
            let mut map = Vec::new();
            if let Some(tbl) = args {
                for (k, v) in tbl.pairs::<String, LuaValue>().flatten() {
                    map.push((k, lua_to_action_value(&v)));
                }
            }
            map.push((
                "__rpc".to_string(),
                ActionValue::String(name),
            ));
            push_command(ScriptCommand::Action {
                name: "net_rpc".to_string(),
                target_entity: None,
                args: map,
            });
            Ok(())
        })
        .unwrap(),
    );

    // action_on("EntityName", "name", { key = value, ... }) — action targeting another entity
    let _ = globals.set(
        "action_on",
        lua.create_function(
            |_, (target, name, args): (String, String, Option<LuaTable>)| {
                let mut map = Vec::new();
                if let Some(tbl) = args {
                    for (k, v) in tbl.pairs::<String, LuaValue>().flatten() {
                        map.push((k, lua_to_action_value(&v)));
                    }
                }
                push_command(ScriptCommand::Action {
                    name,
                    target_entity: Some(target),
                    args: map,
                });
                Ok(())
            },
        )
        .unwrap(),
    );

    // -- Component Reflection --
    // get_component("ComponentType") — returns all fields as a table
    let _ = globals.set(
        "get_component",
        lua.create_function(|lua, component_type: String| {
            match host::call_get_component(None, &component_type) {
                Some(fields) => {
                    let t = lua.create_table()?;
                    for (key, val) in fields {
                        if let Ok(lv) = property_value_to_lua_result(lua, val) {
                            let _ = t.set(key, lv);
                        }
                    }
                    Ok(LuaValue::Table(t))
                }
                None => Ok(LuaValue::Nil),
            }
        })
        .unwrap(),
    );

    // get_component_on("EntityName", "ComponentType") — returns all fields from named entity
    let _ = globals.set(
        "get_component_on",
        lua.create_function(|lua, (entity_name, component_type): (String, String)| {
            match host::call_get_component(Some(&entity_name), &component_type) {
                Some(fields) => {
                    let t = lua.create_table()?;
                    for (key, val) in fields {
                        if let Ok(lv) = property_value_to_lua_result(lua, val) {
                            let _ = t.set(key, lv);
                        }
                    }
                    Ok(LuaValue::Table(t))
                }
                None => Ok(LuaValue::Nil),
            }
        })
        .unwrap(),
    );

    // get_components() — list all reflected component names on self
    let _ = globals.set(
        "get_components",
        lua.create_function(|lua, ()| {
            let names = host::call_get_components(None);
            let t = lua.create_table()?;
            for (i, name) in names.iter().enumerate() {
                t.set(i + 1, name.as_str())?;
            }
            Ok(t)
        })
        .unwrap(),
    );

    // get_components_on("EntityName") — list component names on named entity
    let _ = globals.set(
        "get_components_on",
        lua.create_function(|lua, entity_name: String| {
            let names = host::call_get_components(Some(&entity_name));
            let t = lua.create_table()?;
            for (i, name) in names.iter().enumerate() {
                t.set(i + 1, name.as_str())?;
            }
            Ok(t)
        })
        .unwrap(),
    );

    // has_component("ComponentType") — check if self has a component
    let _ = globals.set(
        "has_component",
        lua.create_function(|_, component_type: String| {
            Ok(host::call_get_component(None, &component_type).is_some())
        })
        .unwrap(),
    );

    // has_component_on("EntityName", "ComponentType") — check on named entity
    let _ = globals.set(
        "has_component_on",
        lua.create_function(|_, (entity_name, component_type): (String, String)| {
            Ok(
                host::call_get_component(Some(&entity_name), &component_type)
                    .is_some(),
            )
        })
        .unwrap(),
    );

    // -- Asset Load Progress --
    // asset_progress() — returns the runtime asset-load tracker as a table.
    // Returns nil when no scene is loading (idle / no rpak / no scene yet).
    // Fields: state ("idle"/"loading"/"done"), total_files, loaded_files,
    // total_bytes, loaded_bytes, fraction (0..1), current_path, elapsed_secs.
    //
    // Typical loading-screen pattern in a script attached to the boot scene:
    //   function on_update()
    //     local p = asset_progress()
    //     if p == nil then return end
    //     action("ui_set_progress", { name="LoadBar", value=p.fraction })
    //     if p.current_path then
    //       action("ui_set_text", { name="LoadLabel", text=p.current_path })
    //     end
    //     if p.state == "done" then
    //       action("ui_hide", { name="LoadingScreen" })
    //     end
    //   end
    let _ = globals.set(
        "asset_progress",
        lua.create_function(|lua, ()| {
            let Some(snapshot) = host::call_asset_progress() else {
                return Ok(LuaValue::Nil);
            };
            let t = lua.create_table()?;
            t.set("state", snapshot.state)?;
            t.set("total_files", snapshot.total_files)?;
            t.set("loaded_files", snapshot.loaded_files)?;
            t.set("total_bytes", snapshot.total_bytes as f64)?;
            t.set("loaded_bytes", snapshot.loaded_bytes as f64)?;
            t.set("fraction", snapshot.fraction)?;
            t.set("elapsed_secs", snapshot.elapsed_secs)?;
            match snapshot.current_path {
                Some(p) => t.set("current_path", p)?,
                None => t.set("current_path", LuaValue::Nil)?,
            }
            Ok(LuaValue::Table(t))
        })
        .unwrap(),
    );

    // scene_load_state() — which scene is loading and how far through spawning
    // it is, as a table. Returns nil before any scene load has been observed.
    // Fields: phase ("idle"/"loading"/"ready"/"failed"), current_path, progress.
    //
    // Distinct from asset_progress(): this tracks the *scene* being spawned,
    // that tracks how many of its models have finished loading. A scene hits
    // "ready" while its meshes are still streaming, so a loading screen that
    // waits only on this one will uncover an unfinished world.
    //
    // Only scripts that survive the load see the transition — put this on a
    // Persistent entity (an autoload/global scene), not in the scene itself:
    //   function on_update()
    //     local s = scene_load_state()
    //     if s and s.phase == "loading" then
    //       action("ui_show", { name = "LoadingScreen" })
    //     end
    //   end
    //   function on_scene_loaded(path) action("ui_hide", { name="LoadingScreen" }) end
    //   function on_scene_load_failed(path, err) print("load failed: "..err) end
    let _ = globals.set(
        "scene_load_state",
        lua.create_function(|lua, ()| {
            let Some(snapshot) = host::call_scene_load_state() else {
                return Ok(LuaValue::Nil);
            };
            let t = lua.create_table()?;
            t.set("phase", snapshot.phase)?;
            t.set("progress", snapshot.progress)?;
            match snapshot.current_path {
                Some(p) => t.set("current_path", p)?,
                None => t.set("current_path", LuaValue::Nil)?,
            }
            Ok(LuaValue::Table(t))
        })
        .unwrap(),
    );

    // is_loading() — convenience boolean wrapper around asset_progress().state.
    let _ = globals.set(
        "is_loading",
        lua.create_function(|_, ()| {
            Ok(host::call_asset_progress()
                .map(|s| s.state == "loading")
                .unwrap_or(false))
        })
        .unwrap(),
    );

    // is_loaded() — true once every tracked asset has finished loading.
    let _ = globals.set(
        "is_loaded",
        lua.create_function(|_, ()| {
            Ok(host::call_asset_progress()
                .map(|s| s.state == "done")
                .unwrap_or(false))
        })
        .unwrap(),
    );

    // -- Math helpers --
    let _ = globals.set(
        "vec3",
        lua.create_function(|lua, (x, y, z): (f32, f32, f32)| {
            let t = lua.create_table()?;
            t.set("x", x)?;
            t.set("y", y)?;
            t.set("z", z)?;
            Ok(t)
        })
        .unwrap(),
    );
    let _ = globals.set(
        "vec2",
        lua.create_function(|lua, (x, y): (f32, f32)| {
            let t = lua.create_table()?;
            t.set("x", x)?;
            t.set("y", y)?;
            Ok(t)
        })
        .unwrap(),
    );
    let _ = globals.set(
        "lerp",
        lua.create_function(|_, (a, b, t): (f32, f32, f32)| Ok(a + (b - a) * t))
            .unwrap(),
    );
    let _ = globals.set(
        "clamp",
        lua.create_function(|_, (v, min, max): (f32, f32, f32)| Ok(v.max(min).min(max)))
            .unwrap(),
    );
}

// Helper to register a 3-arg (f32, f32, f32) -> () function
fn register_fn3(lua: &Lua, globals: &LuaTable, name: &str, f: fn(f32, f32, f32)) {
    let _ = globals.set(
        name,
        lua.create_function(move |_, (x, y, z): (f32, f32, f32)| {
            f(x, y, z);
            Ok(())
        })
        .unwrap(),
    );
}

fn register_fn1(lua: &Lua, globals: &LuaTable, name: &str, f: fn(f32)) {
    let _ = globals.set(
        name,
        lua.create_function(move |_, v: f32| {
            f(v);
            Ok(())
        })
        .unwrap(),
    );
}

// =============================================================================
// Context marshalling
// =============================================================================

fn set_context_globals(lua: &Lua, ctx: &Ctx, vars: &[(String, ScriptValue)]) {
    let g = lua.globals();
    let frame = ctx.frame;
    let ent = ctx.entity;

    // Time
    let _ = g.set("delta", frame.time.delta as f64);
    let _ = g.set("elapsed", frame.time.elapsed);

    // Transform. Rotation comes over as degrees already — see the note on
    // `EntityContext::rotation_euler` for why the engine converts rather than
    // each language plugin.
    let _ = g.set("position_x", ent.position[0] as f64);
    let _ = g.set("position_y", ent.position[1] as f64);
    let _ = g.set("position_z", ent.position[2] as f64);
    let _ = g.set("rotation_x", ent.rotation_euler[0] as f64);
    let _ = g.set("rotation_y", ent.rotation_euler[1] as f64);
    let _ = g.set("rotation_z", ent.rotation_euler[2] as f64);
    let _ = g.set("scale_x", ent.scale[0] as f64);
    let _ = g.set("scale_y", ent.scale[1] as f64);
    let _ = g.set("scale_z", ent.scale[2] as f64);

    // Input
    let _ = g.set("input_x", frame.input_movement[0] as f64);
    let _ = g.set("input_y", frame.input_movement[1] as f64);
    let _ = g.set("mouse_x", frame.mouse_position[0] as f64);
    let _ = g.set("mouse_y", frame.mouse_position[1] as f64);
    let _ = g.set("mouse_delta_x", frame.mouse_delta[0] as f64);
    let _ = g.set("mouse_delta_y", frame.mouse_delta[1] as f64);
    let _ = g.set("camera_yaw", frame.camera_yaw as f64);

    // Mouse buttons
    let _ = g.set("mouse_left", frame.mouse_buttons_pressed[0]);
    let _ = g.set("mouse_right", frame.mouse_buttons_pressed[1]);
    let _ = g.set("mouse_middle", frame.mouse_buttons_pressed[2]);
    let _ = g.set(
        "mouse_left_just_pressed",
        frame.mouse_buttons_just_pressed[0],
    );
    let _ = g.set(
        "mouse_right_just_pressed",
        frame.mouse_buttons_just_pressed[1],
    );
    let _ = g.set("mouse_scroll", frame.mouse_scroll as f64);

    // Camera state — live scene EV-100 from auto-exposure readback.
    let _ = g.set("camera_ev", frame.camera_ev as f64);

    // Project — configured game resolution (world units). Handy for 2D:
    // centre a follow camera by offsetting half of these (top-left origin).
    let _ = g.set("project_width", frame.project_width as f64);
    let _ = g.set("project_height", frame.project_height as f64);

    // Gamepad. The legacy single-pad globals are the first connected pad; the
    // engine sends the list and this derives them rather than sending both.
    let first = frame.gamepads.first();
    let axis = |f: fn(&GamepadSnapshot) -> f32| first.map(f).unwrap_or(0.0);
    let _ = g.set("gamepad_left_x", axis(|p| p.left_stick[0]) as f64);
    let _ = g.set("gamepad_left_y", axis(|p| p.left_stick[1]) as f64);
    let _ = g.set("gamepad_right_x", axis(|p| p.right_stick[0]) as f64);
    let _ = g.set("gamepad_right_y", axis(|p| p.right_stick[1]) as f64);
    let _ = g.set("gamepad_left_trigger", axis(|p| p.left_trigger) as f64);
    let _ = g.set("gamepad_right_trigger", axis(|p| p.right_trigger) as f64);
    // Buttons: South(X/A), East(O/B), West(square/X), North(triangle/Y),
    //          L1, R1, L2, R2, Select, Start, L3, R3,
    //          DPadUp, DPadDown, DPadLeft, DPadRight
    let buttons = first.map(|p| p.buttons).unwrap_or([false; 16]);
    for (i, name) in GAMEPAD_BUTTON_NAMES.iter().enumerate() {
        let _ = g.set(format!("gamepad_{name}"), buttons[i]);
    }

    // Multi-gamepad: `_gamepads` keyed by stable pad slot id, read through
    // gamepad_count() / gamepad_axis() / gamepad_button() etc.
    let _ = g.set("_gamepad_count", frame.gamepads.len() as i64);
    if let Ok(pads) = lua.create_table() {
        for pad in &frame.gamepads {
            let Ok(pad_t) = lua.create_table() else {
                continue;
            };
            let _ = pad_t.set("left_x", pad.left_stick[0] as f64);
            let _ = pad_t.set("left_y", pad.left_stick[1] as f64);
            let _ = pad_t.set("right_x", pad.right_stick[0] as f64);
            let _ = pad_t.set("right_y", pad.right_stick[1] as f64);
            let _ = pad_t.set("left_trigger", pad.left_trigger as f64);
            let _ = pad_t.set("right_trigger", pad.right_trigger as f64);
            if let Ok(bt) = lua.create_table() {
                for (i, name) in GAMEPAD_BUTTON_NAMES.iter().enumerate() {
                    let _ = bt.set(*name, pad.buttons[i]);
                }
                let _ = pad_t.set("buttons", bt);
            }
            if let Ok(just) = lua.create_table() {
                for (i, name) in GAMEPAD_BUTTON_NAMES.iter().enumerate() {
                    let _ = just.set(*name, pad.buttons_just_pressed[i]);
                }
                let _ = pad_t.set("just_pressed", just);
            }
            let _ = pads.set(pad.id as i64, pad_t);
        }
        let _ = g.set("_gamepads", pads);
    }

    // Entity
    let _ = g.set("self_entity_id", ent.entity_id as i64);
    let _ = g.set("self_entity_name", ent.name.clone());

    // Network status (read via net_is_server() / net_is_connected() / etc.)
    let _ = g.set("_net_is_server", frame.net_is_server);
    let _ = g.set("_net_is_connected", frame.net_is_connected);
    let _ = g.set("_net_player_count", frame.net_player_count);

    // Keyboard maps. The boundary carries only the keys that are down, so a
    // lookup that misses reads `nil` — which is falsey in Lua, exactly as the
    // `false` entries the old dense map carried were.
    let set_flags = |name: &str, names: &[String]| {
        if let Ok(t) = lua.create_table() {
            for k in names {
                let _ = t.set(k.as_str(), true);
            }
            let _ = g.set(name, t);
        }
    };
    set_flags("_keys_pressed", &frame.keys_pressed);
    set_flags("_keys_just_pressed", &frame.keys_just_pressed);
    set_flags("_keys_just_released", &frame.keys_just_released);

    // Action-based input (InputMap). Exposed as _action_* tables keyed by
    // action name; Lua side reads via `input_button_pressed("jump")` etc.
    set_flags("_action_pressed", &frame.actions_pressed);
    set_flags("_action_just_pressed", &frame.actions_just_pressed);
    set_flags("_action_just_released", &frame.actions_just_released);
    if let Ok(t) = lua.create_table() {
        for (k, v) in &frame.action_axis_1d {
            let _ = t.set(k.as_str(), *v as f64);
        }
        let _ = g.set("_action_axis_1d", t);
    }
    if let Ok(t) = lua.create_table() {
        for (k, v) in &frame.action_axis_2d {
            if let Ok(pair) = lua.create_table() {
                let _ = pair.set(1, v[0] as f64);
                let _ = pair.set(2, v[1] as f64);
                let _ = t.set(k.as_str(), pair);
            }
        }
        let _ = g.set("_action_axis_2d", t);
    }

    // Collisions
    let _ = g.set("is_colliding", !ent.active_collisions.is_empty());

    // Timers
    if let Ok(t) = lua.create_table() {
        for (i, name) in frame.timers_just_finished.iter().enumerate() {
            let _ = t.set(i + 1, name.clone());
        }
        let _ = g.set("timers_finished", t);
    }

    // Health
    let _ = g.set("self_health", ent.health as f64);
    let _ = g.set("self_max_health", ent.max_health as f64);

    // Parent
    let _ = g.set("has_parent", ent.has_parent);
    let _ = g.set("parent_position_x", ent.parent_position[0] as f64);
    let _ = g.set("parent_position_y", ent.parent_position[1] as f64);
    let _ = g.set("parent_position_z", ent.parent_position[2] as f64);

    // Script variables as globals
    for (key, value) in vars {
        match value {
            ScriptValue::Float(v) => {
                let _ = g.set(key.as_str(), *v as f64);
            }
            ScriptValue::Int(v) => {
                let _ = g.set(key.as_str(), *v as i64);
            }
            ScriptValue::Bool(v) => {
                let _ = g.set(key.as_str(), *v);
            }
            ScriptValue::String(v) | ScriptValue::Entity(v) => {
                let _ = g.set(key.as_str(), v.clone());
            }
            ScriptValue::Vec2(v) => {
                if let Ok(t) = lua.create_table() {
                    let _ = t.set("x", v[0] as f64);
                    let _ = t.set("y", v[1] as f64);
                    let _ = g.set(key.as_str(), t);
                }
            }
            ScriptValue::Vec3(v) => {
                if let Ok(t) = lua.create_table() {
                    let _ = t.set("x", v[0] as f64);
                    let _ = t.set("y", v[1] as f64);
                    let _ = t.set("z", v[2] as f64);
                    let _ = g.set(key.as_str(), t);
                }
            }
            ScriptValue::Color(v) => {
                if let Ok(t) = lua.create_table() {
                    let _ = t.set("r", v[0] as f64);
                    let _ = t.set("g", v[1] as f64);
                    let _ = t.set("b", v[2] as f64);
                    let _ = t.set("a", v[3] as f64);
                    let _ = g.set(key.as_str(), t);
                }
            }
        }
    }
}

/// Read the script's prop globals back out after a hook.
///
/// Only the names the inspector already knows are read back — a script writing
/// a fresh global is not silently promoted to a prop, which is what the engine
/// did before the move and what keeps the inspector's row list stable.
fn read_back_variables(lua: &Lua, vars: &[(String, ScriptValue)]) -> Vec<(String, ScriptValue)> {
    let g = lua.globals();
    let mut out = Vec::with_capacity(vars.len());
    for (name, _) in vars {
        if let Ok(value) = g.get::<LuaValue>(name.as_str()) {
            if let Some(sv) = lua_to_script_value(&value) {
                out.push((name.clone(), sv));
            }
        }
    }
    out
}

/// Turn a decoded JSON value into a Lua one, for `json_decode`.
fn json_to_lua(lua: &Lua, value: &serde_json::Value) -> mlua::Result<LuaValue> {
    use serde_json::Value as J;
    match value {
        J::Null => Ok(LuaValue::Nil),
        J::Bool(b) => Ok(LuaValue::Boolean(*b)),
        J::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i))
            } else {
                Ok(LuaValue::Number(n.as_f64().unwrap_or(0.0)))
            }
        }
        J::String(s) => Ok(LuaValue::String(lua.create_string(s)?)),
        J::Array(arr) => {
            let t = lua.create_table()?;
            for (i, e) in arr.iter().enumerate() {
                t.set(i + 1, json_to_lua(lua, e)?)?;
            }
            Ok(LuaValue::Table(t))
        }
        J::Object(map) => {
            let t = lua.create_table()?;
            for (k, v) in map {
                t.set(k.as_str(), json_to_lua(lua, v)?)?;
            }
            Ok(LuaValue::Table(t))
        }
    }
}

/// A Lua table recognised as engine data.
///
/// The three converters below each used to re-implement this recognition with
/// slightly different rules, so the *same* table meant different things
/// depending on which boundary it crossed: `{x=1, y=2}` was a `Vec2` as a script
/// variable but matched nothing as a reflection write (and fell through to a
/// `0.0` that could silently overwrite an unrelated float field), and no colour
/// table was recognised over RPC at all. One classifier, one precedence order,
/// so a table means one thing everywhere; each converter then maps the shape
/// into whatever its own value enum can actually carry.
#[derive(Clone, Copy, PartialEq, Debug)]
enum TableShape {
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Color([f32; 4]),
}

/// Recognise a Lua table as a vector or a colour.
///
/// The precedence is deliberate and shared by every caller: named fields beat
/// positional ones, and within each group the widest match wins so `{x,y,z}` is
/// never mistaken for `{x,y}`. Alpha defaults to opaque, matching the engine's
/// colour convention. A table matching nothing returns `None` rather than a
/// zero, so callers can refuse the write instead of corrupting a field.
fn classify_table(t: &LuaTable) -> Option<TableShape> {
    if let (Ok(x), Ok(y)) = (t.get::<f64>("x"), t.get::<f64>("y")) {
        if let Ok(z) = t.get::<f64>("z") {
            return Some(TableShape::Vec3([x as f32, y as f32, z as f32]));
        }
        return Some(TableShape::Vec2([x as f32, y as f32]));
    }
    if let (Ok(r), Ok(g), Ok(b)) = (t.get::<f64>("r"), t.get::<f64>("g"), t.get::<f64>("b")) {
        let a: f64 = t.get("a").unwrap_or(1.0);
        return Some(TableShape::Color([r as f32, g as f32, b as f32, a as f32]));
    }
    // Positional: {r,g,b,a} / {x,y,z} / {x,y}.
    if let (Ok(v1), Ok(v2)) = (t.get::<f64>(1), t.get::<f64>(2)) {
        if let Ok(v3) = t.get::<f64>(3) {
            if let Ok(v4) = t.get::<f64>(4) {
                return Some(TableShape::Color([v1 as f32, v2 as f32, v3 as f32, v4 as f32]));
            }
            return Some(TableShape::Vec3([v1 as f32, v2 as f32, v3 as f32]));
        }
        return Some(TableShape::Vec2([v1 as f32, v2 as f32]));
    }
    None
}

fn lua_to_script_value(value: &LuaValue) -> Option<ScriptValue> {
    match value {
        LuaValue::Number(n) => Some(ScriptValue::Float(*n as f32)),
        LuaValue::Integer(n) => Some(ScriptValue::Int(*n as i32)),
        LuaValue::Boolean(b) => Some(ScriptValue::Bool(*b)),
        LuaValue::String(s) => Some(ScriptValue::String(s.to_str().ok()?.to_string())),
        // The only target enum that can carry every shape, so this is a straight
        // mapping — the other two below have to degrade.
        LuaValue::Table(t) => match classify_table(t)? {
            TableShape::Vec2(v) => Some(ScriptValue::Vec2(v)),
            TableShape::Vec3(v) => Some(ScriptValue::Vec3(v)),
            TableShape::Color(v) => Some(ScriptValue::Color(v)),
        },
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

/// Convert a Lua value to a [`PropValue`] for reflection writes.
///
/// Returns `None` for anything unrecognised so `set`/`set_on` can raise a script
/// error. This used to fall through to `Float(0.0)`, which was the worst
/// available answer: a typo'd table wrote a real zero into whatever field the
/// path named, so the script looked like it worked and the value was silently
/// wrong.
///
/// `PropValue` has no `Vec2`, so a two-component table is promoted to `Vec3`
/// with `z = 0.0`. Adding a `Vec2` variant would be the honest fix, but it is a
/// codec change on both `PropValue` and the contract crate's `PropertyValue` —
/// i.e. a plugin ABI bump — so it is deliberately not done here. The promotion
/// at least lands `x`/`y` on a `Vec3` field instead of failing outright.
fn lua_to_property_value(value: &LuaValue) -> Option<PropValue> {
    use renzora_plugin::script::PropValue as PropertyValue;
    match value {
        LuaValue::Number(n) => Some(PropertyValue::Float(*n as f32)),
        LuaValue::Integer(n) => Some(PropertyValue::Int(*n)),
        LuaValue::Boolean(b) => Some(PropertyValue::Bool(*b)),
        LuaValue::String(s) => Some(PropertyValue::String(
            s.to_str().map(|s| s.to_string()).unwrap_or_default(),
        )),
        LuaValue::Table(t) => match classify_table(t)? {
            TableShape::Vec2([x, y]) => Some(PropertyValue::Vec3([x, y, 0.0])),
            TableShape::Vec3(v) => Some(PropertyValue::Vec3(v)),
            TableShape::Color(v) => Some(PropertyValue::Color(v)),
        },
        _ => None,
    }
}

/// Convert a PropertyValue to a Lua value (requires Lua context for strings/tables).
fn property_value_to_lua_result(
    lua: &Lua,
    value: PropValue,
) -> LuaResult<LuaValue> {
    use renzora_plugin::script::PropValue as PropertyValue;
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

/// Convert a `ScriptActionValue` back into a Lua value, for handing RPC args
/// to `on_rpc(name, args)`. Inverse of [`lua_to_action_value`].
fn action_value_to_lua(lua: &Lua, value: &ActionValue) -> LuaResult<LuaValue> {
    use ActionValue as ScriptActionValue;
    match value {
        ScriptActionValue::Float(v) => Ok(LuaValue::Number(*v as f64)),
        ScriptActionValue::Int(v) => Ok(LuaValue::Integer(*v)),
        ScriptActionValue::Bool(v) => Ok(LuaValue::Boolean(*v)),
        ScriptActionValue::String(v) => Ok(LuaValue::String(lua.create_string(v)?)),
        ScriptActionValue::Vec3(v) => {
            let t = lua.create_table()?;
            t.set("x", v[0] as f64)?;
            t.set("y", v[1] as f64)?;
            t.set("z", v[2] as f64)?;
            Ok(LuaValue::Table(t))
        }
    }
}

/// Convert a Lua value into an [`ActionValue`] for RPC arguments.
///
/// `ActionValue` is the narrowest of the three targets — no `Vec2`, no `Color` —
/// so both degrade to `Vec3`, and a colour loses its alpha. That is lossy but at
/// least *recognised*; previously any colour table fell to the catch-all below.
///
/// The catch-all no longer uses `format!("{:?}", value)`. For a table that
/// rendered a pointer (`Table(Ref(0x7f…))`), which differs run to run and is
/// useless to the receiving script — a fixed marker is at least deterministic.
fn lua_to_action_value(value: &LuaValue) -> ActionValue {
    use ActionValue as ScriptActionValue;
    match value {
        LuaValue::Number(n) => ScriptActionValue::Float(*n as f32),
        LuaValue::Integer(n) => ScriptActionValue::Int(*n),
        LuaValue::Boolean(b) => ScriptActionValue::Bool(*b),
        LuaValue::String(s) => ScriptActionValue::String(s.to_string_lossy().to_string()),
        LuaValue::Table(t) => match classify_table(t) {
            Some(TableShape::Vec2([x, y])) => ScriptActionValue::Vec3([x, y, 0.0]),
            Some(TableShape::Vec3(v)) => ScriptActionValue::Vec3(v),
            Some(TableShape::Color([r, g, b, _a])) => ScriptActionValue::Vec3([r, g, b]),
            None => ScriptActionValue::String("<table>".into()),
        },
        LuaValue::Nil => ScriptActionValue::String("nil".into()),
        _ => ScriptActionValue::String("<unsupported>".into()),
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

#[cfg(test)]
mod marshalling_tests {
    //! One table shape, one meaning, at every boundary.
    //!
    //! These exist because the three converters used to disagree: the same
    //! `{x=1, y=2}` was a `Vec2` as a script variable and an unrecognised value
    //! that silently became `0.0` as a reflection write. The assertions below
    //! are deliberately written per-shape *across all three* converters, so a
    //! future edit to one of them that forgets the others fails here.

    use super::*;

    fn table(src: &str) -> (Lua, LuaValue) {
        let lua = Lua::new();
        let v: LuaValue = lua.load(src).eval().expect("table literal");
        (lua, v)
    }

    fn shape(src: &str) -> Option<TableShape> {
        let (_lua, v) = table(src);
        match v {
            LuaValue::Table(t) => classify_table(&t),
            _ => panic!("not a table"),
        }
    }

    #[test]
    fn named_xy_is_vec2_everywhere() {
        assert_eq!(shape("return {x=1, y=2}"), Some(TableShape::Vec2([1.0, 2.0])));

        let (_lua, v) = table("return {x=1, y=2}");
        assert_eq!(lua_to_script_value(&v), Some(ScriptValue::Vec2([1.0, 2.0])));
        // No Vec2 in either narrower enum, so both promote with z = 0 rather
        // than falling through to a zero (property) or a debug string (action).
        assert_eq!(
            lua_to_property_value(&v),
            Some(PropValue::Vec3([1.0, 2.0, 0.0]))
        );
        assert_eq!(lua_to_action_value(&v), ActionValue::Vec3([1.0, 2.0, 0.0]));
    }

    #[test]
    fn named_xyz_is_vec3_everywhere() {
        let (_lua, v) = table("return {x=1, y=2, z=3}");
        assert_eq!(
            lua_to_script_value(&v),
            Some(ScriptValue::Vec3([1.0, 2.0, 3.0]))
        );
        assert_eq!(
            lua_to_property_value(&v),
            Some(PropValue::Vec3([1.0, 2.0, 3.0]))
        );
        assert_eq!(lua_to_action_value(&v), ActionValue::Vec3([1.0, 2.0, 3.0]));
    }

    #[test]
    fn rgb_defaults_alpha_to_opaque() {
        assert_eq!(
            shape("return {r=1, g=0, b=0}"),
            Some(TableShape::Color([1.0, 0.0, 0.0, 1.0]))
        );
        assert_eq!(
            shape("return {r=1, g=0, b=0, a=0.5}"),
            Some(TableShape::Color([1.0, 0.0, 0.0, 0.5]))
        );
    }

    #[test]
    fn colour_is_recognised_over_rpc_even_though_it_degrades() {
        let (_lua, v) = table("return {r=1, g=0, b=0, a=0.5}");
        assert_eq!(
            lua_to_script_value(&v),
            Some(ScriptValue::Color([1.0, 0.0, 0.0, 0.5]))
        );
        // ActionValue has no Color; alpha is dropped, but it is no longer
        // stringified into a pointer-debug.
        assert_eq!(lua_to_action_value(&v), ActionValue::Vec3([1.0, 0.0, 0.0]));
    }

    #[test]
    fn positional_tables_work_at_every_boundary() {
        assert_eq!(shape("return {1, 2}"), Some(TableShape::Vec2([1.0, 2.0])));
        assert_eq!(
            shape("return {1, 2, 3}"),
            Some(TableShape::Vec3([1.0, 2.0, 3.0]))
        );
        assert_eq!(
            shape("return {1, 2, 3, 4}"),
            Some(TableShape::Color([1.0, 2.0, 3.0, 4.0]))
        );

        // Previously only the reflection path understood positional tables.
        let (_lua, v) = table("return {1, 2, 3}");
        assert_eq!(
            lua_to_script_value(&v),
            Some(ScriptValue::Vec3([1.0, 2.0, 3.0]))
        );
    }

    #[test]
    fn named_fields_beat_positional_ones() {
        assert_eq!(
            shape("return {9, 9, 9, x=1, y=2, z=3}"),
            Some(TableShape::Vec3([1.0, 2.0, 3.0]))
        );
    }

    #[test]
    fn unrecognised_table_is_refused_not_zeroed() {
        assert_eq!(shape("return {foo=1}"), None);

        let (_lua, v) = table("return {foo=1}");
        assert_eq!(lua_to_script_value(&v), None);
        // The regression this whole module exists for: this used to be
        // `Float(0.0)`, which wrote a real zero into whatever field was named.
        assert_eq!(lua_to_property_value(&v), None);
        assert_eq!(lua_to_action_value(&v), ActionValue::String("<table>".into()));
    }
}
